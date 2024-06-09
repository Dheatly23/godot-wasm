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
                            Some(v) => Ok(v.get_type().ord() as _),
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
    (null, "null") => NIL,
    (r#bool, "bool") => BOOL,
    (int, "int") => INT,
    (float, "float") => FLOAT,
    (string, "string") => STRING,
    (vector2, "vector2") => VECTOR2,
    (vector2i, "vector2i") => VECTOR2I,
    (rect2, "rect2") => RECT2,
    (rect2i, "rect2i") => RECT2I,
    (vector3, "vector3") => VECTOR3,
    (vector3i, "vector3i") => VECTOR3I,
    (transform2d, "transform2d") => TRANSFORM2D,
    (vector4, "vector4") => VECTOR4,
    (vector4i, "vector4i") => VECTOR4I,
    (plane, "plane") => PLANE,
    (quat, "quat") => QUATERNION,
    (aabb, "aabb") => AABB,
    (basis, "basis") => BASIS,
    (transform3d, "transform3d") => TRANSFORM3D,
    (color, "color") => COLOR,
    (stringname, "stringname") => STRING_NAME,
    (nodepath, "nodepath") => NODE_PATH,
    (rid, "rid") => RID,
    (object, "object") => OBJECT,
    (callabe, "callabe") => CALLABLE,
    (signal, "signal") => SIGNAL,
    (dictionary, "dictionary") => DICTIONARY,
    (array, "array") => ARRAY,
    (byte_array, "byte_array") => PACKED_BYTE_ARRAY,
    (int32_array, "int32_array") => PACKED_INT32_ARRAY,
    (int64_array, "int64_array") => PACKED_INT64_ARRAY,
    (float32_array, "float32_array") => PACKED_FLOAT32_ARRAY,
    (float64_array, "float64_array") => PACKED_FLOAT64_ARRAY,
    (string_array, "string_array") => PACKED_STRING_ARRAY,
    (vector2_array, "vector2_array") => PACKED_VECTOR2_ARRAY,
    (vector3_array, "vector3_array") => PACKED_VECTOR3_ARRAY,
    (color_array, "color_array") => PACKED_COLOR_ARRAY,
}
