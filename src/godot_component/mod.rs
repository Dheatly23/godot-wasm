mod array;
mod packed_array;
mod primitive;
mod typeis;

use std::borrow::Cow;

use anyhow::Result as AnyResult;
use godot::prelude::*;
use slab::Slab;
use wasmtime::component::Resource as WasmResource;

use crate::wasm_util::SendSyncWrapper;

pub struct GodotCtx {
    table: Slab<SendSyncWrapper<Variant>>,
}

impl GodotCtx {
    pub fn get_var_borrow(&mut self, res: WasmResource<Variant>) -> Cow<Variant> {
        if res.owned() {
            Cow::Owned(self.table.remove(res.rep() as _).into_inner())
        } else {
            Cow::Borrowed(&**self.table.get(res.rep() as _).expect("index must be valid"))
        }
    }

    pub fn get_var(&mut self, res: WasmResource<Variant>) -> Variant {
        self.get_var_borrow(res).into_owned()
    }

    pub fn maybe_get_var_borrow(&mut self, res: Option<WasmResource<Variant>>) -> Cow<Variant> {
        match res {
            None => Cow::Owned(Variant::nil()),
            Some(res) => self.get_var_borrow(res),
        }
    }

    pub fn maybe_get_var(&mut self, res: Option<WasmResource<Variant>>) -> Variant {
        match res {
            None => Variant::nil(),
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
        world: "godot:core/godot-core",
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
        self.get_var(rep);
        Ok(())
    }
}

impl bindgen::godot::core::core::Host for GodotCtx {
    fn var_equals(
        &mut self,
        a: WasmResource<Variant>,
        b: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        Ok(self.get_var(a) == self.get_var(b))
    }

    fn var_hash(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        Ok(self.get_var(var).hash())
    }

    fn var_stringify(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(self.get_var(var).to_string())
    }
}
