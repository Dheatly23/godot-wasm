use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, Func, StoreContextMut};

use crate::wasm_instance::StoreData;

macro_rules! is_typecheck{
    ($(($i:ident, $n:literal) => $var:ident),* $(,)?) => {
        #[derive(Default)]
        pub struct Funcs {
            $($i: Option<Func>,)*
            not_null: Option<Func>,
            variant_type: Option<Func>,
        }

        impl Funcs {
            pub fn get_func<T>(&mut self, store: &mut StoreContextMut<'_, T>, name: &str) -> Option<Func>
            where
                T: AsRef<StoreData> + AsMut<StoreData>,
            {
                match name {
                    $(concat!($n, ".is") => Some(self.$i.get_or_insert_with(move || Func::wrap(store, |ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
                        match ctx.data().as_ref().get_registry()?.get(i as _) {
                            Some(v) if v.get_type() == VariantType::$var => Ok(1),
                            _ => Ok(0),
                        }
                    })).clone()),)*
                    "null.is_not" => Some(self.not_null.get_or_insert_with(move || Func::wrap(store, |ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
                        match ctx.data().as_ref().get_registry()?.get(i as _) {
                            Some(_) => Ok(1),
                            None => Ok(0),
                        }
                    })).clone()),
                    "variant_type" => Some(self.variant_type.get_or_insert_with(move || Func::wrap(store, |ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
                        match ctx.data().as_ref().get_registry()?.get(i as _) {
                            Some(v) => Ok(v.get_type() as _),
                            _ => Ok(0),
                        }
                    })).clone()),
                    _ => None,
                }
            }
        }
    };
}

is_typecheck! {
    (null, "null") => Nil,
    (r#bool, "bool") => Bool,
    (int, "int") => Int,
    (float, "float") => Float,
    (string, "string") => String,
    (vector2, "vector2") => Vector2,
    (vector2i, "vector2i") => Vector2i,
    (rect2, "rect2") => Rect2,
    (rect2i, "rect2i") => Rect2i,
    (vector3, "vector3") => Vector3,
    (vector3i, "vector3i") => Vector3i,
    (transform2d, "transform2d") => Transform2D,
    (vector4, "vector4") => Vector4,
    (vector4i, "vector4i") => Vector4i,
    (plane, "plane") => Plane,
    (quat, "quat") => Quaternion,
    (aabb, "aabb") => Aabb,
    (basis, "basis") => Basis,
    (transform3d, "transform3d") => Transform3D,
    (color, "color") => Color,
    (stringname, "stringname") => StringName,
    (nodepath, "nodepath") => NodePath,
    (rid, "rid") => Rid,
    (object, "object") => Object,
    (callabe, "callabe") => Callable,
    (signal, "signal") => Signal,
    (dictionary, "dictionary") => Dictionary,
    (array, "array") => Array,
    (byte_array, "byte_array") => PackedByteArray,
    (int32_array, "int32_array") => PackedInt32Array,
    (int64_array, "int64_array") => PackedInt64Array,
    (float32_array, "float32_array") => PackedFloat32Array,
    (float64_array, "float64_array") => PackedFloat64Array,
    (string_array, "string_array") => PackedStringArray,
    (vector2_array, "vector2_array") => PackedVector2Array,
    (vector3_array, "vector3_array") => PackedVector3Array,
    (color_array, "color_array") => PackedColorArray,
}
