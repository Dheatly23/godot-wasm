use anyhow::Error;
use gdnative::prelude::*;
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
