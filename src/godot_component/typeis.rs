use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::bindgen::godot::core::typeis;
use crate::godot_component::GodotCtx;

impl typeis::Host for GodotCtx {
    fn var_type(&mut self, var: WasmResource<Variant>) -> AnyResult<typeis::VariantType> {
        Ok(match self.get_var_borrow(var)?.get_type() {
            VariantType::Bool => typeis::VariantType::Bool,
            VariantType::Int => typeis::VariantType::Int,
            VariantType::Float => typeis::VariantType::Float,
            VariantType::String => typeis::VariantType::String,
            VariantType::Vector2 => typeis::VariantType::Vector2,
            VariantType::Vector2i => typeis::VariantType::Vector2i,
            VariantType::Rect2 => typeis::VariantType::Rect2,
            VariantType::Rect2i => typeis::VariantType::Rect2i,
            VariantType::Vector3 => typeis::VariantType::Vector3,
            VariantType::Vector3i => typeis::VariantType::Vector3i,
            VariantType::Transform2D => typeis::VariantType::Transform2d,
            VariantType::Vector4 => typeis::VariantType::Vector4,
            VariantType::Vector4i => typeis::VariantType::Vector4i,
            VariantType::Plane => typeis::VariantType::Plane,
            VariantType::Quaternion => typeis::VariantType::Quaternion,
            VariantType::Aabb => typeis::VariantType::Aabb,
            VariantType::Basis => typeis::VariantType::Basis,
            VariantType::Transform3D => typeis::VariantType::Transform3d,
            VariantType::Projection => typeis::VariantType::Projection,
            VariantType::Color => typeis::VariantType::Color,
            VariantType::StringName => typeis::VariantType::Stringname,
            VariantType::NodePath => typeis::VariantType::Nodepath,
            VariantType::Rid => typeis::VariantType::Rid,
            VariantType::Object => typeis::VariantType::Object,
            VariantType::Callable => typeis::VariantType::Callable,
            VariantType::Signal => typeis::VariantType::Signal,
            VariantType::Dictionary => typeis::VariantType::Dictionary,
            VariantType::Array => typeis::VariantType::Array,
            VariantType::PackedByteArray => typeis::VariantType::ByteArray,
            VariantType::PackedInt32Array => typeis::VariantType::Int32Array,
            VariantType::PackedInt64Array => typeis::VariantType::Int64Array,
            VariantType::PackedFloat32Array => typeis::VariantType::Float32Array,
            VariantType::PackedFloat64Array => typeis::VariantType::Float64Array,
            VariantType::PackedStringArray => typeis::VariantType::StringArray,
            VariantType::PackedVector2Array => typeis::VariantType::Vector2Array,
            VariantType::PackedVector3Array => typeis::VariantType::Vector3Array,
            VariantType::PackedColorArray => typeis::VariantType::ColorArray,
            VariantType::Nil => unreachable!("Variant must not be nil"),
        })
    }

    fn is_bool(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Bool)
    }

    fn is_int(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Int)
    }

    fn is_float(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Float)
    }

    fn is_string(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::String)
    }

    fn is_vector2(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Vector2)
    }

    fn is_vector2i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Vector2i)
    }

    fn is_rect2(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Rect2)
    }

    fn is_rect2i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Rect2i)
    }

    fn is_vector3(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Vector3)
    }

    fn is_vector3i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Vector3i)
    }

    fn is_transform2d(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Transform2D)
    }

    fn is_vector4(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Vector4)
    }

    fn is_vector4i(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Vector4i)
    }

    fn is_plane(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Plane)
    }

    fn is_quaternion(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Quaternion)
    }

    fn is_aabb(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Aabb)
    }

    fn is_basis(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Basis)
    }

    fn is_transform3d(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Transform3D)
    }

    fn is_projection(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Projection)
    }

    fn is_color(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Color)
    }

    fn is_stringname(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::StringName)
    }

    fn is_nodepath(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::NodePath)
    }

    fn is_rid(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Rid)
    }

    fn is_object(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Object)
    }

    fn is_callable(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Callable)
    }

    fn is_signal(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Signal)
    }

    fn is_dictionary(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Dictionary)
    }

    fn is_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::Array)
    }

    fn is_byte_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedByteArray)
    }

    fn is_int32_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedInt32Array)
    }

    fn is_int64_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedInt64Array)
    }

    fn is_float32_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedFloat32Array)
    }

    fn is_float64_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedFloat64Array)
    }

    fn is_string_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedStringArray)
    }

    fn is_vector2_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedVector2Array)
    }

    fn is_vector3_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedVector3Array)
    }

    fn is_color_array(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.get_type() == VariantType::PackedColorArray)
    }
}
