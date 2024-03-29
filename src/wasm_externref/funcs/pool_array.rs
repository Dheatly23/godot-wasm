use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, ExternRef, Func, StoreContextMut, TypedFunc};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

macro_rules! readwrite_array {
    ($(
        $head:tt =>
        $v:ident : $t:ty [$c:expr]
        [$($sz:literal | $($i:ident $([$ix:literal])?).+ : $g:ty);* $(;)?]
    ),* $(,)?) => {$(
        func_registry!{
            $head,
            len => |_: Caller<_>, a: Option<ExternRef>| -> Result<i32, Error> {
                let a = site_context!(<$t>::from_variant(&externref_to_variant(a)))?;
                Ok(a.len())
            },
            read => |mut ctx: Caller<_>, a: Option<ExternRef>, p: u32| -> Result<u32, Error> {
                let a = site_context!(<$t>::from_variant(&externref_to_variant(a)))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                for $v in a.read().iter().copied() {
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
            slice => |mut ctx: Caller<_>, a: Option<ExternRef>, from: u32, to: u32, p: u32| -> Result<u32, Error> {
                if to > from {
                    bail_with_site!("Invalid range ({}..{})", from, to);
                } else if to == from {
                    return Ok(0);
                }

                let a = site_context!(<$t>::from_variant(&externref_to_variant(a)))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                let s = a.read();
                let s = match s.get(from as usize..to as usize) {
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
            write => |mut ctx: Caller<_>, p: u32, n: u32| -> Result<Option<ExternRef>, Error> {
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
                        site_context!(mem.read(&ctx, p, &mut s))?;
                        $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                        p += $sz;
                    })*
                    v.push($v);
                }

                Ok(variant_to_externref(<$t>::from_vec(v).owned_to_variant()))
            },
        }
    )*};
}

#[derive(Default)]
pub struct Funcs {
    byte_array: ByteArrayFuncs,
    int_array: IntArrayFuncs,
    float_array: FloatArrayFuncs,
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
        } else if let r @ Some(_) = self.int_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.float_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector2_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector3_array.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.color_array.get_func(&mut *store, name) {
            r
        } else {
            self.string_array.get_func(&mut *store, name)
        }
    }
}

func_registry! {
    (ByteArrayFuncs, "byte_array."),
    len => |_: Caller<_>, a: Option<ExternRef>| -> Result<i32, Error> {
        let a = site_context!(<PoolArray<u8>>::from_variant(&externref_to_variant(a)))?;
        Ok(a.len())
    },
    read => |mut ctx: Caller<_>, a: Option<ExternRef>, p: u32| -> Result<u32, Error> {
        let a = site_context!(<PoolArray<u8>>::from_variant(&externref_to_variant(a)))?;
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        site_context!(mem.write(&mut ctx, p as _, &a.read()))?;
        Ok(1)
    },
    write => |mut ctx: Caller<_>, p: u32, n: u32| -> Result<Option<ExternRef>, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(None),
        };

        let a = match mem.data(&ctx).get(p as usize..(p + n) as usize) {
            Some(v) => <PoolArray<u8>>::from_slice(v),
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n),
        };
        Ok(variant_to_externref(a.owned_to_variant()))
    },
}

readwrite_array! {
    (IntArrayFuncs, "int_array.") => v: PoolArray<i32> [0i32] [4 | v: i32],
    (FloatArrayFuncs, "float_array.") => v: PoolArray<f32> [0f32] [4 | v: f32],
    (Vector2ArrayFuncs, "vector2_array.") => v: PoolArray<Vector2> [Vector2::ZERO] [4 | v.x: f32; 4 | v.y: f32],
    (Vector3ArrayFuncs, "vector3_array.") => v: PoolArray<Vector3> [Vector3::ZERO] [
        4 | v.x: f32;
        4 | v.y: f32;
        4 | v.z: f32;
    ],
    (ColorArrayFuncs, "color_array.") => v: PoolArray<Color> [Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0}] [
        4 | v.r: f32;
        4 | v.g: f32;
        4 | v.b: f32;
        4 | v.a: f32;
    ],
}

func_registry! {
    (StringArrayFuncs, "string_array."),
    len => |_: Caller<_>, a: Option<ExternRef>| -> Result<i32, Error> {
        let a = site_context!(<PoolArray<GodotString>>::from_variant(
            &externref_to_variant(a)
        ))?;
        Ok(a.len())
    },
    get => |_: Caller<_>, a: Option<ExternRef>, i: i32| -> Result<Option<ExternRef>, Error> {
        let a = site_context!(<PoolArray<GodotString>>::from_variant(
            &externref_to_variant(a)
        ))?;
        Ok(variant_to_externref(a.get(i).owned_to_variant()))
    },
    get_many => |mut ctx: Caller<_>, a: Option<ExternRef>, i: i32, f: Option<Func>| -> Result<u32, Error> {
        let f: TypedFunc<Option<ExternRef>, u32> = match f {
            Some(f) => site_context!(f.typed(&ctx))?,
            None => return Ok(0),
        };
        let a = site_context!(<PoolArray<GodotString>>::from_variant(
            &externref_to_variant(a)
        ))?;

        let mut n = 0;
        let mut s = &a.read()[i as _..];
        while let Some((v, rest)) = s.split_first() {
            n += 1;
            if site_context!(f.call(&mut ctx, variant_to_externref(v.to_variant())))? == 0 {
                break;
            }
            s = rest;
        }

        Ok(n)
    },
    build => |mut ctx: Caller<_>, f: Option<Func>| -> Result<Option<ExternRef>, Error> {
        let f: TypedFunc<u32, (Option<ExternRef>, u32)> = match f {
            Some(f) => site_context!(f.typed(&ctx))?,
            None => return Ok(None),
        };

        let mut v = Vec::new();
        loop {
            let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
            v.push(site_context!(GodotString::from_variant(
                &externref_to_variant(e)
            ))?);
            if n == 0 {
                break;
            }
        }

        Ok(variant_to_externref(
            <PoolArray<GodotString>>::from_vec(v).owned_to_variant(),
        ))
    },
}
