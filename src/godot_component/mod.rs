mod classes;
mod core;
mod global;

use std::borrow::Cow;

use anyhow::{bail, Result as AnyResult};
use godot::engine::global::Error;
use godot::prelude::*;
use slab::Slab;
use wasmtime::component::{Linker, Resource as WasmResource};

use crate::bail_with_site;
use crate::godot_util::SendSyncWrapper;

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
    pub inst_id: Option<InstanceId>,
    pub allow_unsafe_behavior: bool,
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
    pub use super::GVar;

    wasmtime::component::bindgen!({
        path: "wit",
        interfaces: "
            include godot:core/imports@0.1.0;
            include godot:reflection/imports@0.1.0;
            include godot:global/imports@0.1.0;
        ",
        tracing: false,
        async: false,
        ownership: Borrowing {
            duplicate_if_necessary: false
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

        this.set_into_var(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased())?)
    }
}

pub fn add_to_linker<T, U: AsMut<GodotCtx>>(
    linker: &mut Linker<T>,
    f: impl Fn(&mut T) -> &mut U + Send + Sync + Copy + 'static,
) -> AnyResult<()> {
    bindgen::godot::core::core::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::typeis::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::primitive::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::byte_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::int32_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::int64_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::float32_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::float64_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::vector2_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::vector3_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::color_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::string_array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::array::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::dictionary::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::object::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::callable::add_to_linker(&mut *linker, f)?;
    bindgen::godot::core::signal::add_to_linker(&mut *linker, f)?;

    bindgen::godot::reflection::this::add_to_linker(&mut *linker, f)
}
