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
            concat!($name, ".slice"),
            |mut ctx: Caller<StoreData>, i: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
                if to > from {
                    bail!("Invalid range ({}-{})", from, to);
                }
                let $v = <$t>::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                if to == from {
                    return Ok(0);
                }

                let mut p = p as usize;
                let s = $v.read();
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
                let a = <PoolArray<u8>>::from_variant(&reg.get_or_nil(i as _))?;
                Ok(a.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "byte_array.read",
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let a =
                    <PoolArray<u8>>::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
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
                    Some(v) => <PoolArray<u8>>::from_slice(v),
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
                    Some(v) => <PoolArray<u8>>::from_slice(v),
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n),
                };
                Ok(ctx.data_mut().get_registry_mut()?.register(a.to_variant()) as _)
            },
        )
        .unwrap();

    readwrite_array!(
        linker,
        ("int_array" => v: PoolArray<i32> [0i32] [4 | v: i32]),
        ("float_array" => v: PoolArray<f32> [0f32] [4 | v: f32]),
        ("vector2_array" => v: PoolArray<Vector2> [Vector2::ZERO] [4 | v.x: f32; 4 | v.y: f32]),
        ("vector3_array" => v: PoolArray<Vector3> [Vector3::ZERO] [
            4 | v.x: f32;
            4 | v.y: f32;
            4 | v.z: f32;
        ]),
        ("color_array" => v: PoolArray<Color> [Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0}] [
            4 | v.r: f32;
            4 | v.g: f32;
            4 | v.b: f32;
            4 | v.a: f32;
        ]),
    );

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string_array.len",
            |ctx: Caller<StoreData>, a: u32| -> Result<i32, Error> {
                let reg = ctx.data().get_registry()?;
                let a = <PoolArray<GodotString>>::from_variant(&reg.get_or_nil(a as _))?;
                Ok(a.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string_array.get",
            |mut ctx: Caller<StoreData>, a: u32, i: i32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let a = <PoolArray<GodotString>>::from_variant(&reg.get_or_nil(a as _))?;
                Ok(reg.register(a.get(i).owned_to_variant()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string_array.slice",
            |mut ctx: Caller<StoreData>,
             a: u32,
             from: u32,
             to: u32,
             p: u32|
             -> Result<u32, Error> {
                if to > from {
                    bail!("Invalid range ({}-{})", from, to);
                }
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let a = <PoolArray<GodotString>>::from_variant(
                    &ctx.data().get_registry()?.get_or_nil(a as _),
                )?;
                let s = a.read();
                let s = match s.get(from as usize..to as usize) {
                    Some(v) => v,
                    None => bail!("Invalid array index ({}-{})", from as usize, to as usize),
                };

                if to == from {
                    return Ok(0);
                }

                let n = (to - from) as usize;
                let p = p as usize;

                let (ps, data) = mem.data_and_store_mut(&mut ctx);
                let reg = data.get_registry_mut()?;
                let ps = match ps.get_mut(p..p + n * 4) {
                    Some(v) => v,
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n * 4),
                };

                let mut ret = 0u32;
                for (i, v) in s.iter().enumerate() {
                    let v = reg.register(v.to_variant()) as u32;

                    ps[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                    ret += 1;
                }

                Ok(ret)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string_array.write",
            |mut ctx: Caller<StoreData>, a: u32, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let n = n as usize;
                let p = p as usize;

                let (ps, data) = mem.data_and_store_mut(&mut ctx);
                let reg = data.get_registry_mut()?;
                let ps = match ps.get_mut(p..p + n * 4) {
                    Some(v) => v,
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n * 4),
                };
                let mut v = Vec::with_capacity(n);
                for s in ps.chunks(4) {
                    v.push(GodotString::from_variant(
                        &reg.get_or_nil(u32::from_le_bytes(s.try_into().unwrap()) as _),
                    )?);
                }

                reg.replace(a as _, <PoolArray<GodotString>>::from_vec(v).to_variant());
                Ok(1)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string_array.write_new",
            |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let n = n as usize;
                let p = p as usize;

                let (ps, data) = mem.data_and_store_mut(&mut ctx);
                let reg = data.get_registry_mut()?;
                let ps = match ps.get_mut(p..p + n * 4) {
                    Some(v) => v,
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n * 4),
                };
                let mut v = Vec::with_capacity(n);
                for s in ps.chunks(4) {
                    v.push(GodotString::from_variant(
                        &reg.get_or_nil(u32::from_le_bytes(s.try_into().unwrap()) as _),
                    )?);
                }

                Ok(reg.register(<PoolArray<GodotString>>::from_vec(v).to_variant()) as _)
            },
        )
        .unwrap();
}
