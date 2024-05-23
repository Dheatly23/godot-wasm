use anyhow::{bail, Result as AnyResult};
use godot::engine::global::*;
use godot::engine::utilities::*;
use godot::engine::{ResourceLoader, ResourceSaver};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::bindgen::godot::core::typeis::VariantType as CompVarType;
use crate::godot_component::bindgen::godot::global::globalscope;
use crate::godot_component::{wrap_error, ErrorRes, GodotCtx};

pub fn from_joy_axis(v: globalscope::JoyAxis) -> JoyAxis {
    match v {
        globalscope::JoyAxis::LeftX => JoyAxis::LEFT_X,
        globalscope::JoyAxis::LeftY => JoyAxis::LEFT_Y,
        globalscope::JoyAxis::RightX => JoyAxis::RIGHT_X,
        globalscope::JoyAxis::RightY => JoyAxis::RIGHT_Y,
        globalscope::JoyAxis::TriggerLeft => JoyAxis::TRIGGER_LEFT,
        globalscope::JoyAxis::TriggerRight => JoyAxis::TRIGGER_RIGHT,
    }
}

pub fn from_joy_button(v: globalscope::JoyButton) -> JoyButton {
    match v {
        globalscope::JoyButton::A => JoyButton::A,
        globalscope::JoyButton::B => JoyButton::B,
        globalscope::JoyButton::X => JoyButton::X,
        globalscope::JoyButton::Y => JoyButton::Y,
        globalscope::JoyButton::Back => JoyButton::BACK,
        globalscope::JoyButton::Guide => JoyButton::GUIDE,
        globalscope::JoyButton::Start => JoyButton::START,
        globalscope::JoyButton::LeftStick => JoyButton::LEFT_STICK,
        globalscope::JoyButton::RightStick => JoyButton::RIGHT_STICK,
        globalscope::JoyButton::LeftShoulder => JoyButton::LEFT_SHOULDER,
        globalscope::JoyButton::RightShoulder => JoyButton::RIGHT_SHOULDER,
        globalscope::JoyButton::DpadUp => JoyButton::DPAD_UP,
        globalscope::JoyButton::DpadDown => JoyButton::DPAD_DOWN,
        globalscope::JoyButton::DpadLeft => JoyButton::DPAD_LEFT,
        globalscope::JoyButton::DpadRight => JoyButton::DPAD_RIGHT,
        globalscope::JoyButton::Misc1 => JoyButton::MISC1,
        globalscope::JoyButton::Paddle1 => JoyButton::PADDLE1,
        globalscope::JoyButton::Paddle2 => JoyButton::PADDLE2,
        globalscope::JoyButton::Paddle3 => JoyButton::PADDLE3,
        globalscope::JoyButton::Paddle4 => JoyButton::PADDLE4,
        globalscope::JoyButton::Touchpad => JoyButton::TOUCHPAD,
    }
}

pub fn from_mouse_button(v: globalscope::MouseButton) -> MouseButton {
    match v {
        globalscope::MouseButton::None => MouseButton::NONE,
        globalscope::MouseButton::Left => MouseButton::LEFT,
        globalscope::MouseButton::Right => MouseButton::RIGHT,
        globalscope::MouseButton::Middle => MouseButton::MIDDLE,
        globalscope::MouseButton::WheelUp => MouseButton::WHEEL_UP,
        globalscope::MouseButton::WheelDown => MouseButton::WHEEL_DOWN,
        globalscope::MouseButton::WheelLeft => MouseButton::WHEEL_LEFT,
        globalscope::MouseButton::WheelRight => MouseButton::WHEEL_RIGHT,
        globalscope::MouseButton::Xbutton1 => MouseButton::XBUTTON1,
        globalscope::MouseButton::Xbutton2 => MouseButton::XBUTTON2,
    }
}

#[allow(dead_code)]
pub fn from_mouse_button_mask(v: globalscope::MouseButtonMask) -> MouseButtonMask {
    (if v.contains(globalscope::MouseButtonMask::LEFT) {
        MouseButtonMask::LEFT
    } else {
        MouseButtonMask::default()
    }) | (if v.contains(globalscope::MouseButtonMask::RIGHT) {
        MouseButtonMask::RIGHT
    } else {
        MouseButtonMask::default()
    }) | (if v.contains(globalscope::MouseButtonMask::MIDDLE) {
        MouseButtonMask::MIDDLE
    } else {
        MouseButtonMask::default()
    }) | (if v.contains(globalscope::MouseButtonMask::MB_XBUTTON1) {
        MouseButtonMask::MB_XBUTTON1
    } else {
        MouseButtonMask::default()
    }) | (if v.contains(globalscope::MouseButtonMask::MB_XBUTTON2) {
        MouseButtonMask::MB_XBUTTON2
    } else {
        MouseButtonMask::default()
    })
}

pub fn to_mouse_button_mask(v: MouseButtonMask) -> globalscope::MouseButtonMask {
    (if v.is_set(MouseButtonMask::LEFT) {
        globalscope::MouseButtonMask::LEFT
    } else {
        globalscope::MouseButtonMask::empty()
    }) | (if v.is_set(MouseButtonMask::RIGHT) {
        globalscope::MouseButtonMask::RIGHT
    } else {
        globalscope::MouseButtonMask::empty()
    }) | (if v.is_set(MouseButtonMask::MIDDLE) {
        globalscope::MouseButtonMask::MIDDLE
    } else {
        globalscope::MouseButtonMask::empty()
    }) | (if v.is_set(MouseButtonMask::MB_XBUTTON1) {
        globalscope::MouseButtonMask::MB_XBUTTON1
    } else {
        globalscope::MouseButtonMask::empty()
    }) | (if v.is_set(MouseButtonMask::MB_XBUTTON2) {
        globalscope::MouseButtonMask::MB_XBUTTON2
    } else {
        globalscope::MouseButtonMask::empty()
    })
}

filter_macro! {method [
    print -> "print",
    print_rich -> "print-rich",
    printerr -> "printerr",
    push_error -> "push-error",
    push_warning -> "push-warning",
    bytes_to_var -> "bytes-to-var",
    bytes_to_var_with_objects -> "bytes-to-var-with-objects",
    var_to_bytes -> "var-to-bytes",
    var_to_bytes_with_objects -> "var-to-bytes-with-objects",
    var_to_str -> "var-to-str",
    str_to_var -> "str-to-var",
    weakref -> "weakref",
    is_instance_valid -> "is-instance-valid",
    is_instance_id_valid -> "is-instance-id-valid",
    is_same -> "is-same",
    type_convert -> "type-convert",
    rand_from_seed -> "rand-from-seed",
    randf -> "randf",
    randf_range -> "randf-range",
    randfn -> "randfn",
    randi -> "randi",
    randi_range -> "randi-range",
    randomize -> "randomize",
    seed -> "seed",
    load -> "load",
    save -> "save",
]}

impl globalscope::Host for GodotCtx {
    fn print(&mut self, s: String) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, print)?;
        godot::engine::utilities::print(s.to_variant(), &[]);
        Ok(())
    }

    fn print_rich(&mut self, s: String) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, print_rich)?;
        godot::engine::utilities::print_rich(s.to_variant(), &[]);
        Ok(())
    }

    fn printerr(&mut self, s: String) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, printerr)?;
        printerr(s.to_variant(), &[]);
        Ok(())
    }

    fn push_error(&mut self, s: String) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, push_error)?;
        push_error(s.to_variant(), &[]);
        Ok(())
    }

    fn push_warning(&mut self, s: String) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, push_warning)?;
        push_warning(s.to_variant(), &[]);
        Ok(())
    }

    fn bytes_to_var(
        &mut self,
        b: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, bytes_to_var)?;
        let v = bytes_to_var(self.get_value(b)?);
        self.set_var(v)
    }

    fn bytes_to_var_with_objects(
        &mut self,
        b: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, bytes_to_var_with_objects)?;
        let v = bytes_to_var_with_objects(self.get_value(b)?);
        self.set_var(v)
    }

    fn var_to_bytes(
        &mut self,
        v: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, var_to_bytes)?;
        let b = var_to_bytes(self.maybe_get_var(v)?);
        self.set_into_var(b)
    }

    fn var_to_bytes_with_objects(
        &mut self,
        v: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, var_to_bytes_with_objects)?;
        let b = var_to_bytes_with_objects(self.maybe_get_var(v)?);
        self.set_into_var(b)
    }

    fn var_to_str(&mut self, v: Option<WasmResource<Variant>>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, var_to_str)?;
        let s = var_to_str(self.maybe_get_var(v)?);
        self.set_into_var(s)
    }

    fn str_to_var(&mut self, s: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, str_to_var)?;
        let v = str_to_var(self.get_value(s)?);
        self.set_var(v)
    }

    fn weakref(&mut self, v: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, weakref)?;
        let v = weakref(self.get_var(v)?);
        self.set_var(v)
    }

    fn is_instance_valid(&mut self, v: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, is_instance_valid)?;
        Ok(is_instance_valid(self.get_var(v)?))
    }

    fn is_instance_id_valid(&mut self, id: u64) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, is_instance_id_valid)?;
        Ok(is_instance_id_valid(id as _))
    }

    fn is_same(&mut self, a: WasmResource<Variant>, b: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, is_same)?;
        Ok(is_same(self.get_var(a)?, self.get_var(b)?))
    }

    fn type_convert(
        &mut self,
        v: WasmResource<Variant>,
        t: CompVarType,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, type_convert)?;
        let v = self.get_var(v)?;
        let t = match t {
            CompVarType::Bool => VariantType::Bool,
            CompVarType::Int => VariantType::Int,
            CompVarType::Float => VariantType::Float,
            CompVarType::String => VariantType::String,
            CompVarType::Vector2 => VariantType::Vector2,
            CompVarType::Vector2i => VariantType::Vector2i,
            CompVarType::Rect2 => VariantType::Rect2,
            CompVarType::Rect2i => VariantType::Rect2i,
            CompVarType::Vector3 => VariantType::Vector3,
            CompVarType::Vector3i => VariantType::Vector3i,
            CompVarType::Transform2d => VariantType::Transform2D,
            CompVarType::Vector4 => VariantType::Vector4,
            CompVarType::Vector4i => VariantType::Vector4i,
            CompVarType::Plane => VariantType::Plane,
            CompVarType::Quaternion => VariantType::Quaternion,
            CompVarType::Aabb => VariantType::Aabb,
            CompVarType::Basis => VariantType::Basis,
            CompVarType::Transform3d => VariantType::Transform3D,
            CompVarType::Projection => VariantType::Projection,
            CompVarType::Color => VariantType::Color,
            CompVarType::Stringname => VariantType::StringName,
            CompVarType::Nodepath => VariantType::NodePath,
            CompVarType::Rid => VariantType::Rid,
            CompVarType::Object => VariantType::Object,
            CompVarType::Callable => VariantType::Callable,
            CompVarType::Signal => VariantType::Signal,
            CompVarType::Dictionary => VariantType::Dictionary,
            CompVarType::Array => VariantType::Array,
            CompVarType::ByteArray => VariantType::PackedByteArray,
            CompVarType::Int32Array => VariantType::PackedInt32Array,
            CompVarType::Int64Array => VariantType::PackedInt64Array,
            CompVarType::Float32Array => VariantType::PackedFloat32Array,
            CompVarType::Float64Array => VariantType::PackedFloat64Array,
            CompVarType::StringArray => VariantType::PackedStringArray,
            CompVarType::Vector2Array => VariantType::PackedVector2Array,
            CompVarType::Vector3Array => VariantType::PackedVector3Array,
            CompVarType::ColorArray => VariantType::PackedColorArray,
        } as i64;
        let r = type_convert(v, t);
        assert!(!r.is_nil(), "Value should be nonnull");
        self.set_var(r).map(|v| v.unwrap())
    }

    fn rand_from_seed(&mut self, seed: u64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, rand_from_seed)?;
        self.set_into_var(rand_from_seed(seed as _))
    }

    fn randf(&mut self) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, randf)?;
        Ok(randf())
    }

    fn randf_range(&mut self, from: f64, to: f64) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, randf_range)?;
        Ok(randf_range(from, to))
    }

    fn randfn(&mut self, mean: f64, deviation: f64) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, randfn)?;
        Ok(randfn(mean, deviation))
    }

    fn randi(&mut self) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, randi)?;
        Ok(randi())
    }

    fn randi_range(&mut self, from: i64, to: i64) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, randi_range)?;
        Ok(randi_range(from, to))
    }

    fn randomize(&mut self) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, randomize)?;
        randomize();
        Ok(())
    }

    fn seed(&mut self, s: u64) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, seed)?;
        seed(s as _);
        Ok(())
    }

    fn load(&mut self, path: String) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, load)?;
        match ResourceLoader::singleton().load((&path).into()) {
            Some(v) => self.set_into_var(v),
            None => bail!("Cannot load resource {path}"),
        }
    }

    fn save(&mut self, res: WasmResource<Variant>, path: String) -> ErrorRes {
        filter_macro!(filter self.filter.as_ref(), godot_global, globalscope, save)?;
        wrap_error(
            ResourceSaver::singleton()
                .save_ex(self.get_value(res)?)
                .path((&path).into())
                .done(),
        )
    }
}
