use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::site_context;

impl<T: AsMut<GodotCtx>> bindgen::godot::core::object::Host for T {
    fn from_instance_id(&mut self, id: i64) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "from-instance-id"))?;
        let Some(id) = InstanceId::try_from_i64(id) else {
            bail!("Instance ID is 0")
        };

        this.set_into_var(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased())?)
    }

    fn instance_id(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "instance-id"))?;
        this.get_value::<Gd<Object>>(var)
            .map(|v| v.instance_id().to_i64())
    }

    fn get_class(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-class"))?;
        let o: Gd<Object> = this.get_value(var)?;
        this.set_into_var(o.get_class())
    }

    fn is_class(
        &mut self,
        var: WasmResource<Variant>,
        class: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "is-class"))?;
        let o: Gd<Object> = this.get_value(var)?;
        let c: GString = this.get_value(class)?;
        Ok(o.is_class(c))
    }

    fn get_script(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-script"))?;
        let o: Gd<Object> = this.get_value(var)?;
        this.set_var(o.get_script())
    }

    fn get_property_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:core", "object", "get-property-list"))?;
        let o: Gd<Object> = this.get_value(var)?;
        this.set_into_var(o.get_property_list())
    }

    fn get_method_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-method-list"))?;
        let o: Gd<Object> = this.get_value(var)?;
        this.set_into_var(o.get_method_list())
    }

    fn get_signal_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-signal-list"))?;
        let o: Gd<Object> = this.get_value(var)?;
        this.set_into_var(o.get_signal_list())
    }

    fn has_method(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "has-method"))?;
        let o: Gd<Object> = this.get_value(var)?;
        Ok(o.has_method(this.get_value(name)?))
    }

    fn has_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "has-signal"))?;
        let o: Gd<Object> = this.get_value(var)?;
        Ok(o.has_signal(this.get_value(name)?))
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "call"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        let name: StringName = this.get_value(name)?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        this.set_var(o.try_call(name, &args)?)
    }

    fn callv(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "callv"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        let name: StringName = this.get_value(name)?;
        let args: VariantArray = this.get_value(args)?;
        this.set_var(o.callv(name, args))
    }

    fn call_deferred(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "call-deferred"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        let name: StringName = this.get_value(name)?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        this.set_var(o.try_call_deferred(name, &args)?)
    }

    fn connect(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        callable: WasmResource<Variant>,
        flags: u32,
    ) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "connect"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        wrap_error(
            o.connect_ex(this.get_value(name)?, this.get_value(callable)?)
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
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "disconnect"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        o.disconnect(this.get_value(name)?, this.get_value(callable)?);
        Ok(())
    }

    fn is_connected(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        callable: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "is-connected"))?;
        let o: Gd<Object> = this.get_value(var)?;
        Ok(o.is_connected(this.get_value(name)?, this.get_value(callable)?))
    }

    fn emit_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "emit-signal"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        let name: StringName = this.get_value(name)?;
        let args = args
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<Vec<_>>>()?;
        wrap_error(o.try_emit_signal(name, &args)?)
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get"))?;
        let o: Gd<Object> = this.get_value(var)?;
        let name: StringName = this.get_value(name)?;
        this.set_var(o.get(name))
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "set"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        o.set(this.get_value(name)?, this.maybe_get_var(val)?);
        Ok(())
    }

    fn set_deferred(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "set-deferred"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        o.set_deferred(this.get_value(name)?, this.maybe_get_var(val)?);
        Ok(())
    }

    fn get_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_value(var)?;
        let name: NodePath = this.get_value(name)?;
        this.set_var(o.get_indexed(name))
    }

    fn set_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-indexed"))?;
        let mut o: Gd<Object> = this.get_value(var)?;
        o.set_indexed(this.get_value(name)?, this.maybe_get_var(val)?);
        Ok(())
    }

    fn can_translate_messages(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:core", "object", "can-translate-messages"))?;
        Ok(this.get_value::<Gd<Object>>(var)?.can_translate_messages())
    }

    fn set_message_translation(&mut self, var: WasmResource<Variant>, val: bool) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:core", "object", "set-message-translation"))?;
        this.get_value::<Gd<Object>>(var)?
            .set_message_translation(val);
        Ok(())
    }

    fn tr(
        &mut self,
        var: WasmResource<Variant>,
        msg: WasmResource<Variant>,
        ctx: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "tr"))?;
        let o: Gd<Object> = this.get_value(var)?;
        let m: StringName = this.get_value(msg)?;
        let r = if let Some(ctx) = ctx {
            o.tr_ex(m).context(this.get_value(ctx)?).done()
        } else {
            o.tr(m)
        };
        this.set_into_var(r)
    }

    fn tr_n(
        &mut self,
        var: WasmResource<Variant>,
        msg: WasmResource<Variant>,
        plural: WasmResource<Variant>,
        n: i32,
        ctx: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "tr-n"))?;
        let o: Gd<Object> = this.get_value(var)?;
        let m: StringName = this.get_value(msg)?;
        let p: StringName = this.get_value(plural)?;
        let r = if let Some(ctx) = ctx {
            o.tr_n_ex(m, p, n).context(this.get_value(ctx)?).done()
        } else {
            o.tr_n(m, p, n)
        };
        this.set_into_var(r)
    }
}
