use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use super::wrap_error;

impl<T: AsMut<crate::godot_component::GodotCtx>>
    crate::godot_component::bindgen::godot::core::object::Host for T
{
    fn from_instance_id(&mut self, id: i64) -> AnyResult<WasmResource<Variant>> {
        let Some(id) = InstanceId::try_from_i64(id) else {
            bail!("Instance ID is 0")
        };

        self.as_mut()
            .set_into_var(<Gd<Object>>::try_from_instance_id(id)?)
    }

    fn instance_id(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        Ok(self
            .as_mut()
            .get_var_borrow(var)?
            .try_to::<Gd<Object>>()?
            .instance_id()
            .to_i64())
    }

    fn get_property_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        this.set_into_var(o.get_property_list())
    }

    fn get_method_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        this.set_into_var(o.get_method_list())
    }

    fn get_signal_list(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        this.set_into_var(o.get_signal_list())
    }

    fn has_method(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        Ok(o.has_method(this.get_var_borrow(name)?.try_to()?))
    }

    fn has_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        Ok(o.has_signal(this.get_var_borrow(name)?.try_to()?))
    }

    fn call(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        let name: StringName = this.get_var_borrow(name)?.try_to()?;
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
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        let name: StringName = this.get_var_borrow(name)?.try_to()?;
        let args: Array<Variant> = this.get_var_borrow(args)?.try_to()?;
        this.set_var(o.callv(name, args))
    }

    fn call_deferred(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        let name: StringName = this.get_var_borrow(name)?.try_to()?;
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
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        wrap_error(
            o.connect_ex(
                this.get_var_borrow(name)?.try_to()?,
                this.get_var_borrow(callable)?.try_to()?,
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
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        o.disconnect(
            this.get_var_borrow(name)?.try_to()?,
            this.get_var_borrow(callable)?.try_to()?,
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
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        Ok(o.is_connected(
            this.get_var_borrow(name)?.try_to()?,
            this.get_var_borrow(callable)?.try_to()?,
        ))
    }

    fn emit_signal(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        args: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        let name: StringName = this.get_var_borrow(name)?.try_to()?;
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
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        let name: StringName = this.get_var_borrow(name)?.try_to()?;
        this.set_var(o.get(name))
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        o.set(
            this.get_var_borrow(name)?.try_to()?,
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
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        o.set_deferred(
            this.get_var_borrow(name)?.try_to()?,
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
        let o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        let name: NodePath = this.get_var_borrow(name)?.try_to()?;
        this.set_var(o.get_indexed(name))
    }

    fn set_indexed(
        &mut self,
        var: WasmResource<Variant>,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut o: Gd<Object> = this.get_var_borrow(var)?.try_to()?;
        o.set_indexed(
            this.get_var_borrow(name)?.try_to()?,
            this.maybe_get_var(val)?,
        );
        Ok(())
    }
}
