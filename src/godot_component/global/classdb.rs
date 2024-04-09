use anyhow::Result as AnyResult;
use godot::engine::ClassDb;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use super::gate_unsafe;
use crate::godot_util::from_var_any;

impl<T: AsMut<crate::godot_component::GodotCtx>>
    crate::godot_component::bindgen::godot::global::classdb::Host for T
{
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut().set_into_var(ClassDb::singleton())
    }

    fn get_class_list(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(ClassDb::singleton().get_class_list())
    }

    fn class_exists(&mut self, name: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(ClassDb::singleton().class_exists(from_var_any(&*self.as_mut().get_var_borrow(name)?)?))
    }

    fn is_class_enabled(&mut self, name: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(ClassDb::singleton()
            .is_class_enabled(from_var_any(&*self.as_mut().get_var_borrow(name)?)?))
    }

    fn get_parent_class(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let r = ClassDb::singleton().get_parent_class(from_var_any(&*this.get_var_borrow(name)?)?);
        this.set_into_var(r)
    }

    fn is_parent_class(
        &mut self,
        name: WasmResource<Variant>,
        parent: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        Ok(ClassDb::singleton().is_parent_class(
            from_var_any(&*this.get_var_borrow(name)?)?,
            from_var_any(&*this.get_var_borrow(parent)?)?,
        ))
    }

    fn get_inheriters_from_class(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let r = ClassDb::singleton()
            .get_inheriters_from_class(from_var_any(&*this.get_var_borrow(name)?)?);
        this.set_into_var(r)
    }

    fn can_instantiate(&mut self, name: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(ClassDb::singleton()
            .can_instantiate(from_var_any(&*self.as_mut().get_var_borrow(name)?)?))
    }

    fn instantiate(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        let r = ClassDb::singleton().instantiate(from_var_any(&*this.get_var_borrow(name)?)?);
        this.set_var(r)
    }
}
