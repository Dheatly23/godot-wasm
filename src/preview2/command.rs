use anyhow::Error;
use cfg_if::cfg_if;
use either::{Either, Left, Right};
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use wasmtime::component::{Linker, ResourceTable};
use wasmtime::Store;
use wasmtime_wasi::bindings::sync::Command;
use wasmtime_wasi::{add_to_linker_sync, WasiCtx, WasiCtxBuilder, WasiView};

#[cfg(feature = "godot-component")]
use crate::godot_component::filter::Filter;
#[cfg(feature = "godot-component")]
use crate::godot_component::{add_to_linker as godot_add_to_linker, GodotCtx};
use crate::wasi_ctx::WasiContext;
use crate::wasm_config::Config;
use crate::wasm_engine::WasmModule;
#[cfg(feature = "memory-limiter")]
use crate::wasm_instance::MemoryLimit;
use crate::wasm_instance::{InnerLock, InstanceData, InstanceType};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::config_store_epoch;
use crate::{bail_with_site, site_context};

#[derive(Default)]
struct CommandConfig {
    config: Config,

    #[cfg(feature = "godot-component")]
    use_comp_godot: bool,
    #[cfg(feature = "godot-component")]
    filter: Filter,
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
        #[cfg(feature = "godot-component")]
        let use_comp_godot = via
            .get("component.godot.enable")
            .map(|v| v.try_to())
            .transpose()?
            .unwrap_or_default();
        #[cfg(feature = "godot-component")]
        let filter = via
            .get("component.godot.filter")
            .map(|v| v.try_to())
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            config: Config::try_from_godot(via)?,
            #[cfg(feature = "godot-component")]
            use_comp_godot,
            #[cfg(feature = "godot-component")]
            filter,
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

pub struct CommandData {
    instance: InstanceData<StoreData>,
    bindings: Command,
}

pub struct StoreData {
    #[cfg(feature = "epoch-timeout")]
    epoch_timeout: u64,

    #[cfg(feature = "memory-limiter")]
    memory_limits: MemoryLimit,

    table: ResourceTable,
    wasi_ctx: WasiCtx,
    #[cfg(not(feature = "godot-component"))]
    inner_lock: InnerLock,
    #[cfg(feature = "godot-component")]
    godot_ctx: Either<InnerLock, GodotCtx>,
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

impl WasiView for StoreData {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }
}

fn instantiate(
    _inst_id: InstanceId,
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
    let comp = site_context!(module.bind().get_data()?.module.get_component())?.clone();

    let mut builder = WasiCtxBuilder::new();
    if let Config {
        with_wasi: true,
        wasi_context: Some(ctx),
        ..
    } = &config
    {
        WasiContext::build_ctx(ctx.clone(), &mut builder, &config)
    } else {
        builder.inherit_stdout().inherit_stderr();
        WasiContext::init_ctx_no_context(&mut builder, &config)
    }?;
    let wasi_ctx = builder.build();

    #[cfg(feature = "godot-component")]
    let godot_ctx = if use_comp_godot {
        let mut ctx = GodotCtx::new(_inst_id);
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

            table: ResourceTable::new(),
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
    add_to_linker_sync(&mut linker)?;
    #[cfg(feature = "godot-component")]
    godot_add_to_linker(&mut linker, |v| {
        v.godot_ctx
            .as_mut()
            .right()
            .expect("Godot component is enabled, but no context is provided")
    })?;

    let bindings = Command::instantiate(&mut store, &comp, &linker)?;

    Ok(CommandData {
        instance: InstanceData {
            store: Mutex::new(store),
            instance: InstanceType::NoInstance,
            module,

            wasi_stdin: None,
        },
        bindings,
    })
}

impl WasiCommand {
    fn emit_error_wrapper(&self, msg: String) {
        self.to_gd().emit_signal(
            &StringName::from(c"error_happened"),
            &[GString::from(msg).to_variant()],
        );
    }

    pub fn get_data(&self) -> Result<&CommandData, Error> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized instance")
        }
    }

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

    pub fn initialize_(&self, module: Gd<WasmModule>, config: Option<Variant>) -> bool {
        let t = self.data.get_or_try_init(move || {
            let config = config.and_then(|v| match v.try_to() {
                Ok(v) => Some(v),
                Err(e) => {
                    godot_error!("{}", e);
                    None
                }
            });
            instantiate(
                self.base().instance_id(),
                config.unwrap_or_default(),
                module,
            )
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
    #[signal]
    fn error_happened(message: GString);

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[func]
    fn initialize(&self, module: Gd<WasmModule>, config: Variant) -> Option<Gd<WasiCommand>> {
        let config = if config.is_nil() { None } else { Some(config) };

        if self.initialize_(module, config) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    #[func]
    fn get_module(&self) -> Option<Gd<WasmModule>> {
        self.unwrap_data(|m| Ok(m.instance.module.clone()))
    }

    #[func]
    fn run(&self) -> bool {
        self.unwrap_data(move |m| {
            m.instance.acquire_store(move |_, mut store| {
                #[cfg(feature = "epoch-timeout")]
                if let v @ 1.. = store.data().epoch_timeout {
                    store.set_epoch_deadline(v);
                }

                Ok(m.bindings.wasi_cli_run().call_run(&mut store)?.is_ok())
            })
        })
        .unwrap_or_default()
    }
}
