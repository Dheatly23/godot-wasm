use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;

use anyhow::Error;
use cfg_if::cfg_if;
#[cfg(feature = "godot-component")]
use either::{Either, Left, Right};
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use tracing::{Level, instrument};
use wasi_isolated_fs::bindings::{Command, LinkOptions};
use wasi_isolated_fs::context::WasiContext as WasiCtx;
use wasmtime::component::{HasSelf, Linker};
use wasmtime::{AsContextMut, Store};

#[cfg(feature = "godot-component")]
use crate::godot_component::filter::Filter;
#[cfg(feature = "godot-component")]
use crate::godot_component::{GodotCtx, add_to_linker as godot_add_to_linker};
use crate::godot_util::SendSyncWrapper;
use crate::wasi_ctx::WasiContext;
use crate::wasi_ctx::stdio::PackedByteArrayReader;
use crate::wasm_config::{Config, PipeBindingType};
use crate::wasm_engine::WasmModule;
#[cfg(feature = "memory-limiter")]
use crate::wasm_instance::MemoryLimit;
use crate::wasm_instance::{InnerLock, InstanceData, InstanceType};
use crate::wasm_util::HasEpochTimeout;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::{config_store_epoch, reset_epoch};
use crate::{bail_with_site, site_context};

#[derive(Default)]
struct CommandConfig {
    config: Config,

    #[cfg(feature = "godot-component")]
    use_comp_godot: bool,
    #[cfg(feature = "godot-component")]
    filter: Filter,
}

impl Debug for CommandConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("CommandConfig")
            .field("config", &self.config)
            .field("use_comp_godot", &self.use_comp_godot)
            .finish_non_exhaustive()
    }
}

impl GodotConvert for CommandConfig {
    type Via = Dictionary;
}

impl FromGodot for CommandConfig {
    fn try_from_variant(v: &Variant) -> Result<Self, ConvertError> {
        if v.is_nil() {
            return Ok(Self::default());
        }
        Self::try_from_godot(v.try_to()?)
    }

    fn try_from_godot(via: Self::Via) -> Result<Self, ConvertError> {
        Ok(Self {
            #[cfg(feature = "godot-component")]
            use_comp_godot: via
                .get("component.godot.enable")
                .map(|v| v.try_to())
                .transpose()?
                .unwrap_or_default(),
            #[cfg(feature = "godot-component")]
            filter: via
                .get("component.godot.filter")
                .map(|v| v.try_to())
                .transpose()?
                .unwrap_or_default(),

            config: Config::try_from_godot(via)?,
        })
    }
}

#[derive(GodotClass)]
#[class(base=RefCounted, init, tool)]
pub struct WasiCommand {
    base: Base<RefCounted>,
    data: OnceCell<CommandData>,

    #[var(get = get_module)]
    #[allow(dead_code)]
    module: Option<Gd<WasmModule>>,
}

impl Debug for WasiCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("WasiCommand").field(&self.base).finish()
    }
}

pub struct CommandData {
    instance: InstanceData<StoreData>,
    bindings: Command,
}

pub struct StoreData {
    #[cfg(feature = "epoch-timeout")]
    epoch_timeout: u64,

    #[cfg(feature = "memory-limiter")]
    memory_limits: MemoryLimit,

    wasi_ctx: WasiCtx,
    #[cfg(not(feature = "godot-component"))]
    inner_lock: InnerLock,
    #[cfg(feature = "godot-component")]
    godot_ctx: Either<InnerLock, GodotCtx>,
}

impl AsRef<Self> for StoreData {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsMut<Self> for StoreData {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl AsRef<InnerLock> for StoreData {
    fn as_ref(&self) -> &InnerLock {
        cfg_if! {
            if #[cfg(feature = "godot-component")] {
                match &self.godot_ctx {
                    Left(v) => v,
                    Right(v) => v.as_ref(),
                }
            } else {
                &self.inner_lock
            }
        }
    }
}

impl AsMut<InnerLock> for StoreData {
    fn as_mut(&mut self) -> &mut InnerLock {
        cfg_if! {
            if #[cfg(feature = "godot-component")] {
                match &mut self.godot_ctx {
                    Left(v) => v,
                    Right(v) => v.as_mut(),
                }
            } else {
                &mut self.inner_lock
            }
        }
    }
}

#[cfg(feature = "godot-component")]
impl AsMut<GodotCtx> for StoreData {
    fn as_mut(&mut self) -> &mut GodotCtx {
        self.godot_ctx
            .as_mut()
            .right()
            .expect("Godot component is enabled, but no context is provided")
    }
}

impl HasEpochTimeout for StoreData {
    #[cfg(feature = "epoch-timeout")]
    fn get_epoch_timeout(&self) -> u64 {
        self.epoch_timeout
    }

    #[cfg(feature = "wasi")]
    fn get_wasi_ctx(&mut self) -> Option<&mut WasiCtx> {
        Some(&mut self.wasi_ctx)
    }
}

#[instrument]
fn instantiate(
    obj: &Gd<WasiCommand>,
    config: CommandConfig,
    module: Gd<WasmModule>,
) -> Result<CommandData, Error> {
    let CommandConfig {
        config,
        #[cfg(feature = "godot-component")]
        use_comp_godot,
        #[cfg(feature = "godot-component")]
        filter,
    } = config;
    let comp = site_context!(
        module
            .bind()
            .get_data()
            .and_then(|m| m.module.get_component().cloned())
    )?;

    let mut builder = WasiCtx::builder();
    if config.with_wasi {
        if config.wasi_stdin == PipeBindingType::Instance {
            if let Some(data) = config.wasi_stdin_data.clone() {
                builder.stdin(Arc::new(PackedByteArrayReader::from(data)))
            } else {
                let signal = SendSyncWrapper::new(Signal::from_object_signal(obj, "stdin_request"));
                builder.stdin_signal(Box::new(move || signal.emit(&[])))
            }?;
        }
        if config.wasi_stdout == PipeBindingType::Instance {
            builder.stdout(WasiContext::make_host_stdout(
                Signal::from_object_signal(obj, "stdout_emit"),
                config.wasi_stdout_buffer,
            ))?;
        }
        if config.wasi_stderr == PipeBindingType::Instance {
            builder.stderr(WasiContext::make_host_stdout(
                Signal::from_object_signal(obj, "stderr_emit"),
                config.wasi_stderr_buffer,
            ))?;
        }

        match &config.wasi_context {
            Some(ctx) => WasiContext::build_ctx(ctx, &mut builder, &config),
            None => WasiContext::init_ctx_no_context(&mut builder, &config),
        }?;
    }
    let wasi_ctx = builder.build()?;
    let wasi_stdin = wasi_ctx.stdin_provider().map(|v| v.dup());

    #[cfg(feature = "godot-component")]
    let godot_ctx = if use_comp_godot {
        let mut ctx = GodotCtx::new(obj.instance_id());
        ctx.filter = filter;
        Right(ctx)
    } else {
        Left(InnerLock::default())
    };
    let mut store = Store::new(
        comp.engine(),
        StoreData {
            #[cfg(feature = "epoch-timeout")]
            epoch_timeout: if config.with_epoch {
                config.epoch_timeout
            } else {
                0
            },

            #[cfg(feature = "memory-limiter")]
            memory_limits: MemoryLimit::from_config(&config),

            wasi_ctx,
            #[cfg(not(feature = "godot-component"))]
            inner_lock: InnerLock::default(),
            #[cfg(feature = "godot-component")]
            godot_ctx,
        },
    );
    #[cfg(feature = "epoch-timeout")]
    config_store_epoch(&mut store, &config)?;
    #[cfg(feature = "memory-limiter")]
    store.limiter(|data| &mut data.memory_limits);

    let mut linker = <Linker<StoreData>>::new(store.engine());
    site_context!(Command::add_to_linker::<_, HasSelf<WasiCtx>>(
        &mut linker,
        LinkOptions::default()
            .cli_exit_with_code(true)
            .clocks_timezone(true)
            .network_error_code(true),
        |v| &mut v.wasi_ctx,
    ))?;
    #[cfg(feature = "godot-component")]
    if use_comp_godot {
        site_context!(godot_add_to_linker(&mut linker))?;
    }

    let bindings = Command::instantiate(&mut store, &comp, &linker)?;

    Ok(CommandData {
        instance: InstanceData {
            store: Mutex::new(store),
            instance: InstanceType::NoInstance,
            module,

            wasi_stdin,
        },
        bindings,
    })
}

impl WasiCommand {
    #[instrument(level = Level::ERROR)]
    fn emit_error_wrapper(&self, msg: String) {
        let arg = GString::from(&msg);
        drop(msg);
        self.to_gd()
            .emit_signal("error_happened", &[arg.to_variant()]);
    }

    pub fn get_data(&self) -> Result<&CommandData, Error> {
        match self.data.get() {
            Some(data) => Ok(data),
            None => bail_with_site!("Uninitialized instance"),
        }
    }

    #[instrument(level = Level::DEBUG, skip(f))]
    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&CommandData) -> Result<R, Error>,
    {
        match self.get_data().and_then(f) {
            Ok(v) => Some(v),
            Err(e) => {
                let s = format!("{e:?}");
                /*
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    &s,
                );
                */
                godot_error!("{s}");
                self.emit_error_wrapper(s);
                None
            }
        }
    }

    #[instrument(level = Level::DEBUG, skip(config))]
    pub fn initialize_(&self, module: Gd<WasmModule>, config: Option<Variant>) -> bool {
        let t = self.data.get_or_try_init(move || {
            let config = config.and_then(|v| match v.try_to() {
                Ok(v) => Some(v),
                Err(e) => {
                    godot_error!("{}", e);
                    None
                }
            });
            instantiate(&self.to_gd(), config.unwrap_or_default(), module)
        });
        match t {
            Ok(_) => true,
            Err(e) => {
                let s = format!("{e:?}");
                godot_error!("{s}");
                self.emit_error_wrapper(s);
                false
            }
        }
    }
}

#[godot_api]
impl WasiCommand {
    /// Emitted if an error happened. Use it to handle errors.
    #[signal]
    fn error_happened(message: GString);
    /// Emitted whenever WASI stdout is written. Only usable with WASI.
    #[signal]
    fn stdout_emit(message: Variant);
    /// Emitted whenever WASI stderr is written. Only usable with WASI.
    #[signal]
    fn stderr_emit(message: Variant);
    /// Emitted whenever WASI stdin is tried to be read. Only usable with WASI.
    #[signal]
    fn stdin_request();

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[func]
    #[instrument(skip(config), ret)]
    fn initialize(&self, module: Gd<WasmModule>, config: Variant) -> Option<Gd<WasiCommand>> {
        let config = if config.is_nil() { None } else { Some(config) };

        if self.initialize_(module, config) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    #[func]
    #[instrument(ret)]
    fn get_module(&self) -> Option<Gd<WasmModule>> {
        self.unwrap_data(|m| Ok(m.instance.module.clone()))
    }

    #[func]
    #[instrument(ret)]
    fn run(&self) -> bool {
        self.unwrap_data(move |m| {
            m.instance.acquire_store(move |_, mut store| {
                #[cfg(feature = "epoch-timeout")]
                reset_epoch(store.as_context_mut());

                Ok(m.bindings.wasi_cli_run().call_run(store)?.is_ok())
            })
        })
        .unwrap_or_default()
    }

    /// Inserts a line to stdin. Only usable with WASI.
    #[func]
    #[instrument(skip(line), fields(line.len = line.len()))]
    fn stdin_add_line(&self, line: GString) {
        self.unwrap_data(move |m| {
            if let Some(stdin) = &m.instance.wasi_stdin {
                stdin.write(line.to_string().as_bytes());
            }
            Ok(())
        });
    }

    /// Closes stdin. Only usable with WASI.
    #[func]
    #[instrument]
    fn stdin_close(&self) {
        self.unwrap_data(|m| {
            if let Some(stdin) = &m.instance.wasi_stdin {
                stdin.close();
            }
            Ok(())
        });
    }
}
