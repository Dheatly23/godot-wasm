use anyhow::Result as AnyResult;
use godot::classes::{InputEvent, InputMap};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;

filter_macro! {method [
    singleton -> "singleton",
    action_add_event -> "action-add-event",
    action_erase_event -> "action-erase-event",
    action_erase_events -> "action-erase-events",
    action_get_deadzone -> "action-get-deadzone",
    action_get_events -> "action-get-events",
    action_has_event -> "action-has-event",
    action_set_deadzone -> "action-set-deadzone",
    add_action -> "add-action",
    erase_action -> "erase-action",
    event_is_action -> "event-is-action",
    get_actions -> "get-actions",
    has_action -> "has-action",
    load_from_project_settings -> "load-from-project-settings",
]}

impl crate::godot_component::bindgen::godot::global::input_map::Host
    for crate::godot_component::GodotCtx
{
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, singleton)?;
        self.set_into_var(InputMap::singleton())
    }

    fn action_add_event(
        &mut self,
        a: WasmResource<Variant>,
        e: WasmResource<Variant>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, action_add_event)?;
        InputMap::singleton()
            .action_add_event(self.get_value(a)?, self.get_object::<InputEvent>(e)?);
        Ok(())
    }

    fn action_erase_event(
        &mut self,
        a: WasmResource<Variant>,
        e: WasmResource<Variant>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, action_erase_event)?;
        InputMap::singleton()
            .action_erase_event(self.get_value(a)?, self.get_object::<InputEvent>(e)?);
        Ok(())
    }

    fn action_erase_events(&mut self, a: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, action_erase_events)?;
        InputMap::singleton().action_erase_events(self.get_value(a)?);
        Ok(())
    }

    fn action_get_deadzone(&mut self, a: WasmResource<Variant>) -> AnyResult<f32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, action_get_deadzone)?;
        Ok(InputMap::singleton().action_get_deadzone(self.get_value(a)?))
    }

    fn action_get_events(&mut self, a: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, action_get_events)?;
        let r = InputMap::singleton().action_get_events(self.get_value(a)?);
        self.set_into_var(r)
    }

    fn action_has_event(
        &mut self,
        a: WasmResource<Variant>,
        e: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, action_has_event)?;
        Ok(InputMap::singleton()
            .action_has_event(self.get_value(a)?, self.get_object::<InputEvent>(e)?))
    }

    fn action_set_deadzone(&mut self, a: WasmResource<Variant>, v: f32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, action_set_deadzone)?;
        InputMap::singleton().action_set_deadzone(self.get_value(a)?, v);
        Ok(())
    }

    fn add_action(&mut self, a: WasmResource<Variant>, v: f32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, add_action)?;
        InputMap::singleton()
            .add_action_ex(self.get_value(a)?)
            .deadzone(v)
            .done();
        Ok(())
    }

    fn erase_action(&mut self, a: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, erase_action)?;
        InputMap::singleton().erase_action(self.get_value(a)?);
        Ok(())
    }

    fn event_is_action(
        &mut self,
        e: WasmResource<Variant>,
        a: WasmResource<Variant>,
        m: bool,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, event_is_action)?;
        Ok(InputMap::singleton()
            .event_is_action_ex(self.get_object::<InputEvent>(e)?, self.get_value(a)?)
            .exact_match(m)
            .done())
    }

    fn get_actions(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, get_actions)?;
        self.set_into_var(InputMap::singleton().get_actions())
    }

    fn has_action(&mut self, a: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, has_action)?;
        Ok(InputMap::singleton().has_action(self.get_value(a)?))
    }

    fn load_from_project_settings(&mut self) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input_map, load_from_project_settings)?;
        InputMap::singleton().load_from_project_settings();
        Ok(())
    }
}
