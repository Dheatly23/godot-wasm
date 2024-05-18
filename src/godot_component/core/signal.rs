use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
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
        let o: Gd<Object> = this.get_value(obj)?;
        let s: StringName = this.get_value(signal)?;
        this.set_into_var(Signal::from_object_signal(&o, s))
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "object"))?;
        let v: Signal = this.get_value(var)?;
        match v.object() {
            Some(v) => this.set_into_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn name(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "name"))?;
        let v: Signal = this.get_value(var)?;
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
        let v: Signal = this.get_value(var)?;
        wrap_error(v.connect(this.get_value(callable)?, flags as _))
    }

    fn disconnect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "disconnect"))?;
        let v: Signal = this.get_value(var)?;
        v.disconnect(this.get_value(callable)?);
        Ok(())
    }

    fn is_connected(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "is-connected"))?;
        let v: Signal = this.get_value(var)?;
        Ok(v.is_connected(this.get_value(callable)?))
    }

    fn emit(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "signal", "emit"))?;
        let v: Signal = this.get_value(var)?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        v.emit(&args);
        Ok(())
    }
}
