use anyhow::Result as AnyResult;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut};

use crate::wasm_externref::externref_to_variant;
use crate::wasm_instance::StoreData;

macro_rules! is_typecheck{
    ($(($i:ident, $n:literal) => $var:ident),* $(,)?) => {
        #[derive(Default)]
        pub struct Funcs {
            $($i: Option<Func>,)*
            variant_type: Option<Func>,
        }

        impl Funcs {
            pub fn get_func<T>(&mut self, store: &mut StoreContextMut<'_, T>, name: &str) -> Option<Func>
            where
                T: AsRef<StoreData> + AsMut<StoreData>,
            {
                match name {
                    $(concat!($n, ".is") => Some(self.$i.get_or_insert_with(move || Func::wrap(store, |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
                        match externref_to_variant(&ctx, v)?.get_type() {
                            VariantType::$var => Ok(1),
                            _ => Ok(0),
                        }
                    })).clone()),)*
                    "variant_type" => Some(self.variant_type.get_or_insert_with(move || Func::wrap(store, |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
                        Ok(externref_to_variant(&ctx, v)?.get_type() as _)
                    })).clone()),
                    _ => None,
                }
            }
        }
    };
}

is_typecheck! {
    (r#bool, "bool") => Bool,
    (int, "int") => I64,
    (float, "float") => F64,
    (string, "string") => GodotString,
    (vector2, "vector2") => Vector2,
    (rect2, "rect2") => Rect2,
    (vector3, "vector3") => Vector3,
    (transform2d, "transform2d") => Transform2D,
    (plane, "plane") => Plane,
    (quat, "quat") => Quat,
    (aabb, "aabb") => Aabb,
    (basis, "basis") => Basis,
    (transform, "transform") => Transform,
    (color, "color") => Color,
    (nodepath, "nodepath") => NodePath,
    (rid, "rid") => Rid,
    (object, "object") => Object,
    (dictionary, "dictionary") => Dictionary,
    (array, "array") => VariantArray,
    (byte_array, "byte_array") => ByteArray,
    (int_array, "int_array") => Int32Array,
    (float_array, "float_array") => Float32Array,
    (string_array, "string_array") => StringArray,
    (vector2_array, "vector2_array") => Vector2Array,
    (vector3_array, "vector3_array") => Vector3Array,
    (color_array, "color_array") => ColorArray,
}
