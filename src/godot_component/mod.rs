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

use crate::godot_util::{PhantomProperty, SendSyncWrapper};
use crate::wasm_config::Config;
use crate::wasm_engine::{WasmModule, ENGINE};
#[cfg(feature = "memory-limiter")]
use crate::wasm_instance::MemoryLimit;
use crate::wasm_instance::{InnerLock, InstanceData, InstanceType};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::config_store_epoch;
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

impl AsMut<GodotCtx> for GodotCtx {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl GodotCtx {
    pub fn new(inst_id: InstanceId) -> Self {
        Self {
            inst_id: Some(inst_id),
            ..Self::default()
        }
    }

    pub fn get_var_borrow(&mut self, res: WasmResource<Variant>) -> AnyResult<Cow<Variant>> {
        let i = res.rep() as usize;
        if res.owned() {
            if let Some(v) = self.table.try_remove(i) {
                return Ok(Cow::Owned(v.into_inner()));
            }
        } else if let Some(v) = self.table.get(i) {
            return Ok(Cow::Borrowed(&**v));
        }

        bail!("index is not valid")
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

    pub fn try_insert(&mut self, var: Variant) -> AnyResult<u32> {
        let entry = self.table.vacant_entry();
        let ret = u32::try_from(entry.key())?;
        entry.insert(SendSyncWrapper::new(var));
        Ok(ret)
    }

    pub fn set_var(&mut self, var: Variant) -> AnyResult<Option<WasmResource<Variant>>> {
        if var.is_nil() {
            Ok(None)
        } else {
            self.try_insert(var).map(|v| Some(WasmResource::new_own(v)))
        }
    }

    pub fn set_into_var<V: ToGodot>(&mut self, var: V) -> AnyResult<WasmResource<Variant>> {
        let v = var.to_variant();
        drop(var);
        self.try_insert(v).map(WasmResource::new_own)
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

impl<T: AsMut<GodotCtx>> bindgen::godot::core::core::HostGodotVar for T {
    fn drop(&mut self, rep: WasmResource<Variant>) -> AnyResult<()> {
        self.as_mut().get_var(rep)?;
        Ok(())
    }

    fn clone(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v = this.get_var(var)?;
        Ok(WasmResource::new_own(this.try_insert(v)?))
    }
}

impl<T: AsMut<GodotCtx>> bindgen::godot::core::core::Host for T {
    fn var_equals(
        &mut self,
        a: WasmResource<Variant>,
        b: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        Ok(this.get_var(a)? == this.get_var(b)?)
    }

    fn var_hash(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        Ok(self.as_mut().get_var(var)?.hash())
    }

    fn var_stringify(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(self.as_mut().get_var(var)?.to_string())
    }
}

impl<T: AsMut<GodotCtx>> bindgen::godot::reflection::this::Host for T {
    fn get_this(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let Some(id) = this.inst_id else {
            bail_with_site!("Self instance ID is not set")
        };

        this.set_into_var(<Gd<Object>>::try_from_instance_id(id)?)
    }
}

pub fn add_to_linker<T: AsMut<GodotCtx>>(linker: &mut Linker<T>) -> AnyResult<()> {
    bindgen::godot::core::core::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::typeis::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::primitive::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::byte_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::int32_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::int64_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::float32_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::float64_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::vector2_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::vector3_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::color_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::string_array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::array::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::dictionary::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::object::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::callable::add_to_linker(&mut *linker, |v| v)?;
    bindgen::godot::core::signal::add_to_linker(&mut *linker, |v| v)?;

    bindgen::godot::reflection::this::add_to_linker(&mut *linker, |v| v)
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
        site_context!(add_to_linker(&mut linker))?;

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

                let res = store.data_mut().godot_ctx.set_into_var(args)?;
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
