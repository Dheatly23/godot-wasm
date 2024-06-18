use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::bindgen::godot::core::typeis;

filter_macro! {method [
    var_type -> "var-type",
    is_bool -> "is-bool",
    is_int -> "is-int",
    is_float -> "is-float",
    is_string -> "is-string",
    is_vector2 -> "is-vector2",
    is_vector2i -> "is-vector2i",
    is_rect2 -> "is-rect2",
    is_rect2i -> "is-rect2i",
    is_vector3 -> "is-vector3",
    is_vector3i -> "is-vector3i",
    is_transform2d -> "is-transform2d",
    is_vector4 -> "is-vector4",
    is_vector4i -> "is-vector4i",
    is_plane -> "is-plane",
    is_quaternion -> "is-quaternion",
    is_aabb -> "is-aabb",
    is_basis -> "is-basis",
    is_transform3d -> "is-transform3d",
    is_projection -> "is-projection",
    is_color -> "is-color",
    is_stringname -> "is-stringname",
    is_nodepath -> "is-nodepath",
    is_rid -> "is-rid",
    is_object -> "is-object",
    is_callable -> "is-callable",
    is_signal -> "is-signal",
    is_dictionary -> "is-dictionary",
    is_array -> "is-array",
    is_byte_array -> "is-byte-array",
    is_int32_array -> "is-int32-array",
    is_int64_array -> "is-int64-array",
    is_float32_array -> "is-float32-array",
    is_float64_array -> "is-float64-array",
    is_string_array -> "is-string-array",
    is_vector2_array -> "is-vector2-array",
    is_vector3_array -> "is-vector3-array",
    is_color_array -> "is-color-array",
]}

impl typeis::Host for crate::godot_component::GodotCtx {
    fn var_type(&mut self, var: WasmResource<Variant>) -> AnyResult<typeis::VariantType> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, var_type)?;
        Ok(match self.get_var_borrow(var)?.get_type() {
            VariantType::BOOL => typeis::VariantType::Bool,
            VariantType::INT => typeis::VariantType::Int,
            VariantType::FLOAT => typeis::VariantType::Float,
            VariantType::STRING => typeis::VariantType::String,
            VariantType::VECTOR2 => typeis::VariantType::Vector2,
            VariantType::VECTOR2I => typeis::VariantType::Vector2i,
            VariantType::RECT2 => typeis::VariantType::Rect2,
            VariantType::RECT2I => typeis::VariantType::Rect2i,
            VariantType::VECTOR3 => typeis::VariantType::Vector3,
            VariantType::VECTOR3I => typeis::VariantType::Vector3i,
            VariantType::TRANSFORM2D => typeis::VariantType::Transform2d,
            VariantType::VECTOR4 => typeis::VariantType::Vector4,
            VariantType::VECTOR4I => typeis::VariantType::Vector4i,
            VariantType::PLANE => typeis::VariantType::Plane,
            VariantType::QUATERNION => typeis::VariantType::Quaternion,
            VariantType::AABB => typeis::VariantType::Aabb,
            VariantType::BASIS => typeis::VariantType::Basis,
            VariantType::TRANSFORM3D => typeis::VariantType::Transform3d,
            VariantType::PROJECTION => typeis::VariantType::Projection,
            VariantType::COLOR => typeis::VariantType::Color,
            VariantType::STRING_NAME => typeis::VariantType::Stringname,
            VariantType::NODE_PATH => typeis::VariantType::Nodepath,
            VariantType::RID => typeis::VariantType::Rid,
            VariantType::OBJECT => typeis::VariantType::Object,
            VariantType::CALLABLE => typeis::VariantType::Callable,
            VariantType::SIGNAL => typeis::VariantType::Signal,
            VariantType::DICTIONARY => typeis::VariantType::Dictionary,
            VariantType::ARRAY => typeis::VariantType::Array,
            VariantType::PACKED_BYTE_ARRAY => typeis::VariantType::ByteArray,
            VariantType::PACKED_INT32_ARRAY => typeis::VariantType::Int32Array,
            VariantType::PACKED_INT64_ARRAY => typeis::VariantType::Int64Array,
            VariantType::PACKED_FLOAT32_ARRAY => typeis::VariantType::Float32Array,
            VariantType::PACKED_FLOAT64_ARRAY => typeis::VariantType::Float64Array,
            VariantType::PACKED_STRING_ARRAY => typeis::VariantType::StringArray,
            VariantType::PACKED_VECTOR2_ARRAY => typeis::VariantType::Vector2Array,
            VariantType::PACKED_VECTOR3_ARRAY => typeis::VariantType::Vector3Array,
            VariantType::PACKED_COLOR_ARRAY => typeis::VariantType::ColorArray,
            VariantType::NIL => unreachable!("Variant must not be nil"),
            t => unreachable!("Unhandleable type {t:?}"),
        })
    }

    fn is_bool(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_bool)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::BOOL)
    }

    fn is_int(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_int)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::INT)
    }

    fn is_float(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_float)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::FLOAT)
    }

    fn is_string(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_string)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::STRING)
    }

    fn is_vector2(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector2)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::VECTOR2)
    }

    fn is_vector2i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector2i)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::VECTOR2I)
    }

    fn is_rect2(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_rect2)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::RECT2)
    }

    fn is_rect2i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_rect2i)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::RECT2I)
    }

    fn is_vector3(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector3)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::VECTOR3)
    }

    fn is_vector3i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector3i)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::VECTOR3I)
    }

    fn is_transform2d(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_transform2d)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::TRANSFORM2D)
    }

    fn is_vector4(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector4)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::VECTOR4)
    }

    fn is_vector4i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector4i)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::VECTOR4I)
    }

    fn is_plane(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_plane)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PLANE)
    }

    fn is_quaternion(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_quaternion)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::QUATERNION)
    }

    fn is_aabb(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_aabb)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::AABB)
    }

    fn is_basis(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_basis)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::BASIS)
    }

    fn is_transform3d(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_transform3d)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::TRANSFORM3D)
    }

    fn is_projection(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_projection)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PROJECTION)
    }

    fn is_color(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_color)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::COLOR)
    }

    fn is_stringname(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_stringname)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::STRING_NAME)
    }

    fn is_nodepath(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_nodepath)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::NODE_PATH)
    }

    fn is_rid(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_rid)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::RID)
    }

    fn is_object(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_object)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::OBJECT)
    }

    fn is_callable(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_callable)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::CALLABLE)
    }

    fn is_signal(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_signal)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::SIGNAL)
    }

    fn is_dictionary(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_dictionary)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::DICTIONARY)
    }

    fn is_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::ARRAY)
    }

    fn is_byte_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_byte_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_BYTE_ARRAY)
    }

    fn is_int32_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_int32_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_INT32_ARRAY)
    }

    fn is_int64_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_int64_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_INT64_ARRAY)
    }

    fn is_float32_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_float32_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_FLOAT32_ARRAY)
    }

    fn is_float64_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_float64_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_FLOAT64_ARRAY)
    }

    fn is_string_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_string_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_STRING_ARRAY)
    }

    fn is_vector2_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector2_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_VECTOR2_ARRAY)
    }

    fn is_vector3_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_vector3_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_VECTOR3_ARRAY)
    }

    fn is_color_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, typeis, is_color_array)?;
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PACKED_COLOR_ARRAY)
    }
}
