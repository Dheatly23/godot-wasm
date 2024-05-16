use anyhow::Result as AnyResult;
use godot::engine::ClassDb;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::godot_util::from_var_any;
use crate::site_context;

impl<T: AsMut<GodotCtx>> bindgen::godot::global::classdb::Host for T {
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "classdb", "singleton"))?;
        this.set_into_var(ClassDb::singleton())
    }

    fn get_class_list(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "get-class-list"))?;
        this.set_into_var(ClassDb::singleton().get_class_list())
    }

    fn class_exists(&mut self, class: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "classdb", "class-exists"))?;
        Ok(ClassDb::singleton().class_exists(from_var_any(this.get_var_borrow(class)?)?))
    }

    fn is_class_enabled(&mut self, class: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "is-class-enabled"))?;
        Ok(ClassDb::singleton().is_class_enabled(from_var_any(this.get_var_borrow(class)?)?))
    }

    fn get_parent_class(
        &mut self,
        class: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "get-parent-class"))?;
        let r = ClassDb::singleton().get_parent_class(from_var_any(this.get_var_borrow(class)?)?);
        this.set_into_var(r)
    }

    fn is_parent_class(
        &mut self,
        class: WasmResource<Variant>,
        parent: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "is-parent-class"))?;
        Ok(ClassDb::singleton().is_parent_class(
            from_var_any(this.get_var_borrow(class)?)?,
            from_var_any(this.get_var_borrow(parent)?)?,
        ))
    }

    fn get_inheriters_from_class(
        &mut self,
        class: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "get-inheriters-from-class"))?;
        let r = ClassDb::singleton()
            .get_inheriters_from_class(from_var_any(this.get_var_borrow(class)?)?);
        this.set_into_var(r)
    }

    fn can_instantiate(&mut self, class: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "can-instantiate"))?;
        Ok(ClassDb::singleton().can_instantiate(from_var_any(this.get_var_borrow(class)?)?))
    }

    fn instantiate(
        &mut self,
        class: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "classdb", "instantiate"))?;
        let r = ClassDb::singleton().instantiate(from_var_any(this.get_var_borrow(class)?)?);
        this.set_var(r)
    }

    fn class_get_enum_constants(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-enum-constants"))?;
        let r = ClassDb::singleton()
            .class_get_enum_constants_ex(
                from_var_any(this.get_var_borrow(class)?)?,
                from_var_any(this.get_var_borrow(name)?)?,
            )
            .no_inheritance(no_inherit)
            .done();
        this.set_into_var(r)
    }

    fn class_get_enum_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-enum-list"))?;
        let r = ClassDb::singleton()
            .class_get_enum_list_ex(from_var_any(this.get_var_borrow(class)?)?)
            .no_inheritance(no_inherit)
            .done();
        this.set_into_var(r)
    }

    fn class_get_integer_constant(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<i64> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-integer-constants"))?;
        Ok(ClassDb::singleton().class_get_integer_constant(
            from_var_any(this.get_var_borrow(class)?)?,
            from_var_any(this.get_var_borrow(name)?)?,
        ))
    }

    fn class_get_integer_constant_enum(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass(
            "godot:global",
            "classdb",
            "class-get-integer-constant-enum"
        ))?;
        let r = ClassDb::singleton()
            .class_get_integer_constant_enum_ex(
                from_var_any(this.get_var_borrow(class)?)?,
                from_var_any(this.get_var_borrow(name)?)?,
            )
            .no_inheritance(no_inherit)
            .done();
        this.set_into_var(r)
    }

    fn class_get_integer_constant_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass(
            "godot:global",
            "classdb",
            "class-get-integer-constant-list"
        ))?;
        let r = ClassDb::singleton()
            .class_get_integer_constant_list_ex(from_var_any(this.get_var_borrow(class)?)?)
            .no_inheritance(no_inherit)
            .done();
        this.set_into_var(r)
    }

    fn class_get_method_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-method-list"))?;
        let r = ClassDb::singleton()
            .class_get_method_list_ex(from_var_any(this.get_var_borrow(class)?)?)
            .no_inheritance(no_inherit)
            .done();
        this.set_into_var(r)
    }

    fn class_get_property_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-property-list"))?;
        let r = ClassDb::singleton()
            .class_get_property_list_ex(from_var_any(this.get_var_borrow(class)?)?)
            .no_inheritance(no_inherit)
            .done();
        this.set_into_var(r)
    }

    fn class_get_signal_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-signal-list"))?;
        let r = ClassDb::singleton()
            .class_get_signal_list_ex(from_var_any(this.get_var_borrow(class)?)?)
            .no_inheritance(no_inherit)
            .done();
        this.set_into_var(r)
    }

    fn class_get_signal(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-signal"))?;
        let r = ClassDb::singleton().class_get_signal(
            from_var_any(this.get_var_borrow(class)?)?,
            from_var_any(this.get_var_borrow(name)?)?,
        );
        this.set_into_var(r)
    }

    fn class_get_property(
        &mut self,
        object: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-get-property"))?;
        let r = ClassDb::singleton().class_get_property(
            from_var_any(this.get_var_borrow(object)?)?,
            from_var_any(this.get_var_borrow(name)?)?,
        );
        this.set_var(r)
    }

    fn class_set_property(
        &mut self,
        object: WasmResource<Variant>,
        name: WasmResource<Variant>,
        value: Option<WasmResource<Variant>>,
    ) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-set-property"))?;
        wrap_error(ClassDb::singleton().class_set_property(
            from_var_any(this.get_var_borrow(object)?)?,
            from_var_any(this.get_var_borrow(name)?)?,
            this.maybe_get_var(value)?,
        ))
    }

    fn class_has_enum(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-has-enum"))?;
        Ok(ClassDb::singleton()
            .class_has_enum_ex(
                from_var_any(this.get_var_borrow(class)?)?,
                from_var_any(this.get_var_borrow(name)?)?,
            )
            .no_inheritance(no_inherit)
            .done())
    }

    fn class_has_integer_constant(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-has-integer-constant"))?;
        Ok(ClassDb::singleton().class_has_integer_constant(
            from_var_any(this.get_var_borrow(class)?)?,
            from_var_any(this.get_var_borrow(name)?)?,
        ))
    }

    fn class_has_method(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-has-method"))?;
        Ok(ClassDb::singleton()
            .class_has_method_ex(
                from_var_any(this.get_var_borrow(class)?)?,
                from_var_any(this.get_var_borrow(name)?)?,
            )
            .no_inheritance(no_inherit)
            .done())
    }

    fn class_has_signal(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "classdb", "class-has-signal"))?;
        Ok(ClassDb::singleton().class_has_signal(
            from_var_any(this.get_var_borrow(class)?)?,
            from_var_any(this.get_var_borrow(name)?)?,
        ))
    }
}
