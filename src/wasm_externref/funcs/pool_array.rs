use anyhow::{bail, Error};
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, ExternRef, Linker};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::wasm_util::EXTERNREF_MODULE;

macro_rules! readwrite_array {
    (
        $linker:ident,
        $(($name:literal =>
            $v:ident : $t:ty [$c:expr]
            [$($sz:literal | $($i:ident $([$ix:literal])?).+ : $g:ty);* $(;)?]
        )),* $(,)?
    ) => {$(
        $linker.func_wrap(
            EXTERNREF_MODULE,
            concat!($name, ".len"),
            |_: Caller<_>, a: Option<ExternRef>| -> Result<i32, Error> {
                let a = <$t>::from_variant(&externref_to_variant(a))?;
                Ok(a.len())
            }
        ).unwrap();

        $linker.func_wrap(
            EXTERNREF_MODULE,
            concat!($name, ".read"),
            |mut ctx: Caller<StoreData>, a: Option<ExternRef>, p: u32| -> Result<u32, Error> {
                let a = <$t>::from_variant(&externref_to_variant(a))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                for $v in a.read().iter().copied() {
                    $(
                        mem.write(
                            &mut ctx,
                            p,
                            &<$g>::from($($i $([$ix])?).+).to_le_bytes(),
                        )?;
                        p += $sz;
                    )*
                }
                Ok(1)
            }
        ).unwrap();

        $linker.func_wrap(
            EXTERNREF_MODULE,
            concat!($name, ".slice"),
            |mut ctx: Caller<StoreData>, a: Option<ExternRef>, from: u32, to: u32, p: u32| -> Result<u32, Error> {
                if to > from {
                    bail!("Invalid range ({}-{})", from, to);
                } else if to == from {
                    return Ok(0);
                }

                let a = <$t>::from_variant(&externref_to_variant(a))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                let s = a.read();
                let s = match s.get(from as usize..to as usize) {
                    Some(v) => v,
                    None => bail!("Invalid array index ({}-{})", from as usize, to as usize),
                };
                for $v in s.iter().copied() {
                    $(
                        mem.write(
                            &mut ctx,
                            p,
                            &<$g>::from($($i $([$ix])?).+).to_le_bytes(),
                        )?;
                        p += $sz;
                    )*
                }
                Ok(1)
            }
        ).unwrap();

        $linker.func_wrap(
            EXTERNREF_MODULE,
            concat!($name, ".write"),
            |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<Option<ExternRef>, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(None),
                };

                let mut p = p as usize;
                let n = n as usize;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    #[allow(unused_assignments)]
                    let mut $v = $c;
                    $({
                        let mut s = [0u8; $sz];
                        mem.read(&ctx, p, &mut s)?;
                        $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                        p += $sz;
                    })*
                    v.push($v);
                }

                Ok(variant_to_externref(<$t>::from_vec(v).owned_to_variant()))
            }
        ).unwrap();
    )*};
}

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "byte_array.len",
            |_: Caller<_>, a: Option<ExternRef>| -> Result<i32, Error> {
                let a = ByteArray::from_variant(&externref_to_variant(a))?;
                Ok(a.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "byte_array.read",
            |mut ctx: Caller<StoreData>, a: Option<ExternRef>, p: u32| -> Result<u32, Error> {
                let a = ByteArray::from_variant(&externref_to_variant(a))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                mem.write(&mut ctx, p as _, &a.read())?;
                Ok(1)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "byte_array.write",
            |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<Option<ExternRef>, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(None),
                };

                let a = match mem.data(&ctx).get(p as usize..(p + n) as usize) {
                    Some(v) => ByteArray::from_slice(v),
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n),
                };
                Ok(variant_to_externref(a.owned_to_variant()))
            },
        )
        .unwrap();

    readwrite_array!(
        linker,
        ("int_array" => v: Int32Array [0i32] [4 | v: i32]),
        ("float_array" => v: Float32Array [0f32] [4 | v: f32]),
        ("vector2_array" => v: Vector2Array [Vector2::ZERO] [4 | v.x: f32; 4 | v.y: f32]),
        ("vector3_array" => v: Vector3Array [Vector3::ZERO] [
            4 | v.x: f32;
            4 | v.y: f32;
            4 | v.z: f32;
        ]),
        ("color_array" => v: ColorArray [Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0}] [
            4 | v.r: f32;
            4 | v.g: f32;
            4 | v.b: f32;
            4 | v.a: f32;
        ]),
    );

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "string_array.len",
            |_: Caller<_>, a: Option<ExternRef>| -> Result<i32, Error> {
                let a = StringArray::from_variant(&externref_to_variant(a))?;
                Ok(a.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "string_array.get",
            |_: Caller<_>, a: Option<ExternRef>, i: i32| -> Result<Option<ExternRef>, Error> {
                let a = StringArray::from_variant(&externref_to_variant(a))?;
                Ok(variant_to_externref(a.get(i).owned_to_variant()))
            },
        )
        .unwrap();

    // TODO: String array functions
}
