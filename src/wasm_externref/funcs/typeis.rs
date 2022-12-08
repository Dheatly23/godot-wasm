use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Linker};

use crate::wasm_externref::externref_to_variant;
use crate::wasm_instance::StoreData;
use crate::wasm_util::EXTERNREF_MODULE;

macro_rules! is_typecheck {
    ($linker:ident, $(($name:literal => $var:ident)),* $(,)?) => {$(
        $linker.func_wrap(
            EXTERNREF_MODULE,
            concat!($name, ".is"),
            |_: Caller<_>, v: Option<ExternRef>| -> Result<u32, Error> {
                Ok((externref_to_variant(v).get_type() == VariantType::$var) as _)
            }
        ).unwrap();
    )*};
}

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    is_typecheck!(
        linker,
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
}
