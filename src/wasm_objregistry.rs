use std::mem;

use anyhow::{bail, Error};
use gdnative::prelude::*;
use lazy_static::lazy_static;
use slab::Slab;
use wasmtime::{Caller, Extern, Linker};

use crate::wasm_engine::ENGINE;
use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

pub struct ObjectRegistry {
    slab: Slab<Variant>,
}

impl Default for ObjectRegistry {
    #[inline]
    fn default() -> Self {
        Self { slab: Slab::new() }
    }
}

impl ObjectRegistry {
    #[inline]
    pub fn get(&self, ix: usize) -> Option<Variant> {
        match ix.checked_sub(1) {
            Some(ix) => self.slab.get(ix).cloned(),
            None => None,
        }
    }

    #[inline]
    pub fn register(&mut self, v: Variant) -> usize {
        if v.is_nil() {
            0
        } else {
            self.slab.insert(v) + 1
        }
    }

    #[inline]
    pub fn unregister(&mut self, ix: usize) -> Option<Variant> {
        match ix.checked_sub(1) {
            Some(ix) => self.slab.try_remove(ix),
            None => None,
        }
    }

    #[inline]
    pub fn replace(&mut self, ix: usize, v: Variant) -> Option<Variant> {
        if v.is_nil() {
            return self.unregister(ix);
        }
        match ix.checked_sub(1) {
            Some(ix) => match self.slab.get_mut(ix).as_mut() {
                Some(p) => Some(mem::replace(p, v)),
                None => None,
            },
            None => None,
        }
    }

    fn get_or_nil(&self, ix: usize) -> Variant {
        self.get(ix).unwrap_or_else(Variant::nil)
    }
}

macro_rules! is_typecheck {
    ($linker:ident, $(($name:literal, $var:ident)),* $(,)?) => {$(
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

macro_rules! setget_value {
    (#getter $x:ident as $ex:expr) => {$ex};
    (#getter $x:ident) => {$x};
    (
        $linker:ident,
        $(($name:literal =>
            ($($x:ident : $tx:ty $(as $ex:expr)?),*) $($v:tt)*
        )),* $(,)?
    ) => {$(
        #[allow(unused_parens)]
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".get"),
            |ctx: Caller<StoreData>, i: u32| -> Result<($($tx),*), Error> {
                let v = ctx.data().get_registry()?.get_or_nil(i as _);
                let $($v)* = <_>::from_variant(&v)?;
                Ok(($(setget_value!(#getter $x $(as $ex)?)),*))
            }
        ).unwrap();

        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".set"),
            |mut ctx: Caller<StoreData>, i: u32, $($x : $tx),*| -> Result<(), Error> {
                let v = $($v)*;
                ctx.data_mut().get_registry_mut()?.replace(i as _, v.to_variant());
                Ok(())
            }
        ).unwrap();

        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".new"),
            |mut ctx: Caller<StoreData>, $($x : $tx),*| -> Result<u32, Error> {
                let v = $($v)*;
                Ok(ctx.data_mut().get_registry_mut()?.register(v.to_variant()) as _)
            }
        ).unwrap();
    )*};
}

macro_rules! readwrite_value {
    (
        $linker:ident,
        $(($name:literal =>
            $v:ident : $t:ty [$c:expr]
            [$($off:literal, $sz:literal | $($i:ident $([$ix:literal])?).+ : $g:ty);* $(;)?]
        )),* $(,)?
    ) => {$(
        #[allow(unused_parens)]
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".read"),
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let $v = <$t>::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let p = p as usize;
                $(
                    mem.write(
                        &mut ctx,
                        p + $off,
                        &<$g>::from($($i $([$ix])?).+).to_le_bytes(),
                    )?;
                )*
                Ok(1)
            }
        ).unwrap();

        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".write"),
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let p = p as usize;
                #[allow(unused_assignments)]
                let mut $v: $t = $c;
                $({
                    let mut s = [0u8; $sz];
                    mem.read(&mut ctx, p + $off, &mut s)?;
                    $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                })*

                ctx.data_mut().get_registry_mut()?.replace(i as _, $v.to_variant());
                Ok(1)
            }
        ).unwrap();

        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".write_new"),
            |mut ctx: Caller<StoreData>, p: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let p = p as usize;
                #[allow(unused_assignments)]
                let mut $v: $t = $c;
                $({
                    let mut s = [0u8; $sz];
                    mem.read(&mut ctx, p + $off, &mut s)?;
                    $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                })*

                Ok(ctx.data_mut().get_registry_mut()?.register($v.to_variant()) as _)
            }
        ).unwrap();
    )*};
}

lazy_static! {
    pub static ref OBJREGISTRY_LINKER: Linker<StoreData> = {
        let mut linker: Linker<StoreData> = Linker::new(&ENGINE);

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

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "null.is",
                |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    match ctx.data().get_registry()?.get(i as _) {
                        Some(_) => Ok(0),
                        None => Ok(1),
                    }
                },
            )
            .unwrap();

        is_typecheck!(
            linker,
            ("bool", Bool),
            ("int", I64),
            ("float", F64),
            ("string", GodotString),
            ("vector2", Vector2),
            ("rect2", Rect2),
            ("vector3", Vector3),
            ("transform2d", Transform2D),
            ("plane", Plane),
            ("quat", Quat),
            ("aabb", Aabb),
            ("basis", Basis),
            ("transform", Transform),
            ("color", Color),
            ("nodepath", NodePath),
            ("rid", Rid),
            ("object", Object),
            ("dictionary", Dictionary),
            ("array", VariantArray),
            ("byte_array", ByteArray),
            ("int_array", Int32Array),
            ("float_array", Float32Array),
            ("string_array", StringArray),
            ("vector2_array", Vector2Array),
            ("vector3_array", Vector3Array),
            ("color_array", ColorArray),
        );

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "delete",
                |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    match ctx.data_mut().get_registry_mut()?.unregister(i as _) {
                        Some(_) => Ok(1),
                        None => Ok(0),
                    }
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "duplicate",
                |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = reg.get_or_nil(i as _);
                    Ok(reg.register(v) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "copy",
                |mut ctx: Caller<StoreData>, s: u32, d: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = reg.get_or_nil(s as _);
                    match reg.replace(d as _, v) {
                        Some(_) => Ok(1),
                        None => Ok(0),
                    }
                },
            )
            .unwrap();

        setget_value!(
            linker,
            ("bool" => (v: u32 as {let v: bool = v; v as _}) v),
            ("int" => (v: i64) v),
            ("float" => (v: f64) v),
            ("vector2" => (x: f32, y: f32) Vector2 {x, y}),
            ("vector3" => (x: f32, y: f32, z: f32) Vector3 {x, y, z}),
            ("quat" => (x: f32, y: f32, z: f32, w: f32) Quat {x, y, z, w}),
            ("rect2" => (x: f32, y: f32, w: f32, h: f32) Rect2 {
                position: Vector2 {x, y},
                size: Vector2 {x: w, y: h},
            }),
            ("transform2d" =>
                (ax: f32, ay: f32, bx: f32, by: f32, ox: f32, oy: f32)
                Transform2D {
                    a: Vector2 {x: ax, y: ay},
                    b: Vector2 {x: bx, y: by},
                    origin: Vector2 {x: ox, y: oy},
                }
            ),
            ("plane" => (a: f32, b: f32, c: f32, d: f32) Plane {
                normal: Vector3 {x: a, y: b, z: c},
                d,
            }),
            ("aabb" => (x: f32, y: f32, z: f32, w: f32, h: f32, t: f32) Aabb {
                position : Vector3 {x, y, z},
                size: Vector3 {x: w, y: h, z: t},
            }),
            ("basis" =>
                (ax: f32, ay: f32, az: f32, bx: f32, by: f32, bz: f32, cx: f32, cy: f32, cz: f32)
                Basis {
                    elements: [
                        Vector3 {x: ax, y: ay, z: az},
                        Vector3 {x: bx, y: by, z: bz},
                        Vector3 {x: cx, y: cy, z: cz},
                    ],
                }
            ),
            ("transform" =>
                (
                    ax: f32, ay: f32, az: f32,
                    bx: f32, by: f32, bz: f32,
                    cx: f32, cy: f32, cz: f32,
                    ox: f32, oy: f32, oz: f32
                )
                Transform {
                    basis: Basis {
                        elements: [
                            Vector3 {x: ax, y: ay, z: az},
                            Vector3 {x: bx, y: by, z: bz},
                            Vector3 {x: cx, y: cy, z: cz},
                        ],
                    },
                    origin: Vector3 {x: ox, y: oy, z: oz},
                }
            ),
            ("color" => (r: f32, g: f32, b: f32, a: f32) Color {r, g, b, a}),
        );

        #[derive(Clone, Copy)]
        struct BoolWrapper(u8);

        impl BoolWrapper {
            fn from_le_bytes(s: [u8; 1]) -> Self {
                Self(s[0])
            }

            fn to_le_bytes(self) -> [u8; 1] {
                [self.0]
            }
        }

        impl From<bool> for BoolWrapper {
            fn from(b: bool) -> Self {
                Self(b.into())
            }
        }

        impl Into<bool> for BoolWrapper {
            fn into(self) -> bool {
                self.0 != 0
            }
        }

        readwrite_value!(
            linker,
            ("bool" => v: bool [false] [0, 1 | v: BoolWrapper]),
            ("int" => v: i64 [0i64] [0, 8 | v: i64]),
            ("float" => v: f64 [0f64] [0, 8 | v: f64]),
            ("vector2" => v: Vector2 [Vector2::ZERO] [0, 4 | v.x: f32; 4, 4 | v.y: f32]),
            ("vector3" => v: Vector3 [Vector3::ZERO] [
                0, 4 | v.x: f32;
                4, 4 | v.y: f32;
                8, 4 | v.z: f32;
            ]),
            ("quat" => v: Quat [Quat {x: 0.0, y: 0.0, z: 0.0, w: 0.0}] [
                0, 4 | v.x: f32;
                4, 4 | v.y: f32;
                8, 4 | v.z: f32;
                12, 4 | v.w: f32;
            ]),
            ("rect2" => v: Rect2 [Rect2 {position: Vector2::ZERO, size: Vector2::ZERO}] [
                0, 4 | v.position.x: f32;
                4, 4 | v.position.y: f32;
                8, 4 | v.size.x: f32;
                12, 4 | v.size.y: f32;
            ]),
            ("transform2d" => v: Transform2D [Transform2D {
                a: Vector2::ZERO,
                b: Vector2::ZERO,
                origin: Vector2::ZERO,
            }] [
                0, 4 | v.a.x: f32;
                4, 4 | v.a.y: f32;
                8, 4 | v.b.x: f32;
                12, 4 | v.b.y: f32;
                16, 4 | v.origin.x: f32;
                20, 4 | v.origin.y: f32;
            ]),
            ("plane" => v: Plane [Plane {normal: Vector3::ZERO, d: 0.0}] [
                0, 4 | v.normal.x: f32;
                4, 4 | v.normal.y: f32;
                8, 4 | v.normal.z: f32;
                12, 4 | v.d: f32;
            ]),
            ("aabb" => v: Aabb [Aabb {position: Vector3::ZERO, size: Vector3::ZERO}] [
                0, 4 | v.position.x: f32;
                4, 4 | v.position.y: f32;
                8, 4 | v.position.z: f32;
                12, 4 | v.size.x: f32;
                16, 4 | v.size.y: f32;
                20, 4 | v.size.z: f32;
            ]),
            ("basis" => v: Basis [Basis {elements: [Vector3::ZERO; 3]}] [
                0, 4 | v.elements[0].x: f32;
                4, 4 | v.elements[0].y: f32;
                8, 4 | v.elements[0].z: f32;
                12, 4 | v.elements[1].x: f32;
                16, 4 | v.elements[1].y: f32;
                20, 4 | v.elements[1].z: f32;
                24, 4 | v.elements[2].x: f32;
                28, 4 | v.elements[2].y: f32;
                32, 4 | v.elements[2].z: f32;
            ]),
            ("transform" => v: Transform [Transform {
                basis: Basis {elements: [Vector3::ZERO; 3]},
                origin: Vector3::ZERO,
            }] [
                0, 4 | v.basis.elements[0].x: f32;
                4, 4 | v.basis.elements[0].y: f32;
                8, 4 | v.basis.elements[0].z: f32;
                12, 4 | v.basis.elements[1].x: f32;
                16, 4 | v.basis.elements[1].y: f32;
                20, 4 | v.basis.elements[1].z: f32;
                24, 4 | v.basis.elements[2].x: f32;
                28, 4 | v.basis.elements[2].y: f32;
                32, 4 | v.basis.elements[2].z: f32;
                36, 4 | v.origin.x: f32;
                40, 4 | v.origin.y: f32;
                44, 4 | v.origin.z: f32;
            ]),
            ("color" => v: Color [Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0}] [
                0, 4 | v.r: f32;
                4, 4 | v.g: f32;
                8, 4 | v.b: f32;
                12, 4 | v.a: f32;
            ]),
        );

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "string.len",
                |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    let v = GodotString::from_variant(
                        &ctx.data().get_registry()?.get_or_nil(i as _),
                    )?;
                    Ok(v.len() as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "string.read",
                |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                    let v = GodotString::from_variant(
                        &ctx.data().get_registry()?.get_or_nil(i as _),
                    )?;
                    let mem = match ctx.get_export("memory") {
                        Some(Extern::Memory(v)) => v,
                        _ => return Ok(0),
                    };

                    mem.write(&mut ctx, p as _, v.to_string().as_bytes())?;
                    Ok(1)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "string.write",
                |mut ctx: Caller<StoreData>, i: u32, p: u32, n: u32| -> Result<u32, Error> {
                    let mem = match ctx.get_export("memory") {
                        Some(Extern::Memory(v)) => v,
                        _ => return Ok(0),
                    };

                    let mut v = vec![0u8; n as usize];
                    mem.read(&mut ctx, p as _, &mut v)?;
                    let v = String::from_utf8(v)?;
                    ctx.data_mut()
                        .get_registry_mut()?
                        .replace(i as _, v.to_variant());
                    Ok(1)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "string.write_new",
                |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<u32, Error> {
                    let mem = match ctx.get_export("memory") {
                        Some(Extern::Memory(v)) => v,
                        _ => return Ok(0),
                    };

                    let mut v = vec![0u8; n as usize];
                    mem.read(&mut ctx, p as _, &mut v)?;
                    let v = String::from_utf8(v)?;
                    Ok(ctx.data_mut().get_registry_mut()?.register(v.to_variant()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.new",
                |mut ctx: Caller<StoreData>| -> Result<u32, Error> {
                    Ok(ctx
                        .data_mut()
                        .get_registry_mut()?
                        .register(Dictionary::new().owned_to_variant()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.len",
                |ctx: Caller<StoreData>, i: u32| -> Result<i32, Error> {
                    let v = Dictionary::from_variant(
                        &ctx.data().get_registry()?.get_or_nil(i as _),
                    )?;
                    Ok(v.len())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.has",
                |ctx: Caller<StoreData>, i: u32, k: u32| -> Result<u32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    let k = reg.get_or_nil(k as _);
                    Ok(v.contains(k) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.has_all",
                |ctx: Caller<StoreData>, i: u32, ka: u32| -> Result<u32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    let ka = VariantArray::from_variant(&reg.get_or_nil(ka as _))?;
                    Ok(v.contains_all(&ka) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.get",
                |mut ctx: Caller<StoreData>, i: u32, k: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    let k = reg.get_or_nil(k as _);
                    match v.get(k) {
                        Some(v) => Ok(reg.register(v.to_variant()) as _),
                        _ => Ok(0),
                    }
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.set",
                |ctx: Caller<StoreData>, i: u32, k: u32, v: u32| -> Result<u32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    let k = reg.get_or_nil(k as _);
                    let v = reg.get_or_nil(v as _);

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let d = unsafe {d.assume_unique()};
                    let r = d.contains(k.clone());
                    d.insert(k, v);
                    Ok(r as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.delete",
                |ctx: Caller<StoreData>, i: u32, k: u32| -> Result<u32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    let k = reg.get_or_nil(k as _);

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let d = unsafe {d.assume_unique()};
                    let r = d.contains(k.clone());
                    d.erase(k);
                    Ok(r as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.keys",
                |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    Ok(reg.register(d.keys().owned_to_variant()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.values",
                |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    Ok(reg.register(d.values().owned_to_variant()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.iter_slice",
                |mut ctx: Caller<StoreData>, i: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
                    if to > from {
                        bail!("Invalid range ({}-{})", from, to);
                    }
                    let mem = match ctx.get_export("memory") {
                        Some(Extern::Memory(v)) => v,
                        _ => return Ok(0),
                    };
                    let d = Dictionary::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;

                    if to == from {
                        return Ok(0);
                    }

                    let n = (to - from) as usize;
                    let p = p as usize;

                    let (ps, data) = mem.data_and_store_mut(&mut ctx);
                    let reg = data.get_registry_mut()?;
                    let ps = match ps.get_mut(p..p + n*8) {
                        Some(v) => v,
                        None => bail!("Invalid memory bounds ({}-{})", p, p + n*8),
                    };

                    let mut ret = 0u32;
                    let from = from as usize;
                    for (i, (k, v)) in d.iter().skip(from).take(n).enumerate() {
                        let k = reg.register(k) as u32;
                        let v = reg.register(v) as u32;

                        ps[i*8..i*8 + 4].copy_from_slice(&k.to_le_bytes());
                        ps[i*8 + 4..i*8 + 8].copy_from_slice(&v.to_le_bytes());
                        ret += 1;
                    }

                    Ok(ret)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.clear",
                |ctx: Caller<StoreData>, i: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let d = unsafe {d.assume_unique()};
                    d.clear();
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "dictionary.duplicate",
                |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                    Ok(reg.register(d.duplicate().owned_to_variant()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.new",
                |mut ctx: Caller<StoreData>| -> Result<u32, Error> {
                    Ok(ctx
                        .data_mut()
                        .get_registry_mut()?
                        .register(VariantArray::new().owned_to_variant()) as _
                    )
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.len",
                |ctx: Caller<StoreData>, i: u32| -> Result<i32, Error> {
                    Ok(VariantArray::from_variant(
                        &ctx.data().get_registry()?.get_or_nil(i as _)
                    )?.len())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.len",
                |ctx: Caller<StoreData>, i: u32| -> Result<i32, Error> {
                    Ok(VariantArray::from_variant(
                        &ctx.data().get_registry()?.get_or_nil(i as _)
                    )?.len())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.get",
                |mut ctx: Caller<StoreData>, v: u32, i: i32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    Ok(reg.register(v.get(i)) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.set",
                |ctx: Caller<StoreData>, v: u32, i: i32, x: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);
                    v.set(i, x);
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.count",
                |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<i32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);
                    Ok(v.count(x))
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.contains",
                |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<u32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);
                    Ok(v.contains(x) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.find",
                |ctx: Caller<StoreData>, v: u32, x: u32, from: i32| -> Result<i32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);
                    Ok(v.find(x, from))
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.rfind",
                |ctx: Caller<StoreData>, v: u32, x: u32, from: i32| -> Result<i32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);
                    Ok(v.rfind(x, from))
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.find_last",
                |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<i32, Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);
                    Ok(v.find_last(x))
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.invert",
                |ctx: Caller<StoreData>, v: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    v.invert();
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.sort",
                |ctx: Caller<StoreData>, v: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    v.sort();
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.duplicate",
                |mut ctx: Caller<StoreData>, v: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    Ok(reg.register(v.duplicate().owned_to_variant()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.clear",
                |ctx: Caller<StoreData>, v: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    v.clear();
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.remove",
                |ctx: Caller<StoreData>, v: u32, i: i32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    v.remove(i);
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.erase",
                |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    v.erase(x);
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.resize",
                |ctx: Caller<StoreData>, v: u32, i: i32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    v.resize(i);
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.push",
                |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    v.push(x);
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.pop",
                |mut ctx: Caller<StoreData>, v: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    Ok(reg.register(v.pop()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.push_front",
                |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    v.push_front(x);
                    Ok(())
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.pop_front",
                |mut ctx: Caller<StoreData>, v: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    Ok(reg.register(v.pop_front()) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "array.insert",
                |ctx: Caller<StoreData>, v: u32, i: i32, x: u32| -> Result<(), Error> {
                    let reg = ctx.data().get_registry()?;
                    let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                    let x = reg.get_or_nil(x as _);

                    // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                    let v = unsafe {v.assume_unique()};
                    v.insert(i, x);
                    Ok(())
                },
            )
            .unwrap();

        linker
    };
}
