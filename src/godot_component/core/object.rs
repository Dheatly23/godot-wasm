use anyhow::{Result as AnyResult, bail};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::{ErrorRes, GodotCtx, bindgen, wrap_error};
use crate::wasm_util::get_godot_param_cache;

filter_macro! {method [
    from_instance_id -> "from-instance-id",
    instance_id -> "instance-id",
    free -> "free",
    queue_free -> "queue-free",
    is_queued_for_deletion -> "is-queued-for-deletion",
    cancel_free -> "cancel-free",
    get_class -> "get-class",
    is_class -> "is-class",
    get_script -> "get-script",
    get_property_list -> "get-property-list",
    get_meta_list -> "get-meta-list",
    get_method_list -> "get-method-list",
    get_signal_list -> "get-signal-list",
    has_meta -> "has-meta",
    has_method -> "has-method",
    get_method_argument_count -> "get-method-argument-count",
    has_signal -> "has-signal",
    call -> "call",
    callv -> "callv",
    call_deferred -> "call-deferred",
    connect -> "connect",
    disconnect -> "disconnect",
    is_connected -> "is-connected",
    emit_signal -> "emit-signal",
    is_blocking_signals -> "is-blocking-signals",
    set_block_signals -> "set-block-signals",
    get -> "get",
    set -> "set",
    set_deferred -> "set-deferred",
    get_indexed -> "get-indexed",
    set_indexed -> "set-indexed",
    get_meta -> "get-meta",
    set_meta -> "set-meta",
    remove_meta -> "remove-meta",
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

    fn free(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, free)?;
        let o: Gd<Object> = self.get_value(var)?;
        self.release_store(move || o.free());
        Ok(())
    }

    // It's weird that is_queued_for_deletion and cancel_free are object method, but queue_free is node method.
    // So for symmetry reason upgrade it to object method.
    fn queue_free(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, queue_free)?;
        let mut o: Gd<Node> = self.get_value(var)?;
        self.release_store(move || o.queue_free());
        Ok(())
    }

    fn is_queued_for_deletion(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, is_queued_for_deletion)?;
        self.get_value::<Gd<Object>>(var)
            .map(|o| o.is_queued_for_deletion())
    }

    fn cancel_free(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, cancel_free)?;
        self.get_value::<Gd<Object>>(var)
            .map(|mut o| o.cancel_free())
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
        Ok(o.is_class(&c))
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
        let r = self.release_store(move || o.get_property_list());
        self.set_into_var(r)
    }

    fn get_meta_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_meta_list)?;
        let o: Gd<Object> = self.get_value(var)?;
        let r = self.release_store(move || o.get_meta_list());
        self.set_into_var(r)
    }

    fn get_method_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_method_list)?;
        let o: Gd<Object> = self.get_value(var)?;
        let r = self.release_store(move || o.get_method_list());
        self.set_into_var(r)
    }

    fn get_signal_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_signal_list)?;
        let o: Gd<Object> = self.get_value(var)?;
        let r = self.release_store(move || o.get_signal_list());
        self.set_into_var(r)
    }

    fn has_meta(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, has_meta)?;
        let o: Gd<Object> = self.get_value(var)?;
        let n: StringName = self.get_value(name)?;
        Ok(self.release_store(move || o.has_meta(&n)))
    }

    fn has_method(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, has_method)?;
        let o: Gd<Object> = self.get_value(var)?;
        let n: StringName = self.get_value(name)?;
        Ok(self.release_store(move || o.has_method(&n)))
    }

    fn get_method_argument_count(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_method_argument_count)?;
        let o: Gd<Object> = self.get_value(var)?;
        let n: StringName = self.get_value(name)?;
        Ok(self.release_store(move || o.get_method_argument_count(&n)))
    }

    fn has_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, has_signal)?;
        let o: Gd<Object> = self.get_value(var)?;
        let n: StringName = self.get_value(name)?;
        Ok(self.release_store(move || o.has_signal(&n)))
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
        let mut a = get_godot_param_cache(args.len());
        for (i, v) in args.into_iter().enumerate() {
            a[i] = self.maybe_get_var(v)?;
        }
        let r = self.release_store(move || o.try_call(&name, &a))?;
        self.set_var(r)
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
        let r = self.release_store(move || o.callv(&name, &args));
        self.set_var(r)
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
        let mut a = get_godot_param_cache(args.len());
        for (i, v) in args.into_iter().enumerate() {
            a[i] = self.maybe_get_var(v)?;
        }
        let r = self.release_store(move || o.try_call_deferred(&name, &a))?;
        self.set_var(r)
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
            o.connect_ex(
                &self.get_value::<StringName>(name)?,
                &self.get_value(callable)?,
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
        filter_macro!(filter self.filter.as_ref(), godot_core, object, disconnect)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        o.disconnect(
            &self.get_value::<StringName>(name)?,
            &self.get_value(callable)?,
        );
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
        Ok(o.is_connected(
            &self.get_value::<StringName>(name)?,
            &self.get_value(callable)?,
        ))
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
        wrap_error(self.release_store(move || o.try_emit_signal(&name, &args))?)
    }

    fn is_blocking_signals(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, is_blocking_signals)?;
        let o: Gd<Object> = self.get_value(var)?;
        Ok(self.release_store(move || o.is_blocking_signals()))
    }

    fn set_block_signals(&mut self, var: WasmResource<Variant>, val: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set_block_signals)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        self.release_store(move || o.set_block_signals(val));
        Ok(())
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get)?;
        let o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        let r = self.release_store(move || o.get(&name));
        self.set_var(r)
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let n: StringName = self.get_value(name)?;
        let v = self.maybe_get_var(val)?;
        self.release_store(move || o.set(&n, &v));
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
        let n: StringName = self.get_value(name)?;
        let v = self.maybe_get_var(val)?;
        self.release_store(move || o.set_deferred(&n, &v));
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
        let r = self.release_store(move || o.get_indexed(&name));
        self.set_var(r)
    }

    fn set_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set_indexed)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let n: NodePath = self.get_value(name)?;
        let v = self.maybe_get_var(val)?;
        self.release_store(move || o.set_indexed(&n, &v));
        Ok(())
    }

    fn get_meta(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        default: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, get_meta)?;
        let o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        let default = self.maybe_get_var(default)?;
        let r = self.release_store(move || o.get_meta_ex(&name).default(&default).done());
        self.set_var(r)
    }

    fn set_meta(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, set_meta)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let n: StringName = self.get_value(name)?;
        let v = self.maybe_get_var(val)?;
        self.release_store(move || o.set_meta(&n, &v));
        Ok(())
    }

    fn remove_meta(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, object, remove_meta)?;
        let mut o: Gd<Object> = self.get_value(var)?;
        let name: StringName = self.get_value(name)?;
        self.release_store(move || o.remove_meta(&name));
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
            o.tr_ex(&m)
                .context(&self.get_value::<StringName>(ctx)?)
                .done()
        } else {
            o.tr(&m)
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
            o.tr_n_ex(&m, &p, n)
                .context(&self.get_value::<StringName>(ctx)?)
                .done()
        } else {
            o.tr_n(&m, &p, n)
        };
        self.set_into_var(r)
    }
}
