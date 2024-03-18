use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use super::wrap_error;

impl crate::godot_component::bindgen::godot::core::signal::Host
    for crate::godot_component::GodotCtx
{
    fn from_object_signal(
        &mut self,
        obj: WasmResource<Variant>,
        signal: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let o: Gd<Object> = self.get_var_borrow(obj)?.try_to()?;
        let s: StringName = self.get_var_borrow(signal)?.try_to()?;
        Ok(self.set_into_var(&Signal::from_object_signal(&o, s)))
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let v: Signal = self.get_var_borrow(var)?.try_to()?;
        Ok(v.object().map(|v| self.set_into_var(&v)))
    }

    fn name(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let v: Signal = self.get_var_borrow(var)?.try_to()?;
        Ok(self.set_into_var(&v.name()))
    }

    fn connect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
        flags: u32,
    ) -> AnyResult<()> {
        let v: Signal = self.get_var_borrow(var)?.try_to()?;
        wrap_error(v.connect(self.get_var_borrow(callable)?.try_to()?, flags as _))
    }

    fn disconnect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let v: Signal = self.get_var_borrow(var)?.try_to()?;
        v.disconnect(self.get_var_borrow(callable)?.try_to()?);
        Ok(())
    }

    fn is_connected(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let v: Signal = self.get_var_borrow(var)?.try_to()?;
        Ok(v.is_connected(self.get_var_borrow(callable)?.try_to()?))
    }

    fn emit(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        let v: Signal = self.get_var_borrow(var)?.try_to()?;
        let args = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        v.emit(&args);
        Ok(())
    }
}
