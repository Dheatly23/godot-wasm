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
    call_deferred -> "call-deferred",
    callv -> "callv",
    bind -> "bind",
    bindv -> "bindv",
    unbind -> "unbind",
    get_argument_count -> "get-argument-count",
    get_bound_arguments -> "get-bound-arguments",
    get_bound_arguments_count -> "get-bound-arguments-count",
    rpc -> "rpc",
    rpc_id -> "rpc-id",
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
        let a = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        let r = self.release_store(move || v.call(&a));
        self.set_var(r)
    }

    fn call_deferred(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, call_deferred)?;
        let v: Callable = self.get_value(var)?;
        let a = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        self.release_store(move || v.call_deferred(&a));
        Ok(())
    }

    fn callv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, callv)?;
        let v: Callable = self.get_value(var)?;
        let args: VarArray = self.get_value(args)?;
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
        let a = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        self.set_into_var(v.bind(&a))
    }

    fn bindv(
        &mut self,
        var: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, bindv)?;
        let v: Callable = self.get_value(var)?;
        let args: VarArray = self.get_value(args)?;
        self.set_into_var(v.bindv(&args))
    }

    fn unbind(&mut self, var: WasmResource<Variant>, n: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, unbind)?;
        let v: Callable = self.get_value(var)?;
        self.set_into_var(v.unbind(n.try_into()?))
    }

    fn get_argument_count(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, get_argument_count)?;
        let v: Callable = self.get_value(var)?;
        Ok(v.get_argument_count() as _)
    }

    fn get_bound_arguments(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, get_bound_arguments)?;
        let v: Callable = self.get_value(var)?;
        self.set_into_var(v.get_bound_arguments())
    }

    fn get_bound_arguments_count(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, get_bound_arguments_count)?;
        let v: Callable = self.get_value(var)?;
        Ok(v.get_bound_arguments_count() as _)
    }

    fn rpc(
        &mut self,
        var: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, rpc)?;
        let v: Callable = self.get_value(var)?;
        let a = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        self.release_store(move || v.rpc(&a));
        Ok(())
    }

    fn rpc_id(
        &mut self,
        var: WasmResource<Variant>,
        peer_id: i64,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, callable, rpc_id)?;
        let v: Callable = self.get_value(var)?;
        let a = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        self.release_store(move || v.rpc_id(peer_id, &a));
        Ok(())
    }
}
