use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

impl crate::godot_component::bindgen::godot::core::callable::Host
    for crate::godot_component::GodotCtx
{
    fn invalid(&mut self) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Callable::invalid()))
    }

    fn from_object_method(
        &mut self,
        obj: WasmResource<Variant>,
        method: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let o: Gd<Object> = self.get_var_borrow(obj)?.try_to()?;
        let m: StringName = self.get_var_borrow(method)?.try_to()?;
        Ok(self.set_into_var(&Callable::from_object_method(&o, m)))
    }

    fn is_custom(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.try_to::<Callable>()?.is_custom())
    }

    fn is_valid(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.try_to::<Callable>()?.is_valid())
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let v: Callable = self.get_var_borrow(var)?.try_to()?;
        Ok(v.object().map(|v| self.set_into_var(&v)))
    }

    fn method_name(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let v: Callable = self.get_var_borrow(var)?.try_to()?;
        Ok(v.method_name().map(|v| self.set_into_var(&v)))
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let v: Callable = self.get_var_borrow(var)?.try_to()?;
        let args = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Array<Variant>>>()?;
        Ok(self.set_var(v.callv(args)))
    }

    fn callv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let v: Callable = self.get_var_borrow(var)?.try_to()?;
        let args: Array<Variant> = self.get_var_borrow(args)?.try_to()?;
        Ok(self.set_var(v.callv(args)))
    }

    fn bind(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let v: Callable = self.get_var_borrow(var)?.try_to()?;
        let args = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Array<Variant>>>()?;
        Ok(self.set_into_var(&v.bindv(args)))
    }

    fn bindv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let v: Callable = self.get_var_borrow(var)?.try_to()?;
        let args: Array<Variant> = self.get_var_borrow(args)?.try_to()?;
        Ok(self.set_into_var(&v.bindv(args)))
    }
}
