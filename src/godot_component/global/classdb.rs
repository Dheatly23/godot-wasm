use anyhow::Result as AnyResult;
use godot::classes::ClassDb;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};

filter_macro! {method [
    singleton -> "singleton",
    get_class_list -> "get-class-list",
    class_exists -> "class-exists",
    is_class_enabled -> "is-class-enabled",
    get_parent_class -> "get-parent-class",
    is_parent_class -> "is-parent-class",
    get_inheriters_from_class -> "get-inheriters-from-class",
    can_instantiate -> "can-instantiate",
    instantiate -> "instantiate",
    class_get_enum_constants -> "class-get-enum-constants",
    class_get_enum_list -> "class-get-enum-list",
    class_get_integer_constant -> "class-get-integer-constant",
    class_get_integer_constant_enum -> "class-get-integer-constant-enum",
    class_get_integer_constant_list -> "class-get-integer-constant-list",
    class_get_method_list -> "class-get-method-list",
    class_get_property_list -> "class-get-property-list",
    class_get_property -> "class-get-property",
    class_set_property -> "class-set-property",
    class_get_signal -> "class-get-signal",
    class_get_signal_list -> "class-get-signal-list",
    class_has_enum -> "class-has-enum",
    class_has_integer_constant -> "class-has-integer-constant",
    class_has_method -> "class-has-method",
    class_has_signal -> "class-has-signal",
]}

impl bindgen::godot::global::classdb::Host for GodotCtx {
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, singleton)?;
        self.set_into_var(ClassDb::singleton())
    }

    fn get_class_list(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, get_class_list)?;
        self.set_into_var(ClassDb::singleton().get_class_list())
    }

    fn class_exists(&mut self, class: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_exists)?;
        Ok(ClassDb::singleton().class_exists(self.get_value(class)?))
    }

    fn is_class_enabled(&mut self, class: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, is_class_enabled)?;
        Ok(ClassDb::singleton().is_class_enabled(self.get_value(class)?))
    }

    fn get_parent_class(
        &mut self,
        class: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, get_parent_class)?;
        let r = ClassDb::singleton().get_parent_class(self.get_value(class)?);
        self.set_into_var(r)
    }

    fn is_parent_class(
        &mut self,
        class: WasmResource<Variant>,
        parent: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, is_parent_class)?;
        Ok(ClassDb::singleton().is_parent_class(self.get_value(class)?, self.get_value(parent)?))
    }

    fn get_inheriters_from_class(
        &mut self,
        class: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, get_inheriters_from_class)?;
        let r = ClassDb::singleton().get_inheriters_from_class(self.get_value(class)?);
        self.set_into_var(r)
    }

    fn can_instantiate(&mut self, class: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, can_instantiate)?;
        Ok(ClassDb::singleton().can_instantiate(self.get_value(class)?))
    }

    fn instantiate(
        &mut self,
        class: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, instantiate)?;
        let r = ClassDb::singleton().instantiate(self.get_value(class)?);
        self.set_var(r)
    }

    fn class_get_enum_constants(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_enum_constants)?;
        let r = ClassDb::singleton()
            .class_get_enum_constants_ex(self.get_value(class)?, self.get_value(name)?)
            .no_inheritance(no_inherit)
            .done();
        self.set_into_var(r)
    }

    fn class_get_enum_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_enum_list)?;
        let r = ClassDb::singleton()
            .class_get_enum_list_ex(self.get_value(class)?)
            .no_inheritance(no_inherit)
            .done();
        self.set_into_var(r)
    }

    fn class_get_integer_constant(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_integer_constant)?;
        Ok(ClassDb::singleton()
            .class_get_integer_constant(self.get_value(class)?, self.get_value(name)?))
    }

    fn class_get_integer_constant_enum(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_integer_constant_enum)?;
        let r = ClassDb::singleton()
            .class_get_integer_constant_enum_ex(self.get_value(class)?, self.get_value(name)?)
            .no_inheritance(no_inherit)
            .done();
        self.set_into_var(r)
    }

    fn class_get_integer_constant_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_integer_constant_list)?;
        let r = ClassDb::singleton()
            .class_get_integer_constant_list_ex(self.get_value(class)?)
            .no_inheritance(no_inherit)
            .done();
        self.set_into_var(r)
    }

    fn class_get_method_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_method_list)?;
        let r = ClassDb::singleton()
            .class_get_method_list_ex(self.get_value(class)?)
            .no_inheritance(no_inherit)
            .done();
        self.set_into_var(r)
    }

    fn class_get_property_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_property_list)?;
        let r = ClassDb::singleton()
            .class_get_property_list_ex(self.get_value(class)?)
            .no_inheritance(no_inherit)
            .done();
        self.set_into_var(r)
    }

    fn class_get_signal_list(
        &mut self,
        class: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_signal_list)?;
        let r = ClassDb::singleton()
            .class_get_signal_list_ex(self.get_value(class)?)
            .no_inheritance(no_inherit)
            .done();
        self.set_into_var(r)
    }

    fn class_get_signal(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_signal)?;
        let r =
            ClassDb::singleton().class_get_signal(self.get_value(class)?, self.get_value(name)?);
        self.set_into_var(r)
    }

    fn class_get_property(
        &mut self,
        object: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_get_property)?;
        let r = ClassDb::singleton()
            .class_get_property(self.get_object::<Object>(object)?, self.get_value(name)?);
        self.set_var(r)
    }

    fn class_set_property(
        &mut self,
        object: WasmResource<Variant>,
        name: WasmResource<Variant>,
        value: Option<WasmResource<Variant>>,
    ) -> ErrorRes {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_set_property)?;
        wrap_error(ClassDb::singleton().class_set_property(
            self.get_object::<Object>(object)?,
            self.get_value(name)?,
            self.maybe_get_var(value)?,
        ))
    }

    fn class_has_enum(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_has_enum)?;
        Ok(ClassDb::singleton()
            .class_has_enum_ex(self.get_value(class)?, self.get_value(name)?)
            .no_inheritance(no_inherit)
            .done())
    }

    fn class_has_integer_constant(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_has_integer_constant)?;
        Ok(ClassDb::singleton()
            .class_has_integer_constant(self.get_value(class)?, self.get_value(name)?))
    }

    fn class_has_method(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
        no_inherit: bool,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_has_method)?;
        Ok(ClassDb::singleton()
            .class_has_method_ex(self.get_value(class)?, self.get_value(name)?)
            .no_inheritance(no_inherit)
            .done())
    }

    fn class_has_signal(
        &mut self,
        class: WasmResource<Variant>,
        name: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, classdb, class_has_signal)?;
        Ok(ClassDb::singleton().class_has_signal(self.get_value(class)?, self.get_value(name)?))
    }
}
