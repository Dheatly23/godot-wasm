mod array;
mod callable;
mod dictionary;
mod object;
mod packed_array;
mod primitive;
mod signal;
mod typeis;

use std::borrow::Cow;

use anyhow::{bail, Result as AnyResult};
use godot::engine::global::Error;
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use slab::Slab;
use wasmtime::component::{Linker, Resource as WasmResource};
use wasmtime::Store;

use crate::wasm_config::Config;
use crate::wasm_engine::{WasmModule, ENGINE};
#[cfg(feature = "memory-limiter")]
use crate::wasm_instance::MemoryLimit;
use crate::wasm_instance::{InnerLock, InstanceData, InstanceType};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::config_store_epoch;
use crate::wasm_util::{PhantomProperty, SendSyncWrapper};
use crate::{bail_with_site, site_context};

fn wrap_error(e: Error) -> AnyResult<()> {
    if e == Error::OK {
        Ok(())
    } else {
        bail!("{e:?}")
    }
}

#[derive(Default)]
pub struct GodotCtx {
    table: Slab<SendSyncWrapper<Variant>>,
    inst_id: Option<InstanceId>,
}

impl GodotCtx {
    pub fn get_var_borrow(&mut self, res: WasmResource<Variant>) -> AnyResult<Cow<Variant>> {
        if res.owned() {
            Ok(Cow::Owned(self.table.remove(res.rep() as _).into_inner()))
        } else if let Some(v) = self.table.get(res.rep() as _) {
            Ok(Cow::Borrowed(&**v))
        } else {
            bail!("index is not valid")
        }
    }

    pub fn get_var(&mut self, res: WasmResource<Variant>) -> AnyResult<Variant> {
        self.get_var_borrow(res).map(|v| v.into_owned())
    }

    pub fn maybe_get_var_borrow(
        &mut self,
        res: Option<WasmResource<Variant>>,
    ) -> AnyResult<Cow<Variant>> {
        match res {
            None => Ok(Cow::Owned(Variant::nil())),
            Some(res) => self.get_var_borrow(res),
        }
    }

    pub fn maybe_get_var(&mut self, res: Option<WasmResource<Variant>>) -> AnyResult<Variant> {
        match res {
            None => Ok(Variant::nil()),
            Some(res) => self.get_var(res),
        }
    }

    pub fn set_var(&mut self, var: Variant) -> Option<WasmResource<Variant>> {
        if var.is_nil() {
            None
        } else {
            Some(WasmResource::new_own(
                self.table.insert(SendSyncWrapper::new(var)) as _,
            ))
        }
    }

    pub fn set_into_var<V: ToGodot>(&mut self, var: &V) -> WasmResource<Variant> {
        WasmResource::new_own(self.table.insert(SendSyncWrapper::new(var.to_variant())) as _)
    }
}

#[allow(dead_code)]
pub type GVar = Variant;

pub mod bindgen {
    use wasmtime::component::bindgen;

    pub use super::GVar;

    bindgen!({
        path: "wit",
        world: "godot-wasm:script/script",
        ownership: Borrowing {
            duplicate_if_necessary: true
        },
        with: {
            "godot:core/core/godot-var": GVar,
        },
    });
}

impl bindgen::godot::core::core::HostGodotVar for GodotCtx {
    fn drop(&mut self, rep: WasmResource<Variant>) -> AnyResult<()> {
        self.get_var(rep)?;
        Ok(())
    }

    fn clone(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let v = self.get_var(var)?;
        Ok(WasmResource::new_own(
            self.table.insert(SendSyncWrapper::new(v)) as _,
        ))
    }
}

impl bindgen::godot::core::core::Host for GodotCtx {
    fn var_equals(
        &mut self,
        a: WasmResource<Variant>,
        b: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        Ok(self.get_var(a)? == self.get_var(b)?)
    }

    fn var_hash(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        Ok(self.get_var(var)?.hash())
    }

    fn var_stringify(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(self.get_var(var)?.to_string())
    }
}

impl bindgen::godot::reflection::this::Host for GodotCtx {
    fn get_this(&mut self) -> AnyResult<WasmResource<Variant>> {
        let Some(id) = self.inst_id else {
            bail_with_site!("Self instance ID is not set")
        };

        Ok(self.set_into_var(&<Gd<Object>>::try_from_instance_id(id)?))
    }
}

pub fn add_to_linker<T>(
    linker: &mut Linker<T>,
    get: impl Fn(&mut T) -> &mut GodotCtx + Send + Sync + Copy + 'static,
) -> AnyResult<()> {
    bindgen::godot::core::core::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::typeis::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::primitive::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::byte_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::int32_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::int64_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::float32_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::float64_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::vector2_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::vector3_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::color_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::string_array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::array::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::dictionary::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::object::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::callable::add_to_linker(&mut *linker, get)?;
    bindgen::godot::core::signal::add_to_linker(&mut *linker, get)?;

    bindgen::godot::reflection::this::add_to_linker(&mut *linker, get)
}

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
    bindings: bindgen::Script,
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

impl WasmScriptLike {
    fn instantiate(config: Config, module: Gd<WasmModule>) -> AnyResult<WasmScriptLikeData> {
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

                godot_ctx: GodotCtx::default(),
            },
        );
        #[cfg(feature = "epoch-timeout")]
        config_store_epoch(&mut store, &config);
        #[cfg(feature = "memory-limiter")]
        store.limiter(|data| &mut data.memory_limits);

        let mut linker = <Linker<WasmScriptLikeStore>>::new(&ENGINE);
        site_context!(add_to_linker(&mut linker, |v| &mut v.godot_ctx))?;

        let (bindings, instance) =
            site_context!(bindgen::Script::instantiate(&mut store, &comp, &linker))?;

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
            Self::instantiate(
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
    fn call_wasm(&self, args: Array<Variant>) -> Variant {
        self.unwrap_data(move |m| {
            m.instance.acquire_store(move |_, mut store| {
                #[cfg(feature = "epoch-timeout")]
                if let v @ 1.. = store.data().epoch_timeout {
                    store.set_epoch_deadline(v);
                }

                let res = store.data_mut().godot_ctx.set_into_var(&args);
                drop(args);

                let ret = m
                    .bindings
                    .call_call(&mut store, WasmResource::new_borrow(res.rep()));
                let ctx = &mut store.data_mut().godot_ctx;
                ctx.get_var(res)?;

                site_context!(ctx.maybe_get_var(ret?))
            })
        })
        .unwrap_or_else(Variant::nil)
    }
}
