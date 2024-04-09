use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, GodotCtx};
use crate::godot_util::from_var_any;

impl<T: AsMut<GodotCtx>> bindgen::godot::core::object::Host for T {
    fn from_instance_id(&mut self, id: i64) -> AnyResult<WasmResource<Variant>> {
        let Some(id) = InstanceId::try_from_i64(id) else {
            bail!("Instance ID is 0")
        };

        self.as_mut()
            .set_into_var(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased())?)
    }

    fn instance_id(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        Ok(
            from_var_any::<Gd<Object>>(&*self.as_mut().get_var_borrow(var)?)?
                .instance_id()
                .to_i64(),
        )
    }

    fn get_class(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(
            from_var_any::<Gd<Object>>(&*self.as_mut().get_var_borrow(var)?)?
                .get_class()
                .to_string(),
        )
    }

    fn is_class(&mut self, var: WasmResource<Variant>, class: String) -> AnyResult<bool> {
        Ok(
            from_var_any::<Gd<Object>>(&*self.as_mut().get_var_borrow(var)?)?
                .is_class(class.into()),
        )
    }

    fn get_script(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        this.set_var(o.get_script())
    }

    fn get_property_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        this.set_into_var(o.get_property_list())
    }

    fn get_method_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        this.set_into_var(o.get_method_list())
    }

    fn get_signal_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        this.set_into_var(o.get_signal_list())
    }

    fn has_method(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        Ok(o.has_method(from_var_any(&*this.get_var_borrow(name)?)?))
    }

    fn has_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        Ok(o.has_signal(from_var_any(&*this.get_var_borrow(name)?)?))
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(&*this.get_var_borrow(name)?)?;
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
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(&*this.get_var_borrow(name)?)?;
        let args: Array<Variant> = from_var_any(&*this.get_var_borrow(args)?)?;
        this.set_var(o.callv(name, args))
    }

    fn call_deferred(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(&*this.get_var_borrow(name)?)?;
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
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        wrap_error(
            o.connect_ex(
                from_var_any(&*this.get_var_borrow(name)?)?,
                from_var_any(&*this.get_var_borrow(callable)?)?,
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
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        o.disconnect(
            from_var_any(&*this.get_var_borrow(name)?)?,
            from_var_any(&*this.get_var_borrow(callable)?)?,
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
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        Ok(o.is_connected(
            from_var_any(&*this.get_var_borrow(name)?)?,
            from_var_any(&*this.get_var_borrow(callable)?)?,
        ))
    }

    fn emit_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(&*this.get_var_borrow(name)?)?;
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
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        let name: StringName = from_var_any(&*this.get_var_borrow(name)?)?;
        this.set_var(o.get(name))
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        o.set(
            from_var_any(&*this.get_var_borrow(name)?)?,
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
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        o.set_deferred(
            from_var_any(&*this.get_var_borrow(name)?)?,
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
        let o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        let name: NodePath = from_var_any(&*this.get_var_borrow(name)?)?;
        this.set_var(o.get_indexed(name))
    }

    fn set_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut o: Gd<Object> = from_var_any(&*this.get_var_borrow(var)?)?;
        o.set_indexed(
            from_var_any(&*this.get_var_borrow(name)?)?,
            this.maybe_get_var(val)?,
        );
        Ok(())
    }
}
