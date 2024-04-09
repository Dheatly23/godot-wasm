use anyhow::Result as AnyResult;
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use wasmtime::component::{bindgen, Linker, Resource as WasmResource};
use wasmtime::Store;

use crate::godot_component::bindgen::godot as g_;
use crate::godot_component::{add_to_linker, GodotCtx};
use crate::godot_util::PhantomProperty;
use crate::wasm_config::Config;
use crate::wasm_engine::{WasmModule, ENGINE};
#[cfg(feature = "memory-limiter")]
use crate::wasm_instance::MemoryLimit;
use crate::wasm_instance::{InnerLock, InstanceData, InstanceType};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::config_store_epoch;
use crate::{bail_with_site, site_context};

bindgen!({
    path: "wit",
    world: "godot-wasm:script/script",
    tracing: false,
    async: false,
    ownership: Borrowing {
        duplicate_if_necessary: false
    },
    with: {
        "godot:core/core": g_::core::core,
        "godot:core/typeis": g_::core::typeis,
        "godot:core/primitive": g_::core::primitive,
        "godot:core/array": g_::core::array,
        "godot:core/dictionary": g_::core::dictionary,
        "godot:core/byte-array": g_::core::byte_array,
        "godot:core/int32-array": g_::core::int32_array,
        "godot:core/int64-array": g_::core::int64_array,
        "godot:core/float32-array": g_::core::float32_array,
        "godot:core/float64-array": g_::core::float64_array,
        "godot:core/color-array": g_::core::color_array,
        "godot:core/vector2-array": g_::core::vector2_array,
        "godot:core/vector3-array": g_::core::vector3_array,
        "godot:core/string-array": g_::core::string_array,
        "godot:core/object": g_::core::object,
        "godot:core/callable": g_::core::callable,
        "godot:core/signal": g_::core::signal,
        "godot:reflection/this": g_::reflection::this,
    },
});

#[derive(GodotClass)]
#[class(base=RefCounted, init, tool)]
pub struct WasmScriptLike {
    base: Base<RefCounted>,
    data: OnceCell<WasmScriptLikeData>,

    #[var(get = get_module)]
    #[allow(dead_code)]
    module: PhantomProperty<Option<Gd<WasmModule>>>,
}

pub struct WasmScriptLikeData {
    instance: InstanceData<WasmScriptLikeStore>,
    bindings: Script,
}

pub struct WasmScriptLikeStore {
    inner_lock: InnerLock,

    #[cfg(feature = "epoch-timeout")]
    epoch_timeout: u64,

    #[cfg(feature = "memory-limiter")]
    memory_limits: MemoryLimit,

    godot_ctx: GodotCtx,
}

impl AsRef<InnerLock> for WasmScriptLikeStore {
    fn as_ref(&self) -> &InnerLock {
        &self.inner_lock
    }
}

impl AsMut<InnerLock> for WasmScriptLikeStore {
    fn as_mut(&mut self) -> &mut InnerLock {
        &mut self.inner_lock
    }
}

impl AsMut<GodotCtx> for WasmScriptLikeStore {
    fn as_mut(&mut self) -> &mut GodotCtx {
        &mut self.godot_ctx
    }
}

impl WasmScriptLike {
    fn instantiate(
        inst_id: InstanceId,
        config: Config,
        module: Gd<WasmModule>,
    ) -> AnyResult<WasmScriptLikeData> {
        let comp = site_context!(module.bind().get_data()?.module.get_component())?.clone();

        let mut store = Store::new(
            &ENGINE,
            WasmScriptLikeStore {
                inner_lock: InnerLock::default(),

                #[cfg(feature = "epoch-timeout")]
                epoch_timeout: if config.with_epoch {
                    config.epoch_timeout
                } else {
                    0
                },

                #[cfg(feature = "memory-limiter")]
                memory_limits: MemoryLimit::from_config(&config),

                godot_ctx: GodotCtx::new(inst_id),
            },
        );
        #[cfg(feature = "epoch-timeout")]
        config_store_epoch(&mut store, &config);
        #[cfg(feature = "memory-limiter")]
        store.limiter(|data| &mut data.memory_limits);

        let mut linker = <Linker<WasmScriptLikeStore>>::new(&ENGINE);
        site_context!(add_to_linker(&mut linker, |v| v))?;

        let (bindings, instance) = site_context!(Script::instantiate(&mut store, &comp, &linker))?;

        Ok(WasmScriptLikeData {
            instance: InstanceData {
                store: Mutex::new(store),
                instance: InstanceType::Component(instance),
                module,

                wasi_stdin: None,
            },
            bindings,
        })
    }

    pub fn get_data(&self) -> AnyResult<&WasmScriptLikeData> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized instance")
        }
    }

    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&WasmScriptLikeData) -> AnyResult<R>,
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
                self.base().clone().emit_signal(
                    StringName::from(c"error_happened"),
                    &[GString::from(s).to_variant()],
                );
                None
            }
        }
    }

    pub fn initialize_(&self, module: Gd<WasmModule>, config: Option<Variant>) -> bool {
        match self.data.get_or_try_init(move || {
            Self::instantiate(
                self.base().instance_id(),
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
                let s = format!("{e:?}");
                godot_error!("{s}");
                self.base().clone().emit_signal(
                    StringName::from(c"error_happened"),
                    &[GString::from(s).to_variant()],
                );
                false
            }
        }
    }
}

#[godot_api]
impl WasmScriptLike {
    #[signal]
    fn error_happened();

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[func]
    fn initialize(&self, module: Gd<WasmModule>, config: Variant) -> Option<Gd<WasmScriptLike>> {
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
    fn call_wasm(&self, args: VariantArray) -> Variant {
        self.unwrap_data(move |m| {
            m.instance.acquire_store(move |_, mut store| {
                #[cfg(feature = "epoch-timeout")]
                if let v @ 1.. = store.data().epoch_timeout {
                    store.set_epoch_deadline(v);
                }

                let res = store.data_mut().godot_ctx.set_into_var(args)?;
                let ret = m
                    .bindings
                    .call_call(&mut store, WasmResource::new_borrow(res.rep()));
                let ctx = &mut store.data_mut().godot_ctx;
                ctx.get_var(res)?;

                site_context!(ctx.maybe_get_var(ret?))
            })
        })
        .unwrap_or_default()
    }
}
