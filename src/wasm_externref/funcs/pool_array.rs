use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{Caller, Extern, ExternRef, Func, Rooted, StoreContextMut, TypedFunc};

use crate::godot_util::from_var_any;
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
            len => |ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
                let a = site_context!(from_var_any::<$t>(&externref_to_variant(&ctx, a)?))?;
                Ok(a.len() as _)
            },
            read => |mut ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>, p: u32| -> AnyResult<u32> {
                let a = site_context!(from_var_any::<$t>(&externref_to_variant(&ctx, a)?))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                for $v in a.as_slice().iter().copied() {
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
            slice => |mut ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>, from: u32, to: u32, p: u32| -> AnyResult<u32> {
                if to > from {
                    bail_with_site!("Invalid range ({}..{})", from, to);
                } else if to == from {
                    return Ok(0);
                }

                let a = site_context!(from_var_any::<$t>(&externref_to_variant(&ctx, a)?))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                let s = match a.as_slice().get(from as usize..to as usize) {
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
            write => |mut ctx: Caller<'_, _>, p: u32, n: u32| -> AnyResult<Option<Rooted<ExternRef>>> {
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

                let r = <$t>::from(&*v).to_variant();
                drop(v);
                variant_to_externref(ctx, r)
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
    len => |ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let a = site_context!(from_var_any::<PackedByteArray>(&externref_to_variant(ctx, a)?))?;
        Ok(a.len() as _)
    },
    read => |mut ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>, p: u32| -> AnyResult<u32> {
        let a = site_context!(from_var_any::<PackedByteArray>(&externref_to_variant(&ctx, a)?))?;
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        site_context!(mem.write(&mut ctx, p as _, a.as_slice()))?;
        Ok(1)
    },
    write => |mut ctx: Caller<'_, _>, p: u32, n: u32| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(None),
        };

        let a = match mem.data(&ctx).get(p as usize..(p + n) as usize) {
            Some(v) => PackedByteArray::from(v),
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n),
        };
        variant_to_externref(ctx, a.to_variant())
    },
}

readwrite_array! {
    (Int32ArrayFuncs, "int32_array") => v: PackedInt32Array [0i32] [4 | v: i32],
    (Int64ArrayFuncs, "int64_array") => v: PackedInt64Array [0i64] [8 | v: i64],
    (Float32ArrayFuncs, "float32_array") => v: PackedFloat32Array [0f32] [4 | v: f32],
    (Float64ArrayFuncs, "float64_array") => v: PackedFloat64Array [0f64] [8 | v: f64],
    (Vector2ArrayFuncs, "vector2_array.") => v: PackedVector2Array [Vector2::ZERO] [4 | v.x: f32; 4 | v.y: f32],
    (Vector3ArrayFuncs, "vector3_array.") => v: PackedVector3Array [Vector3::ZERO] [
        4 | v.x: f32;
        4 | v.y: f32;
        4 | v.z: f32;
    ],
    (ColorArrayFuncs, "color_array.") => v: PackedColorArray [Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0}] [
        4 | v.r: f32;
        4 | v.g: f32;
        4 | v.b: f32;
        4 | v.a: f32;
    ],
}

func_registry! {
    (StringArrayFuncs, "string_array."),
    len => |ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let a = site_context!(from_var_any::<PackedStringArray>(
            &externref_to_variant(ctx, a)?
        ))?;
        Ok(a.len() as _)
    },
    get => |ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>, i: u32| -> AnyResult<Option<Rooted<ExternRef>>> {
        let a = site_context!(from_var_any::<PackedStringArray>(
            &externref_to_variant(&ctx, a)?
        ))?;
        let Some(v) = a.as_slice().get(i as usize).map(|v| v.to_variant()) else {
            bail_with_site!("Index {i} out of bounds")
        };
        variant_to_externref(ctx, v)
    },
    get_many => |mut ctx: Caller<'_, _>, a: Option<Rooted<ExternRef>>, i: u32, f: Option<Func>| -> AnyResult<u32> {
        let f: TypedFunc<Option<Rooted<ExternRef>>, u32> = match f {
            Some(f) => site_context!(f.typed(&ctx))?,
            None => return Ok(0),
        };
        let a = site_context!(from_var_any::<PackedStringArray>(
            &externref_to_variant(&ctx, a)?
        ))?;

        let mut n = 0;
        let mut s = match a.as_slice().get(i as usize..) {
            Some(v) => v,
            None => bail_with_site!("Invalid array index ({}..)", i as usize),
        };
        while let Some((v, rest)) = s.split_first() {
            n += 1;
            let v = variant_to_externref(&mut ctx, v.to_variant())?;
            if site_context!(f.call(&mut ctx, v))? == 0 {
                break;
            }
            s = rest;
        }

        Ok(n)
    },
    build => |mut ctx: Caller<'_, _>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = match f {
            Some(f) => site_context!(f.typed(&ctx))?,
            None => return Ok(None),
        };

        let mut v = Vec::new();
        loop {
            let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
            v.push(site_context!(from_var_any::<GString>(
                &externref_to_variant(&ctx, e)?
            ))?);
            if n == 0 {
                break;
            }
        }

        let r = PackedStringArray::from(&*v).to_variant();
        drop(v);
        variant_to_externref(ctx, r)
    },
}
