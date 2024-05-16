use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::godot_util::from_var_any;
use crate::site_context;

impl<T: AsMut<GodotCtx>> bindgen::godot::core::signal::Host for T {
    fn from_object_signal(
        &mut self,
        obj: WasmResource<Variant>,
        signal: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:core", "signal", "from-object-signal"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(obj)?)?;
        let s: StringName = from_var_any(this.get_var_borrow(signal)?)?;
        this.set_into_var(Signal::from_object_signal(&o, s))
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "object"))?;
        let v: Signal = from_var_any(this.get_var_borrow(var)?)?;
        match v.object() {
            Some(v) => this.set_into_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn name(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "name"))?;
        let v: Signal = from_var_any(this.get_var_borrow(var)?)?;
        this.set_into_var(v.name())
    }

    fn connect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
        flags: u32,
    ) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "connect"))?;
        let v: Signal = from_var_any(this.get_var_borrow(var)?)?;
        wrap_error(v.connect(from_var_any(this.get_var_borrow(callable)?)?, flags as _))
    }

    fn disconnect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "disconnect"))?;
        let v: Signal = from_var_any(this.get_var_borrow(var)?)?;
        v.disconnect(from_var_any(this.get_var_borrow(callable)?)?);
        Ok(())
    }

    fn is_connected(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "is-connected"))?;
        let v: Signal = from_var_any(this.get_var_borrow(var)?)?;
        Ok(v.is_connected(from_var_any(this.get_var_borrow(callable)?)?))
    }

    fn emit(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "emit"))?;
        let v: Signal = from_var_any(this.get_var_borrow(var)?)?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        v.emit(&args);
        Ok(())
    }
}
