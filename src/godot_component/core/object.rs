use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};

filter_macro! {method [
    from_instance_id -> "from-instance-id",
    instance_id -> "instance-id",
    get_class -> "get-class",
    is_class -> "is-class",
    get_script -> "get-script",
    get_property_list -> "get-property-list",
    get_method_list -> "get-method-list",
    get_signal_list -> "get-signal-list",
    has_method -> "has-method",
    has_signal -> "has-signal",
    call -> "call",
    callv -> "callv",
    call_deferred -> "call-deferred",
    connect -> "connect",
    disconnect -> "disconnect",
    is_connected -> "is-connected",
    emit_signal -> "emit-signal",
    get -> "get",
    set -> "set",
    set_deferred -> "set-deferred",
    get_indexed -> "get-indexed",
    set_indexed -> "set-indexed",
    can_translate_messages -> "can-translate-messages",
    set_message_translation -> "set-message-translation",
    tr -> "tr",
    tr_n -> "tr-n",
]}

impl bindgen::godot::core::object::Host for GodotCtx {
    fn from_instance_id(&mut self, id: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, from_instance_id)?;
        let Some(id) = InstanceId::try_from_i64(id) else {
            bail!("Instance ID is 0")
        };

        self.set_into_var(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased())?)
    }

    fn instance_id(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, instance_id)?;
        self.get_value::<Gd<Object>>(var)
            .map(|v| v.instance_id().to_i64())
    }

    fn get_class(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_class)?;
        let o: Gd<Object> = self.get_value(var)?;
        self.set_into_var(o.get_class())
    }

    fn is_class(
        &mut self,
        var: WasmResource<Variant>,
        class: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, is_class)?;
        let o: Gd<Object> = self.get_value(var)?;
        let c: GString = self.get_value(class)?;
        Ok(o.is_class(c))
    }

    fn get_script(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_script)?;
        let o: Gd<Object> = self.get_value(var)?;
        self.set_var(o.get_script())
    }

    fn get_property_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_property_list)?;
        let o: Gd<Object> = self.get_value(var)?;
        self.set_into_var(o.get_property_list())
    }

    fn get_method_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_method_list)?;
        let o: Gd<Object> = self.get_value(var)?;
        self.set_into_var(o.get_method_list())
    }

    fn get_signal_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_signal_list)?;
        let o: Gd<Object> = self.get_value(var)?;
        self.set_into_var(o.get_signal_list())
    }

    fn has_method(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, has_method)?;
        let o: Gd<Object> = self.get_value(var)?;
        Ok(o.has_method(self.get_value(name)?))
    }

    fn has_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, has_signal)?;
        let o: Gd<Object> = self.get_value(var)?;
        Ok(o.has_signal(self.get_value(name)?))
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, call)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        let args = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        self.set_var(o.try_call(name, &args)?)
    }

    fn callv(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, callv)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        let args: VariantArray = self.get_value(args)?;
        self.set_var(o.callv(name, &args))
    }

    fn call_deferred(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, call_deferred)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        let args = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        self.set_var(o.try_call_deferred(name, &args)?)
    }

    fn connect(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        callable: WasmResource<Variant>,
        flags: u32,
    ) -> ErrorRes {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, connect)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        wrap_error(
            o.connect_ex(self.get_value(name)?, self.get_value(callable)?)
                .flags(flags)
                .done(),
        )
    }

    fn disconnect(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, disconnect)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        o.disconnect(self.get_value(name)?, self.get_value(callable)?);
        Ok(())
    }

    fn is_connected(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, is_connected)?;
        let o: Gd<Object> = self.get_value(var)?;
        Ok(o.is_connected(self.get_value(name)?, self.get_value(callable)?))
    }

    fn emit_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> ErrorRes {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, emit_signal)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        let args = args
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        wrap_error(o.try_emit_signal(name, &args)?)
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get)?;
        let o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        self.set_var(o.get(name))
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        o.set(self.get_value(name)?, &*self.maybe_get_var_borrow(val)?);
        Ok(())
    }

    fn set_deferred(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set_deferred)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        o.set_deferred(self.get_value(name)?, &*self.maybe_get_var_borrow(val)?);
        Ok(())
    }

    fn get_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_indexed)?;
        let o: Gd<Object> = self.get_value(var)?;
        let name: NodePath = self.get_value(name)?;
        self.set_var(o.get_indexed(name))
    }

    fn set_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set_indexed)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        o.set_indexed(self.get_value(name)?, &*self.maybe_get_var_borrow(val)?);
        Ok(())
    }

    fn can_translate_messages(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, can_translate_messages)?;
        Ok(self.get_value::<Gd<Object>>(var)?.can_translate_messages())
    }

    fn set_message_translation(&mut self, var: WasmResource<Variant>, val: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set_message_translation)?;
        self.get_value::<Gd<Object>>(var)?
            .set_message_translation(val);
        Ok(())
    }

    fn tr(
        &mut self,
        var: WasmResource<Variant>,
        msg: WasmResource<Variant>,
        ctx: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, tr)?;
        let o: Gd<Object> = self.get_value(var)?;
        let m: StringName = self.get_value(msg)?;
        let r = if let Some(ctx) = ctx {
            o.tr_ex(m).context(self.get_value(ctx)?).done()
        } else {
            o.tr(m)
        };
        self.set_into_var(r)
    }

    fn tr_n(
        &mut self,
        var: WasmResource<Variant>,
        msg: WasmResource<Variant>,
        plural: WasmResource<Variant>,
        n: i32,
        ctx: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, tr_n)?;
        let o: Gd<Object> = self.get_value(var)?;
        let m: StringName = self.get_value(msg)?;
        let p: StringName = self.get_value(plural)?;
        let r = if let Some(ctx) = ctx {
            o.tr_n_ex(m, p, n).context(self.get_value(ctx)?).done()
        } else {
            o.tr_n(m, p, n)
        };
        self.set_into_var(r)
    }
}
