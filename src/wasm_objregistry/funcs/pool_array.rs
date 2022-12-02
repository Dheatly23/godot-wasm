use anyhow::{bail, Error};
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, Linker};

use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

macro_rules! readwrite_array {
    (
        $linker:ident,
        $(($name:literal =>
            $v:ident : $t:ty [$c:expr]
            [$($sz:literal | $($i:ident $([$ix:literal])?).+ : $g:ty);* $(;)?]
        )),* $(,)?
    ) => {$(
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".len"),
            |ctx: Caller<StoreData>, i: u32| -> Result<i32, Error> {
                let v = <$t>::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
                Ok(v.len())
            }
        ).unwrap();

        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".read"),
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let $v = <$t>::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                for $v in $v.read().iter().copied() {
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
            OBJREGISTRY_MODULE,
            concat!($name, ".write"),
            |mut ctx: Caller<StoreData>, i: u32, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                let n = n as usize;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    #[allow(unused_assignments)]
                    let mut $v = $c;
                    $({
                        let mut s = [0u8; $sz];
                        mem.read(&mut ctx, p, &mut s)?;
                        $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                        p += $sz;
                    })*
                    v.push($v);
                }

                ctx.data_mut().get_registry_mut()?.replace(i as _, <$t>::from_vec(v).to_variant());
                Ok(1)
            }
        ).unwrap();

        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".write_new"),
            |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                let n = n as usize;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    #[allow(unused_assignments)]
                    let mut $v = $c;
                    $({
                        let mut s = [0u8; $sz];
                        mem.read(&mut ctx, p, &mut s)?;
                        $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                        p += $sz;
                    })*
                    v.push($v);
                }

                Ok(ctx.data_mut().get_registry_mut()?.register(<$t>::from_vec(v).to_variant()) as _)
            }
        ).unwrap();
    )*};
}

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "byte_array.len",
            |ctx: Caller<StoreData>, i: u32| -> Result<i32, Error> {
                let reg = ctx.data().get_registry()?;
                let a = ByteArray::from_variant(&reg.get_or_nil(i as _))?;
                Ok(a.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "byte_array.read",
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let a = ByteArray::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
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
            OBJREGISTRY_MODULE,
            "byte_array.write",
            |mut ctx: Caller<StoreData>, i: u32, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let a = match mem.data(&ctx).get(p as usize..(p + n) as usize) {
                    Some(v) => ByteArray::from_slice(v),
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n),
                };
                ctx.data_mut()
                    .get_registry_mut()?
                    .replace(i as _, a.to_variant());
                Ok(1)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "byte_array.write_new",
            |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let a = match mem.data(&ctx).get(p as usize..(p + n) as usize) {
                    Some(v) => ByteArray::from_slice(v),
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n),
                };
                Ok(ctx.data_mut().get_registry_mut()?.register(a.to_variant()) as _)
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
}
