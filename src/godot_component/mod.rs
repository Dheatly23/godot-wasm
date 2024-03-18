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
use slab::Slab;
use wasmtime::component::{Linker, Resource as WasmResource};

use crate::wasm_util::SendSyncWrapper;

fn wrap_error(e: Error) -> AnyResult<()> {
    if e == Error::OK {
        Ok(())
    } else {
        bail!("{e:?}")
    }
}

pub struct GodotCtx {
    table: Slab<SendSyncWrapper<Variant>>,
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
        path: "wit/imports/core",
        world: "godot:core/imports",
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

pub fn add_to_linker<T, U>(
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
    bindgen::godot::core::signal::add_to_linker(&mut *linker, get)
}
