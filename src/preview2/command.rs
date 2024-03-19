use anyhow::Error;
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use wasmtime::component::{Linker, ResourceTable};
use wasmtime::Store;
use wasmtime_wasi::preview2::command::sync::{add_to_linker, Command};
use wasmtime_wasi::preview2::{WasiCtx, WasiCtxBuilder, WasiView};

#[cfg(feature = "godot-component")]
use crate::godot_component::{add_to_linker as godot_add_to_linker, GodotCtx};
use crate::wasi_ctx::WasiContext;
use crate::wasm_config::Config;
use crate::wasm_engine::{WasmModule, ENGINE};
#[cfg(feature = "memory-limiter")]
use crate::wasm_instance::MemoryLimit;
use crate::wasm_instance::{InnerLock, InstanceData, InstanceType};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::config_store_epoch;
use crate::{bail_with_site, site_context};

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
    inner_lock: InnerLock,

    #[cfg(feature = "epoch-timeout")]
    epoch_timeout: u64,

    #[cfg(feature = "memory-limiter")]
    memory_limits: MemoryLimit,

    table: ResourceTable,
    wasi_ctx: WasiCtx,
    godot_ctx: GodotCtx,
}

impl AsRef<InnerLock> for StoreData {
    fn as_ref(&self) -> &InnerLock {
        &self.inner_lock
    }
}

impl AsMut<InnerLock> for StoreData {
    fn as_mut(&mut self) -> &mut InnerLock {
        &mut self.inner_lock
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

fn instantiate(config: Config, module: Gd<WasmModule>) -> Result<CommandData, Error> {
    let comp = site_context!(module.bind().get_data()?.module.get_component())?.clone();

    let wasi_ctx = if let Config {
        with_wasi: true,
        wasi_context: Some(ctx),
        ..
    } = &config
    {
        WasiContext::build_ctx_preview_2(ctx.clone(), WasiCtxBuilder::new(), &config)?
    } else {
        let mut ctx = WasiCtxBuilder::new();
        WasiContext::init_ctx_no_context_preview_2(ctx.inherit_stdout().inherit_stderr(), &config)?;
        ctx.build()
    };

    let mut store = Store::new(
        &ENGINE,
        StoreData {
            inner_lock: InnerLock::default(),

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
            godot_ctx: GodotCtx::default(),
        },
    );
    #[cfg(feature = "epoch-timeout")]
    config_store_epoch(&mut store, &config);
    #[cfg(feature = "memory-limiter")]
    store.limiter(|data| &mut data.memory_limits);

    let mut linker = <Linker<StoreData>>::new(&ENGINE);
    add_to_linker(&mut linker)?;
    godot_add_to_linker(&mut linker, |v| &mut v.godot_ctx)?;

    let (bindings, instance) = Command::instantiate(&mut store, &comp, &linker)?;

    Ok(CommandData {
        instance: InstanceData {
            store: Mutex::new(store),
            instance: InstanceType::Component(instance),
            module,

            wasi_stdin: None,
        },
        bindings,
    })
}

impl WasiCommand {
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
                /*
                let s = format!("{:?}", e);
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    &s,
                );
                */
                godot_error!("{:?}", e);
                /*
                self.base.emit_signal(
                    StringName::from("error_happened"),
                    &[format!("{}", e).to_variant()],
                );
                */
                None
            }
        }
    }

    pub fn initialize_(&self, module: Gd<WasmModule>, config: Option<Variant>) -> bool {
        match self.data.get_or_try_init(move || {
            instantiate(
                match config {
                    Some(v) => match Config::try_from_variant(&v) {
                        Ok(v) => v,
                        Err(e) => {
                            godot_error!("{}", e);
                            Config::default()
                        }
                    },
                    None => Config::default(),
                },
                module,
            )
        }) {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }
}

#[godot_api]
impl WasiCommand {
    #[signal]
    fn error_happened();

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
