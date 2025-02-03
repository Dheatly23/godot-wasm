use anyhow::Result as AnyResult;
use godot::classes::ProjectSettings;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;

filter_macro! {method [
    singleton -> "singleton",
    add_property_info -> "add-property-info",
    get_global_class_list -> "get-global-class-list",
    has_setting -> "has-setting",
    clear -> "clear",
    get_setting -> "get-setting",
    get_setting_with_override -> "get-setting-with-override",
    set_setting -> "set-setting",
    get_order -> "get-order",
    set_order -> "set-order",
    set_as_basic -> "set-as-basic",
    set_as_internal -> "set-as-internal",
    set_restart_if_changed -> "set-restart-if-changed",
    set_initial_value -> "set-initial-value",
    globalize_path -> "globalize-path",
    localize_path -> "localize-path",
]}

impl crate::godot_component::bindgen::godot::global::project_settings::Host
    for crate::godot_component::GodotCtx
{
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, singleton)?;
        self.set_into_var(ProjectSettings::singleton())
    }

    fn add_property_info(&mut self, val: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, add_property_info)?;
        let v: Dictionary = self.get_value(val)?;
        self.release_store(move || ProjectSettings::singleton().add_property_info(&v));
        Ok(())
    }

    fn get_global_class_list(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, get_global_class_list)?;
        let r = self.release_store(move || ProjectSettings::singleton().get_global_class_list());
        self.set_into_var(r)
    }

    fn has_setting(&mut self, name: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, has_setting)?;
        let n: GString = self.get_value(name)?;
        Ok(self.release_store(move || ProjectSettings::singleton().has_setting(&n)))
    }

    fn clear(&mut self, name: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, clear)?;
        let n: GString = self.get_value(name)?;
        self.release_store(move || ProjectSettings::singleton().clear(&n));
        Ok(())
    }

    fn get_setting(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, get_setting)?;
        let n: GString = self.get_value(name)?;
        let r = self.release_store(move || ProjectSettings::singleton().get_setting(&n));
        self.set_var(r)
    }

    fn get_setting_with_override(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, get_setting_with_override)?;
        let n: StringName = self.get_value(name)?;
        let r =
            self.release_store(move || ProjectSettings::singleton().get_setting_with_override(&n));
        self.set_var(r)
    }

    fn set_setting(
        &mut self,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, set_setting)?;
        let n: GString = self.get_value(name)?;
        let v = self.maybe_get_var(val)?;
        self.release_store(move || ProjectSettings::singleton().set_setting(&n, &v));
        Ok(())
    }

    fn get_order(&mut self, name: WasmResource<Variant>) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, get_order)?;
        let n: GString = self.get_value(name)?;
        Ok(self.release_store(move || ProjectSettings::singleton().get_order(&n)))
    }

    fn set_order(&mut self, name: WasmResource<Variant>, val: i32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, set_order)?;
        let n: GString = self.get_value(name)?;
        self.release_store(move || ProjectSettings::singleton().set_order(&n, val));
        Ok(())
    }

    fn set_as_basic(&mut self, name: WasmResource<Variant>, val: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, set_as_basic)?;
        let n: GString = self.get_value(name)?;
        self.release_store(move || ProjectSettings::singleton().set_as_basic(&n, val));
        Ok(())
    }

    fn set_as_internal(&mut self, name: WasmResource<Variant>, val: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, set_as_internal)?;
        let n: GString = self.get_value(name)?;
        self.release_store(move || ProjectSettings::singleton().set_as_internal(&n, val));
        Ok(())
    }

    fn set_restart_if_changed(&mut self, name: WasmResource<Variant>, val: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, set_restart_if_changed)?;
        let n: GString = self.get_value(name)?;
        self.release_store(move || ProjectSettings::singleton().set_restart_if_changed(&n, val));
        Ok(())
    }

    fn set_initial_value(
        &mut self,
        name: WasmResource<Variant>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, set_initial_value)?;
        let n: GString = self.get_value(name)?;
        let v = self.maybe_get_var(val)?;
        self.release_store(move || ProjectSettings::singleton().set_initial_value(&n, &v));
        Ok(())
    }

    fn globalize_path(&mut self, path: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, globalize_path)?;
        let p: GString = self.get_value(path)?;
        let r = self.release_store(move || ProjectSettings::singleton().globalize_path(&p));
        self.set_into_var(r)
    }

    fn localize_path(&mut self, path: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, project_settings, localize_path)?;
        let p: GString = self.get_value(path)?;
        let r = self.release_store(move || ProjectSettings::singleton().localize_path(&p));
        self.set_into_var(r)
    }
}
