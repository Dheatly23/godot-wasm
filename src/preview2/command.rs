use std::mem::transmute;

use anyhow::Error;
use gdnative::export::user_data::Map;
use gdnative::log::{error, godot_site, Site};
use gdnative::prelude::*;
use parking_lot::{Mutex, Once, OnceState};
use wasmtime::component::Linker;
use wasmtime::Store;
use wasmtime_wasi::preview2::command::sync::{add_to_linker, Command};
use wasmtime_wasi::preview2::{Table, WasiCtxBuilder};

use crate::wasi_ctx::WasiContext;
use crate::wasm_config::Config;
use crate::wasm_engine::{WasmModule, ENGINE};
use crate::wasm_instance::{InstanceData, InstanceType, MaybeWasi, StoreData};
use crate::wasm_util::config_store_common;
use crate::{bail_with_site, site_context};

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::ArcData<WasiCommand>)]
pub struct WasiCommand {
    once: Once,
    data: Option<CommandData>,
}

pub struct CommandData {
    instance: InstanceData<StoreData>,
    bindings: Command,
}

fn instantiate(config: Config, module: Instance<WasmModule, Shared>) -> Result<CommandData, Error> {
    let comp = module
        .script()
        .map(|m| {
            m.get_data()
                .and_then(|m| site_context!(m.module.get_component()))
                .map(|v| v.clone())
        })
        .unwrap()?;

    let mut store = Store::new(&ENGINE, StoreData::new(config));
    config_store_common(&mut store)?;

    let config = &store.data().config;
    let ctx = if let Config {
        with_wasi: true,
        wasi_context: Some(ctx),
        ..
    } = config
    {
        WasiContext::build_ctx_preview_2(ctx.clone(), WasiCtxBuilder::new(), config)?
    } else {
        let mut ctx = WasiCtxBuilder::new();
        WasiContext::init_ctx_no_context_preview_2(ctx.inherit_stdout().inherit_stderr(), config)?;
        ctx.build()
    };
    store.data_mut().wasi_ctx = MaybeWasi::Preview2(ctx, Table::new());

    let mut linker = <Linker<StoreData>>::new(&ENGINE);
    add_to_linker(&mut linker)?;

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
    fn new(_owner: &Reference) -> Self {
        Self {
            once: Once::new(),
            data: None,
        }
    }

    pub fn get_data(&self) -> Result<&CommandData, Error> {
        if let OnceState::Done = self.once.state() {
            Ok(self.data.as_ref().unwrap())
        } else {
            bail_with_site!("Uninitialized module")
        }
    }

    pub fn unwrap_data<F, R>(&self, base: TRef<Reference>, f: F) -> Option<R>
    where
        F: FnOnce(&CommandData) -> Result<R, Error>,
    {
        match self.get_data().and_then(f) {
            Ok(v) => Some(v),
            Err(e) => {
                let s = format!("{:?}", e);
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    &s,
                );
                base.emit_signal("error_happened", &[s.owned_to_variant()]);
                None
            }
        }
    }

    pub fn initialize_(
        &self,
        _owner: &Reference,
        module: Instance<WasmModule, Shared>,
        config: Option<Variant>,
    ) -> bool {
        let mut r = true;
        let ret = &mut r;

        self.once.call_once(move || {
            match instantiate(
                match config {
                    Some(v) => match Config::from_variant(&v) {
                        Ok(v) => v,
                        Err(e) => {
                            godot_error!("{}", e);
                            Config::default()
                        }
                    },
                    None => Config::default(),
                },
                module,
            ) {
                Ok(v) => {
                    // SAFETY: Should be called only once and nobody else can read module data
                    #[allow(mutable_transmutes)]
                    let data = unsafe {
                        transmute::<&Option<CommandData>, &mut Option<CommandData>>(&self.data)
                    };
                    *data = Some(v);
                }
                Err(e) => {
                    godot_error!("{}", e);
                    *ret = false;
                }
            }
        });

        r
    }
}

#[methods]
impl WasiCommand {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .property::<Option<Instance<WasmModule, Shared>>>("module")
            .with_getter(|v, b| v.unwrap_data(b, |m| Ok(m.instance.module.clone())))
            .done();

        builder
            .signal("error_happened")
            .with_param("message", VariantType::GodotString)
            .done();

        builder
            .signal("stdout_emit")
            .with_param("message", VariantType::GodotString)
            .done();

        builder
            .signal("stderr_emit")
            .with_param("message", VariantType::GodotString)
            .done();

        builder.signal("stdin_request").done();
    }

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[method]
    fn initialize(
        &self,
        #[base] owner: TRef<Reference>,
        module: Instance<WasmModule, Shared>,
        #[opt] config: Option<Variant>,
    ) -> Option<Ref<Reference>> {
        if self.initialize_(owner.as_ref(), module, config) {
            Some(owner.claim())
        } else {
            None
        }
    }

    #[method]
    fn run(&self, #[base] base: TRef<Reference>) -> bool {
        self.unwrap_data(base, move |m| {
            m.instance.acquire_store(move |_, mut store| {
                Ok(m.bindings.wasi_cli_run().call_run(&mut store)?.is_ok())
            })
        })
        .unwrap_or_default()
    }
}
