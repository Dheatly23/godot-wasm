use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use super::wrap_error;

impl<T: AsMut<crate::godot_component::GodotCtx>>
    crate::godot_component::bindgen::godot::core::signal::Host for T
{
    fn from_object_signal(
        &mut self,
        obj: WasmResource<Variant>,
        signal: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_var_borrow(obj)?.try_to()?;
        let s: StringName = this.get_var_borrow(signal)?.try_to()?;
        this.set_into_var(Signal::from_object_signal(&o, s))
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let v: Signal = this.get_var_borrow(var)?.try_to()?;
        match v.object() {
            Some(v) => this.set_into_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn name(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v: Signal = this.get_var_borrow(var)?.try_to()?;
        this.set_into_var(v.name())
    }

    fn connect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
        flags: u32,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let v: Signal = this.get_var_borrow(var)?.try_to()?;
        wrap_error(v.connect(this.get_var_borrow(callable)?.try_to()?, flags as _))
    }

    fn disconnect(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let v: Signal = this.get_var_borrow(var)?.try_to()?;
        v.disconnect(this.get_var_borrow(callable)?.try_to()?);
        Ok(())
    }

    fn is_connected(
        &mut self,
        var: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        let v: Signal = this.get_var_borrow(var)?.try_to()?;
        Ok(v.is_connected(this.get_var_borrow(callable)?.try_to()?))
    }

    fn emit(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let v: Signal = this.get_var_borrow(var)?.try_to()?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        v.emit(&args);
        Ok(())
    }
}
