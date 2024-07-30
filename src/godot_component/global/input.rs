use anyhow::Result as AnyResult;
use godot::classes::input::{CursorShape, MouseMode};
use godot::classes::{Input, InputEvent};
use godot::global::Key;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::bindgen::godot::core::primitive;
use crate::godot_component::bindgen::godot::global::globalscope::{
    JoyAxis, JoyButton, MouseButton, MouseButtonMask,
};
use crate::godot_component::bindgen::godot::global::input;
use crate::godot_component::global::globalscope::{
    from_joy_axis, from_joy_button, from_mouse_button, to_mouse_button_mask,
};
use crate::{bail_with_site, filter_macro};

fn from_cursor_shape(v: input::CursorShape) -> CursorShape {
    match v {
        input::CursorShape::Arrow => CursorShape::ARROW,
        input::CursorShape::Ibeam => CursorShape::IBEAM,
        input::CursorShape::PointingHand => CursorShape::POINTING_HAND,
        input::CursorShape::Cross => CursorShape::CROSS,
        input::CursorShape::Wait => CursorShape::WAIT,
        input::CursorShape::Busy => CursorShape::BUSY,
        input::CursorShape::Drag => CursorShape::DRAG,
        input::CursorShape::CanDrop => CursorShape::CAN_DROP,
        input::CursorShape::Forbidden => CursorShape::FORBIDDEN,
        input::CursorShape::Vsize => CursorShape::VSIZE,
        input::CursorShape::Hsize => CursorShape::HSIZE,
        input::CursorShape::Bdiagsize => CursorShape::BDIAGSIZE,
        input::CursorShape::Fdiagsize => CursorShape::FDIAGSIZE,
        input::CursorShape::Move => CursorShape::MOVE,
        input::CursorShape::Vsplit => CursorShape::VSPLIT,
        input::CursorShape::Hsplit => CursorShape::HSPLIT,
        input::CursorShape::Help => CursorShape::HELP,
    }
}

fn from_key(k: i32) -> AnyResult<Key> {
    match Key::try_from_ord(k) {
        Some(v) => Ok(v),
        None => bail_with_site!("Unknown key {k}"),
    }
}

filter_macro! {method [
    singleton -> "singleton",
    get_mouse_mode -> "get-mouse-mode",
    set_mouse_mode -> "set-mouse-mode",
    is_using_accumulated_input -> "is-using-accumulated-input",
    set_use_accumulated_input -> "set-use-accumulated-input",
    action_press -> "action-press",
    action_release -> "action-release",
    add_joy_mapping -> "add-joy-mapping",
    flush_buffered_events -> "flush-buffered-events",
    get_accelerometer -> "get-accelerometer",
    get_action_raw_strength -> "get-action-raw-strength",
    get_action_strength -> "get-action-strength",
    get_axis -> "get-axis",
    get_connected_joypads -> "get-connected-joypads",
    get_current_cursor_shape -> "get-current-cursor-shape",
    get_gravity -> "get-gravity",
    get_gyroscope -> "get-gyroscope",
    get_joy_axis -> "get-joy-axis",
    get_joy_guid -> "get-joy-guid",
    get_joy_info -> "get-joy-info",
    get_joy_name -> "get-joy-name",
    get_joy_vibration_duration -> "get-joy-vibration-duration",
    get_joy_vibration_strength -> "get-joy-vibration-strength",
    get_last_mouse_velocity -> "get-last-mouse-velocity",
    get_magnetometer -> "get-magnetometer",
    get_mouse_button_mask -> "get-mouse-button-mask",
    get_vector -> "get-vector",
    is_action_just_pressed -> "is-action-just-pressed",
    is_action_just_released -> "is-action-just-released",
    is_action_pressed -> "is-action-pressed",
    is_anything_pressed -> "is-anything-pressed",
    is_joy_button_pressed -> "is-joy-button-pressed",
    is_joy_known -> "is-joy-known",
    is_key_label_pressed -> "is-key-label-pressed",
    is_key_pressed -> "is-key-pressed",
    is_mouse_button_pressed -> "is-mouse-button-pressed",
    is_physical_key_pressed -> "is-physical-key-pressed",
    parse_input_even -> "parse-input-even",
    remove_joy_mapping -> "remove-joy-mapping",
    set_accelerometer -> "set-accelerometer",
    set_custom_mouse_cursor -> "set-custom-mouse-cursor",
    set_default_cursor_shape -> "set-default-cursor-shape",
    set_gravity -> "set-gravity",
    set_gyroscope -> "set-gyroscope",
    set_magnetometer -> "set-magnetometer",
    should_ignore_device -> "should-ignore-device",
    start_joy_vibration -> "start-joy-vibration",
    stop_joy_vibration -> "stop-joy-vibration",
    vibrate_handheld -> "vibrate-handheld",
    warp_mouse -> "warp-mouse",
]}

impl input::Host for crate::godot_component::GodotCtx {
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, singleton)?;
        self.set_into_var(Input::singleton())
    }

    fn get_mouse_mode(&mut self) -> AnyResult<input::MouseMode> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_mouse_mode)?;
        Ok(match Input::singleton().get_mouse_mode() {
            MouseMode::VISIBLE => input::MouseMode::Visible,
            MouseMode::HIDDEN => input::MouseMode::Hidden,
            MouseMode::CAPTURED => input::MouseMode::Captured,
            MouseMode::CONFINED => input::MouseMode::Confined,
            MouseMode::CONFINED_HIDDEN => input::MouseMode::ConfinedHidden,
            v => bail_with_site!("Unknown mouse mode {v:?}"),
        })
    }

    fn set_mouse_mode(&mut self, v: input::MouseMode) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_mouse_mode)?;
        Input::singleton().set_mouse_mode(match v {
            input::MouseMode::Visible => MouseMode::VISIBLE,
            input::MouseMode::Hidden => MouseMode::HIDDEN,
            input::MouseMode::Captured => MouseMode::CAPTURED,
            input::MouseMode::Confined => MouseMode::CONFINED,
            input::MouseMode::ConfinedHidden => MouseMode::CONFINED_HIDDEN,
        });
        Ok(())
    }

    fn is_using_accumulated_input(&mut self) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_using_accumulated_input)?;
        Ok(Input::singleton().is_using_accumulated_input())
    }

    fn set_use_accumulated_input(&mut self, v: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_use_accumulated_input)?;
        Input::singleton().set_use_accumulated_input(v);
        Ok(())
    }

    fn action_press(&mut self, v: WasmResource<Variant>, s: f32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, action_press)?;
        Input::singleton()
            .action_press_ex(self.get_value(v)?)
            .strength(s)
            .done();
        Ok(())
    }

    fn action_release(&mut self, v: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, action_release)?;
        Input::singleton().action_release(self.get_value(v)?);
        Ok(())
    }

    fn add_joy_mapping(&mut self, v: WasmResource<Variant>, u: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, add_joy_mapping)?;
        Input::singleton()
            .add_joy_mapping_ex(self.get_value(v)?)
            .update_existing(u)
            .done();
        Ok(())
    }

    fn flush_buffered_events(&mut self) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, flush_buffered_events)?;
        Input::singleton().flush_buffered_events();
        Ok(())
    }

    fn get_accelerometer(&mut self) -> AnyResult<primitive::Vector3> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_accelerometer)?;
        let Vector3 { x, y, z } = Input::singleton().get_accelerometer();
        Ok(primitive::Vector3 { x, y, z })
    }

    fn get_action_raw_strength(&mut self, v: WasmResource<Variant>, m: bool) -> AnyResult<f32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_action_raw_strength)?;
        Ok(Input::singleton()
            .get_action_raw_strength_ex(self.get_value(v)?)
            .exact_match(m)
            .done())
    }

    fn get_action_strength(&mut self, v: WasmResource<Variant>, m: bool) -> AnyResult<f32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_action_strength)?;
        Ok(Input::singleton()
            .get_action_strength_ex(self.get_value(v)?)
            .exact_match(m)
            .done())
    }

    fn get_axis(&mut self, n: WasmResource<Variant>, p: WasmResource<Variant>) -> AnyResult<f32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_axis)?;
        Ok(Input::singleton().get_axis(self.get_value(n)?, self.get_value(p)?))
    }

    fn get_connected_joypads(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_connected_joypads)?;
        self.set_into_var(Input::singleton().get_connected_joypads())
    }

    fn get_current_cursor_shape(&mut self) -> AnyResult<input::CursorShape> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_current_cursor_shape)?;
        Ok(match Input::singleton().get_current_cursor_shape() {
            CursorShape::ARROW => input::CursorShape::Arrow,
            CursorShape::IBEAM => input::CursorShape::Ibeam,
            CursorShape::POINTING_HAND => input::CursorShape::PointingHand,
            CursorShape::CROSS => input::CursorShape::Cross,
            CursorShape::WAIT => input::CursorShape::Wait,
            CursorShape::BUSY => input::CursorShape::Busy,
            CursorShape::DRAG => input::CursorShape::Drag,
            CursorShape::CAN_DROP => input::CursorShape::CanDrop,
            CursorShape::FORBIDDEN => input::CursorShape::Forbidden,
            CursorShape::VSIZE => input::CursorShape::Vsize,
            CursorShape::HSIZE => input::CursorShape::Hsize,
            CursorShape::BDIAGSIZE => input::CursorShape::Bdiagsize,
            CursorShape::FDIAGSIZE => input::CursorShape::Fdiagsize,
            CursorShape::MOVE => input::CursorShape::Move,
            CursorShape::VSPLIT => input::CursorShape::Vsplit,
            CursorShape::HSPLIT => input::CursorShape::Hsplit,
            CursorShape::HELP => input::CursorShape::Help,
            v => bail_with_site!("Unknown cursor shape {v:?}"),
        })
    }

    fn get_gravity(&mut self) -> AnyResult<primitive::Vector3> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_gravity)?;
        let Vector3 { x, y, z } = Input::singleton().get_gravity();
        Ok(primitive::Vector3 { x, y, z })
    }

    fn get_gyroscope(&mut self) -> AnyResult<primitive::Vector3> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_gyroscope)?;
        let Vector3 { x, y, z } = Input::singleton().get_gyroscope();
        Ok(primitive::Vector3 { x, y, z })
    }

    fn get_joy_axis(&mut self, d: i32, a: JoyAxis) -> AnyResult<f32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_joy_axis)?;
        Ok(Input::singleton().get_joy_axis(d, from_joy_axis(a)))
    }

    fn get_joy_guid(&mut self, d: i32) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_joy_guid)?;
        self.set_into_var(Input::singleton().get_joy_guid(d))
    }

    fn get_joy_info(&mut self, d: i32) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_joy_info)?;
        self.set_into_var(Input::singleton().get_joy_info(d))
    }

    fn get_joy_name(&mut self, d: i32) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_joy_name)?;
        self.set_into_var(Input::singleton().get_joy_name(d))
    }

    fn get_joy_vibration_duration(&mut self, d: i32) -> AnyResult<f32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_joy_vibration_duration)?;
        Ok(Input::singleton().get_joy_vibration_duration(d))
    }

    fn get_joy_vibration_strength(&mut self, d: i32) -> AnyResult<primitive::Vector2> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_joy_vibration_strength)?;
        let Vector2 { x, y } = Input::singleton().get_joy_vibration_strength(d);
        Ok(primitive::Vector2 { x, y })
    }

    fn get_last_mouse_velocity(&mut self) -> AnyResult<primitive::Vector2> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_last_mouse_velocity)?;
        let Vector2 { x, y } = Input::singleton().get_last_mouse_velocity();
        Ok(primitive::Vector2 { x, y })
    }

    fn get_magnetometer(&mut self) -> AnyResult<primitive::Vector3> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_magnetometer)?;
        let Vector3 { x, y, z } = Input::singleton().get_magnetometer();
        Ok(primitive::Vector3 { x, y, z })
    }

    fn get_mouse_button_mask(&mut self) -> AnyResult<MouseButtonMask> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_mouse_button_mask)?;
        Ok(to_mouse_button_mask(
            Input::singleton().get_mouse_button_mask(),
        ))
    }

    fn get_vector(
        &mut self,
        nx: WasmResource<Variant>,
        px: WasmResource<Variant>,
        ny: WasmResource<Variant>,
        py: WasmResource<Variant>,
        d: f32,
    ) -> AnyResult<primitive::Vector2> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, get_vector)?;
        let Vector2 { x, y } = Input::singleton()
            .get_vector_ex(
                self.get_value(nx)?,
                self.get_value(px)?,
                self.get_value(ny)?,
                self.get_value(py)?,
            )
            .deadzone(d)
            .done();
        Ok(primitive::Vector2 { x, y })
    }

    fn is_action_just_pressed(&mut self, a: WasmResource<Variant>, e: bool) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_action_just_pressed)?;
        Ok(Input::singleton()
            .is_action_just_pressed_ex(self.get_value(a)?)
            .exact_match(e)
            .done())
    }

    fn is_action_just_released(&mut self, a: WasmResource<Variant>, e: bool) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_action_just_released)?;
        Ok(Input::singleton()
            .is_action_just_released_ex(self.get_value(a)?)
            .exact_match(e)
            .done())
    }

    fn is_action_pressed(&mut self, a: WasmResource<Variant>, e: bool) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_action_pressed)?;
        Ok(Input::singleton()
            .is_action_pressed_ex(self.get_value(a)?)
            .exact_match(e)
            .done())
    }

    fn is_anything_pressed(&mut self) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_anything_pressed)?;
        Ok(Input::singleton().is_anything_pressed())
    }

    fn is_joy_button_pressed(&mut self, d: i32, b: JoyButton) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_joy_button_pressed)?;
        Ok(Input::singleton().is_joy_button_pressed(d, from_joy_button(b)))
    }

    fn is_joy_known(&mut self, d: i32) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_joy_known)?;
        Ok(Input::singleton().is_joy_known(d))
    }

    fn is_key_label_pressed(&mut self, k: i32) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_key_label_pressed)?;
        Ok(Input::singleton().is_key_label_pressed(from_key(k)?))
    }

    fn is_key_pressed(&mut self, k: i32) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_key_pressed)?;
        Ok(Input::singleton().is_key_pressed(from_key(k)?))
    }

    fn is_mouse_button_pressed(&mut self, b: MouseButton) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_mouse_button_pressed)?;
        Ok(Input::singleton().is_mouse_button_pressed(from_mouse_button(b)))
    }

    fn is_physical_key_pressed(&mut self, k: i32) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, is_physical_key_pressed)?;
        Ok(Input::singleton().is_physical_key_pressed(from_key(k)?))
    }

    fn parse_input_even(&mut self, v: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, parse_input_even)?;
        Input::singleton().parse_input_event(self.get_object::<InputEvent>(v)?);
        Ok(())
    }

    fn remove_joy_mapping(&mut self, v: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, remove_joy_mapping)?;
        Input::singleton().remove_joy_mapping(self.get_value(v)?);
        Ok(())
    }

    fn set_accelerometer(
        &mut self,
        primitive::Vector3 { x, y, z }: primitive::Vector3,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_accelerometer)?;
        Input::singleton().set_accelerometer(Vector3 { x, y, z });
        Ok(())
    }

    fn set_custom_mouse_cursor(
        &mut self,
        i: WasmResource<Variant>,
        s: input::CursorShape,
        primitive::Vector2 { x, y }: primitive::Vector2,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_custom_mouse_cursor)?;
        Input::singleton()
            .set_custom_mouse_cursor_ex(self.get_object::<Resource>(i)?)
            .shape(from_cursor_shape(s))
            .hotspot(Vector2 { x, y })
            .done();
        Ok(())
    }

    fn set_default_cursor_shape(&mut self, s: input::CursorShape) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_default_cursor_shape)?;
        Input::singleton()
            .set_default_cursor_shape_ex()
            .shape(from_cursor_shape(s))
            .done();
        Ok(())
    }

    fn set_gravity(&mut self, primitive::Vector3 { x, y, z }: primitive::Vector3) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_gravity)?;
        Input::singleton().set_gravity(Vector3 { x, y, z });
        Ok(())
    }

    fn set_gyroscope(
        &mut self,
        primitive::Vector3 { x, y, z }: primitive::Vector3,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_gyroscope)?;
        Input::singleton().set_gyroscope(Vector3 { x, y, z });
        Ok(())
    }

    fn set_magnetometer(
        &mut self,
        primitive::Vector3 { x, y, z }: primitive::Vector3,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, set_magnetometer)?;
        Input::singleton().set_magnetometer(Vector3 { x, y, z });
        Ok(())
    }

    fn should_ignore_device(&mut self, v: i32, p: i32) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, should_ignore_device)?;
        Ok(Input::singleton().should_ignore_device(v, p))
    }

    fn start_joy_vibration(&mut self, d: i32, w: f32, s: f32, t: f32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, start_joy_vibration)?;
        Input::singleton()
            .start_joy_vibration_ex(d, w, s)
            .duration(t)
            .done();
        Ok(())
    }

    fn stop_joy_vibration(&mut self, d: i32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, stop_joy_vibration)?;
        Input::singleton().stop_joy_vibration(d);
        Ok(())
    }

    fn vibrate_handheld(&mut self, t: i32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, vibrate_handheld)?;
        Input::singleton()
            .vibrate_handheld_ex()
            .duration_ms(t)
            .done();
        Ok(())
    }

    fn warp_mouse(&mut self, primitive::Vector2 { x, y }: primitive::Vector2) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, input, warp_mouse)?;
        Input::singleton().warp_mouse(Vector2 { x, y });
        Ok(())
    }
}
