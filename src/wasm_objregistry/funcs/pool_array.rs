use anyhow::{bail, Error};
use godot::prelude::*;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

macro_rules! readwrite_array {
    ($(($fi:ident, $name:literal) =>
        $v:ident : $t:ty [$c:expr]
        [$($sz:literal | $($i:ident $([$ix:literal])?).+ : $g:ty);* $(;)?]
    ),* $(,)?) => {$(
        func_registry!{
            ($fi, $name),
            len => |ctx: Caller<T>, i: u32| -> Result<u32, Error> {
                let v = site_context!(<$t>::try_from_variant(&ctx.data().as_ref().get_registry()?.get_or_nil(i as _)))?;
                Ok(v.len() as _)
            },
            read => |mut ctx: Caller<T>, i: u32, p: u32| -> Result<u32, Error> {
                let $v = site_context!(<$t>::try_from_variant(&ctx.data().as_ref().get_registry()?.get_or_nil(i as _)))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                for $v in $v.as_slice().iter().copied() {
                    $(
                        site_context!(mem.write(
                            &mut ctx,
                            p,
                            &<$g>::from($($i $([$ix])?).+).to_le_bytes(),
                        ))?;
                        p += $sz;
                    )*
                }
                Ok(1)
            },
            slice => |mut ctx: Caller<T>, i: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
                if to > from {
                    bail_with_site!("Invalid range ({}..{})", from, to);
                }
                let $v = site_context!(<$t>::try_from_variant(&ctx.data().as_ref().get_registry()?.get_or_nil(i as _)))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                if to == from {
                    return Ok(0);
                }

                let mut p = p as usize;
                let s = match $v.as_slice().get(from as usize..to as usize) {
                    Some(v) => v,
                    None => bail_with_site!("Invalid array index ({}..{})", from as usize, to as usize),
                };
                for $v in s.iter().copied() {
                    $(
                        site_context!(mem.write(
                            &mut ctx,
                            p,
                            &<$g>::from($($i $([$ix])?).+).to_le_bytes(),
                        ))?;
                        p += $sz;
                    )*
                }
                Ok(1)
            },
            write => |mut ctx: Caller<T>, i: u32, p: u32, n: u32| -> Result<u32, Error> {
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
                        site_context!(mem.read(&mut ctx, p, &mut s))?;
                        $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                        p += $sz;
                    })*
                    v.push($v);
                }

                let r = <$t>::from(&*v).to_variant();
                drop(v);
                ctx.data_mut().as_mut().get_registry_mut()?.replace(i as _, r);
                Ok(1)
            },
            write_new => |mut ctx: Caller<T>, p: u32, n: u32| -> Result<u32, Error> {
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
                        site_context!(mem.read(&mut ctx, p, &mut s))?;
                        $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                        p += $sz;
                    })*
                    v.push($v);
                }

                let r = <$t>::from(&*v).to_variant();
                drop(v);
                Ok(ctx.data_mut().as_mut().get_registry_mut()?.register(r) as _)
            },
        }
    )*};
}

#[derive(Default)]
pub struct Funcs {
    byte_array: ByteArrayFuncs,
    int32_array: Int32ArrayFuncs,
    int64_array: Int64ArrayFuncs,
    float32_array: Float32ArrayFuncs,
    float64_array: Float64ArrayFuncs,
    vector2_array: Vector2ArrayFuncs,
    vector3_array: Vector3ArrayFuncs,
    color_array: ColorArrayFuncs,
    string_array: StringArrayFuncs,
}

impl Funcs {
    pub fn get_func<T>(&mut self, store: &mut StoreContextMut<'_, T>, name: &str) -> Option<Func>
    where
        T: AsRef<StoreData> + AsMut<StoreData>,
    {
        if let r @ Some(_) = self.byte_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.int32_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.int64_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.float32_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.float64_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector2_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector3_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.color_array.get_func(&mut *store, name) {
            r
        } else {
            self.string_array.get_func(store, name)
        }
    }
}

func_registry! {
    (ByteArrayFuncs, "byte_array."),
    len => |ctx: Caller<T>, i: u32| -> Result<u32, Error> {
        let a = site_context!(PackedByteArray::try_from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;
        Ok(a.len() as _)
    },
    read => |mut ctx: Caller<T>, i: u32, p: u32| -> Result<u32, Error> {
        let a = site_context!(PackedByteArray::try_from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        site_context!(mem.write(&mut ctx, p as _, a.as_slice()))?;
        Ok(1)
    },
    slice => |mut ctx: Caller<T>, i: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
        if to > from {
            bail_with_site!("Invalid range ({}..{})", from, to);
        }
        let a = site_context!(PackedByteArray::try_from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        if to == from {
            return Ok(0);
        }

        let s = match a.as_slice().get(from as usize..to as usize) {
            Some(v) => v,
            None => bail_with_site!("Invalid array index ({}..{})", from as usize, to as usize),
        };
        site_context!(mem.write(&mut ctx, p as _, s))?;
        Ok(1)
    },
    write => |mut ctx: Caller<T>, i: u32, p: u32, n: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let a = match mem.data(&ctx).get(p as usize..(p + n) as usize) {
            Some(v) => PackedByteArray::from(v),
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n),
        };
        ctx.data_mut().as_mut()
            .get_registry_mut()?
            .replace(i as _, a.to_variant());
        Ok(1)
    },
    write_new => |mut ctx: Caller<T>, p: u32, n: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let a = match mem.data(&ctx).get(p as usize..(p + n) as usize) {
            Some(v) => PackedByteArray::from(v),
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n),
        };
        Ok(ctx.data_mut().as_mut().get_registry_mut()?.register(a.to_variant()) as _)
    },
}

readwrite_array! {
    (Int32ArrayFuncs, "int32_array") => v: PackedInt32Array [0i32] [4 | v: i32],
    (Int64ArrayFuncs, "int64_array") => v: PackedInt64Array [0i64] [8 | v: i64],
    (Float32ArrayFuncs, "float32_array") => v: PackedFloat32Array [0f32] [4 | v: f32],
    (Float64ArrayFuncs, "float64_array") => v: PackedFloat64Array [0f64] [8 | v: f64],
    (Vector2ArrayFuncs, "vector2_array") => v: PackedVector2Array [Vector2::ZERO] [4 | v.x: f32; 4 | v.y: f32],
    (Vector3ArrayFuncs, "vector3_array") => v: PackedVector3Array [Vector3::ZERO] [
        4 | v.x: f32;
        4 | v.y: f32;
        4 | v.z: f32;
    ],
    (ColorArrayFuncs, "color_array") => v: PackedColorArray [Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0}] [
        4 | v.r: f32;
        4 | v.g: f32;
        4 | v.b: f32;
        4 | v.a: f32;
    ],
}

func_registry! {
    (StringArrayFuncs, "string_array."),
    len => |ctx: Caller<T>, a: u32| -> Result<u32, Error> {
        let a = site_context!(PackedStringArray::try_from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(a as _)
        ))?;
        Ok(a.len() as _)
    },
    get => |mut ctx: Caller<T>, a: u32, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let a = site_context!(PackedStringArray::try_from_variant(
            &reg.get_or_nil(a as _)
        ))?;
        Ok(reg.register(a.get(i as _).to_variant()) as _)
    },
    slice => |mut ctx: Caller<T>, a: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
        if to > from {
            bail_with_site!("Invalid range ({}..{})", from, to);
        }
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let a = site_context!(PackedStringArray::try_from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(a as _),
        ))?;
        let s = match a.as_slice().get(from as usize..to as usize) {
            Some(v) => v,
            None => bail_with_site!("Invalid array index ({}..{})", from, to),
        };

        if to == from {
            return Ok(0);
        }

        let n = (to - from) as usize;
        let p = p as usize;

        let (ps, data) = mem.data_and_store_mut(&mut ctx);
        let reg = data.as_mut().get_registry_mut()?;
        let ps = match ps.get_mut(p..p + n * 4) {
            Some(v) => v,
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n * 4),
        };

        let mut ret = 0u32;
        for (v, p) in s.iter().zip(ps.chunks_mut(4)) {
            let v = reg.register(v.to_variant()) as u32;

            p.copy_from_slice(&v.to_le_bytes());
            ret += 1;
        }

        Ok(ret)
    },
    write => |mut ctx: Caller<T>, a: u32, p: u32, n: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let n = n as usize;
        let p = p as usize;

        let (ps, data) = mem.data_and_store_mut(&mut ctx);
        let reg = data.as_mut().get_registry_mut()?;
        let ps = match ps.get_mut(p..p + n * 4) {
            Some(v) => v,
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n * 4),
        };
        let mut v = Vec::with_capacity(n);
        for s in ps.chunks(4) {
            v.push(site_context!(GString::try_from_variant(
                &reg.get_or_nil(u32::from_le_bytes(s.try_into().unwrap()) as _),
            ))?);
        }

        let r = PackedStringArray::from(&*v).to_variant();
        drop(v);
        reg.replace(a as _, r);
        Ok(1)
    },
    write_new => |mut ctx: Caller<T>, p: u32, n: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let n = n as usize;
        let p = p as usize;

        let (ps, data) = mem.data_and_store_mut(&mut ctx);
        let reg = data.as_mut().get_registry_mut()?;
        let ps = match ps.get_mut(p..p + n * 4) {
            Some(v) => v,
            None => bail!("Invalid memory bounds ({}-{})", p, p + n * 4),
        };
        let mut v = Vec::with_capacity(n);
        for s in ps.chunks(4) {
            v.push(site_context!(GString::try_from_variant(
                &reg.get_or_nil(u32::from_le_bytes(s.try_into().unwrap()) as _),
            ))?);
        }

        let r = PackedStringArray::from(&*v).to_variant();
        drop(v);
        Ok(reg.register(r) as _)
    },
}
