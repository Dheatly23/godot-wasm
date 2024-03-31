use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

impl<T: AsMut<crate::godot_component::GodotCtx>>
    crate::godot_component::bindgen::godot::core::callable::Host for T
{
    fn invalid(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut().set_into_var(Callable::invalid())
    }

    fn from_object_method(
        &mut self,
        obj: WasmResource<Variant>,
        method: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_var_borrow(obj)?.try_to()?;
        let m: StringName = this.get_var_borrow(method)?.try_to()?;
        this.set_into_var(Callable::from_object_method(&o, m))
    }

    fn is_custom(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self
            .as_mut()
            .get_var_borrow(var)?
            .try_to::<Callable>()?
            .is_custom())
    }

    fn is_valid(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self
            .as_mut()
            .get_var_borrow(var)?
            .try_to::<Callable>()?
            .is_valid())
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let v: Callable = this.get_var_borrow(var)?.try_to()?;
        v.object().map(|v| this.set_into_var(v)).transpose()
    }

    fn method_name(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let v: Callable = this.get_var_borrow(var)?.try_to()?;
        v.method_name().map(|v| this.set_into_var(v)).transpose()
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let v: Callable = this.get_var_borrow(var)?.try_to()?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Array<Variant>>>()?;
        this.set_var(v.callv(args))
    }

    fn callv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let v: Callable = this.get_var_borrow(var)?.try_to()?;
        let args: Array<Variant> = this.get_var_borrow(args)?.try_to()?;
        this.set_var(v.callv(args))
    }

    fn bind(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v: Callable = this.get_var_borrow(var)?.try_to()?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Array<Variant>>>()?;
        this.set_into_var(v.bindv(args))
    }

    fn bindv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v: Callable = this.get_var_borrow(var)?.try_to()?;
        let args: Array<Variant> = this.get_var_borrow(args)?.try_to()?;
        this.set_into_var(v.bindv(args))
    }
}
