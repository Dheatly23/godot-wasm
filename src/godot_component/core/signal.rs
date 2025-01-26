use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::wasm_util::get_godot_param_cache;

filter_macro! {method [
    from_object_signal -> "from-object-signal",
    object -> "object",
    name -> "name",
    connect -> "connect",
    disconnect -> "disconnect",
    is_connected -> "is-connected",
    emit -> "emit",
]}

impl bindgen::godot::core::signal::Host for GodotCtx {
    fn from_object_signal(
        &mut self,
        obj: WasmResource<Variant>,
        signal: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, signal, from_object_signal)?;
        let o: Gd<Object> = self.get_value(obj)?;
        let s: StringName = self.get_value(signal)?;
        self.set_into_var(Signal::from_object_signal(&o, &s))
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, signal, object)?;
        let v: Signal = self.get_value(var)?;
        match v.object() {
            Some(v) => self.set_into_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn name(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, signal, name)?;
        let v: Signal = self.get_value(var)?;
        self.set_into_var(v.name())
    }

    fn connect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
        flags: u32,
    ) -> ErrorRes {
        filter_macro!(filter self.filter.as_ref(), godot_core, signal, connect)?;
        let v: Signal = self.get_value(var)?;
        wrap_error(v.connect(&self.get_value(callable)?, flags as _))
    }

    fn disconnect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, signal, disconnect)?;
        let v: Signal = self.get_value(var)?;
        v.disconnect(&self.get_value(callable)?);
        Ok(())
    }

    fn is_connected(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, signal, is_connected)?;
        let v: Signal = self.get_value(var)?;
        Ok(v.is_connected(&self.get_value(callable)?))
    }

    fn emit(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, signal, emit)?;
        let v: Signal = self.get_value(var)?;
        let mut a = get_godot_param_cache(args.len());
        for (i, v) in args.into_iter().enumerate() {
            a[i] = self.maybe_get_var(v)?;
        }
        self.release_store(move || v.emit(&a));
        Ok(())
    }
}
