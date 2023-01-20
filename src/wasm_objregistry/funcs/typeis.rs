use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, Linker};

use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

macro_rules! is_typecheck {
    ($linker:ident, $(($name:literal => $var:ident)),* $(,)?) => {$(
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".is"),
            |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                match ctx.data().get_registry()?.get(i as _) {
                    Some(v) if v.get_type() == VariantType::$var => Ok(1),
                    _ => Ok(0),
                }
            }
        ).unwrap();
    )*};
}

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "null.is_not",
            |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                match ctx.data().get_registry()?.get(i as _) {
                    Some(_) => Ok(1),
                    None => Ok(0),
                }
            },
        )
        .unwrap();

    is_typecheck!(
        linker,
        ("null" => Nil),
        ("bool" => Bool),
        ("int" => I64),
        ("float" => F64),
        ("string" => GodotString),
        ("vector2" => Vector2),
        ("rect2" => Rect2),
        ("vector3" => Vector3),
        ("transform2d" => Transform2D),
        ("plane" => Plane),
        ("quat" => Quat),
        ("aabb" => Aabb),
        ("basis" => Basis),
        ("transform" => Transform),
        ("color" => Color),
        ("nodepath" => NodePath),
        ("rid" => Rid),
        ("object" => Object),
        ("dictionary" => Dictionary),
        ("array" => VariantArray),
        ("byte_array" => ByteArray),
        ("int_array" => Int32Array),
        ("float_array" => Float32Array),
        ("string_array" => StringArray),
        ("vector2_array" => Vector2Array),
        ("vector3_array" => Vector3Array),
        ("color_array" => ColorArray),
    );

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "variant_type",
            |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                match ctx.data().get_registry()?.get(i as _) {
                    Some(v) => Ok(v.get_type() as _),
                    _ => Ok(0),
                }
            },
        )
        .unwrap();
}
