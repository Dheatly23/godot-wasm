use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;

filter_macro! {method [
    invalid -> "invalid",
    from_object_method -> "from-object-method",
    is_custom -> "is-custom",
    is_valid -> "is-valid",
    object -> "object",
    method_name -> "method-name",
    call -> "call",
    callv -> "callv",
    bind -> "bind",
    bindv -> "bindv",
]}

impl crate::godot_component::bindgen::godot::core::callable::Host
    for crate::godot_component::GodotCtx
{
    fn invalid(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, invalid)?;
        self.set_into_var(Callable::invalid())
    }

    fn from_object_method(
        &mut self,
        obj: WasmResource<Variant>,
        method: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, from_object_method)?;
        let o: Gd<Object> = self.get_value(obj)?;
        let m: StringName = self.get_value(method)?;
        self.set_into_var(Callable::from_object_method(&o, &m))
    }

    fn is_custom(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, is_custom)?;
        Ok(self.get_value::<Callable>(var)?.is_custom())
    }

    fn is_valid(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, is_valid)?;
        Ok(self.get_value::<Callable>(var)?.is_valid())
    }

    fn object(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, object)?;
        let v: Callable = self.get_value(var)?;
        v.object().map(|v| self.set_into_var(v)).transpose()
    }

    fn method_name(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, method_name)?;
        let v: Callable = self.get_value(var)?;
        v.method_name().map(|v| self.set_into_var(v)).transpose()
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, call)?;
        let v: Callable = self.get_value(var)?;
        let mut a = self.get_arg_arr().clone();
        a.clear();
        for v in args {
            a.push(&*self.maybe_get_var_borrow(v)?);
        }
        let r = self.release_store(move || v.callv(&a));
        self.set_var(r)
    }

    fn callv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, callv)?;
        let v: Callable = self.get_value(var)?;
        let args: VariantArray = self.get_value(args)?;
        let r = self.release_store(move || v.callv(&args));
        self.set_var(r)
    }

    fn bind(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, bind)?;
        let v: Callable = self.get_value(var)?;
        let mut a = self.get_arg_arr().clone();
        a.clear();
        for v in args {
            a.push(&*self.maybe_get_var_borrow(v)?);
        }
        self.set_into_var(v.bindv(&a))
    }

    fn bindv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, bindv)?;
        let v: Callable = self.get_value(var)?;
        let args: VariantArray = self.get_value(args)?;
        self.set_into_var(v.bindv(&args))
    }
}
