use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::godot_util::from_var_any;
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
        this.get_var_borrow(var)
            .and_then(from_var_any::<Gd<Object>>)
            .map(|v| v.instance_id().to_i64())
    }

    fn get_class(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-class"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        this.set_into_var(o.get_class())
    }

    fn is_class(
        &mut self,
        var: WasmResource<Variant>,
        class: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "is-class"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let c: GString = from_var_any(this.get_var_borrow(class)?)?;
        Ok(o.is_class(c))
    }

    fn get_script(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-script"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
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
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        this.set_into_var(o.get_property_list())
    }

    fn get_method_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-method-list"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        this.set_into_var(o.get_method_list())
    }

    fn get_signal_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "get-signal-list"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        this.set_into_var(o.get_signal_list())
    }

    fn has_method(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "has-method"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        Ok(o.has_method(from_var_any(this.get_var_borrow(name)?)?))
    }

    fn has_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "has-signal"))?;
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        Ok(o.has_signal(from_var_any(this.get_var_borrow(name)?)?))
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "call"))?;
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(this.get_var_borrow(name)?)?;
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
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(this.get_var_borrow(name)?)?;
        let args: VariantArray = from_var_any(this.get_var_borrow(args)?)?;
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
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(this.get_var_borrow(name)?)?;
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
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        wrap_error(
            o.connect_ex(
                from_var_any(this.get_var_borrow(name)?)?,
                from_var_any(this.get_var_borrow(callable)?)?,
            )
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
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        o.disconnect(
            from_var_any(this.get_var_borrow(name)?)?,
            from_var_any(this.get_var_borrow(callable)?)?,
        );
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
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        Ok(o.is_connected(
            from_var_any(this.get_var_borrow(name)?)?,
            from_var_any(this.get_var_borrow(callable)?)?,
        ))
    }

    fn emit_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "object", "emit-signal"))?;
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(this.get_var_borrow(name)?)?;
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
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(this.get_var_borrow(name)?)?;
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
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        o.set(
            from_var_any(this.get_var_borrow(name)?)?,
            this.maybe_get_var(val)?,
        );
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
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        o.set_deferred(
            from_var_any(this.get_var_borrow(name)?)?,
            this.maybe_get_var(val)?,
        );
        Ok(())
    }

    fn get_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let name: NodePath = from_var_any(this.get_var_borrow(name)?)?;
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
        let mut o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        o.set_indexed(
            from_var_any(this.get_var_borrow(name)?)?,
            this.maybe_get_var(val)?,
        );
        Ok(())
    }

    fn can_translate_messages(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:core", "object", "can-translate-messages"))?;
        Ok(from_var_any::<Gd<Object>>(this.get_var_borrow(var)?)?.can_translate_messages())
    }

    fn set_message_translation(&mut self, var: WasmResource<Variant>, val: bool) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:core", "object", "set-message-translation"))?;
        from_var_any::<Gd<Object>>(this.get_var_borrow(var)?)?.set_message_translation(val);
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
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let m: StringName = from_var_any(this.get_var_borrow(msg)?)?;
        let r = if let Some(ctx) = ctx {
            o.tr_ex(m)
                .context(from_var_any(this.get_var_borrow(ctx)?)?)
                .done()
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
        let o: Gd<Object> = from_var_any(this.get_var_borrow(var)?)?;
        let m: StringName = from_var_any(this.get_var_borrow(msg)?)?;
        let p: StringName = from_var_any(this.get_var_borrow(plural)?)?;
        let r = if let Some(ctx) = ctx {
            o.tr_n_ex(m, p, n)
                .context(from_var_any(this.get_var_borrow(ctx)?)?)
                .done()
        } else {
            o.tr_n(m, p, n)
        };
        this.set_into_var(r)
    }
}
