use std::mem;
use std::slice;

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
        self.slab.get(ix).cloned()
    }

    #[inline]
    pub fn register(&mut self, v: Variant) -> usize {
        if v.is_nil() {
            panic!("Variant cannot be nil!");
        }
        self.slab.insert(v)
    }

    #[inline]
    pub fn unregister(&mut self, ix: usize) -> Option<Variant> {
        self.slab.try_remove(ix)
    }

    #[inline]
    pub fn replace(&mut self, ix: usize, v: Variant) -> Option<Variant> {
        if v.is_nil() {
            panic!("Variant cannot be nil!");
        }
        match self.slab.get_mut(ix).as_mut() {
            Some(p) => Some(mem::replace(p, v)),
            None => None,
        }
    }

    fn get_with_err(&self, ix: usize) -> Result<Variant, Error> {
        match self.get(ix) {
            Some(v) => Ok(v),
            None => bail!("Index {} is null", ix),
        }
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
                let v = ctx.data().get_registry()?.get_with_err(i as _)?;
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
        $(($name:literal => $t:ty)),* $(,)?
    ) => {$(
        #[allow(unused_parens)]
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".read"),
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let v = <$t>::from_variant(&ctx.data().get_registry()?.get_with_err(i as _)?)?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                // SAFETY: We are reading a live C struct here,
                // so representing as slice should be kosher.
                let s = unsafe {
                    slice::from_raw_parts(&v as *const _ as *const u8, mem::size_of::<$t>())
                };
                mem.write(&mut ctx, p as _, s)?;
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

                let mut v: mem::MaybeUninit<$t> = mem::MaybeUninit::uninit();
                {
                    // SAFETY: Bounds of slice match value slot
                    // and value lifetime exceed slice lifetime.
                    let s = unsafe {
                        slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u8, mem::size_of::<$t>())
                    };
                    mem.read(&mut ctx, p as _, s)?;
                }

                // SAFETY: At this point, value is initialized.
                let v = unsafe { v.assume_init() };
                ctx.data_mut().get_registry_mut()?.replace(i as _, v.to_variant());
                Ok(1)
            }
        ).unwrap();

        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".write_new"),
            |mut ctx: Caller<StoreData>, p: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(u32::MAX),
                };

                let mut v: mem::MaybeUninit<$t> = mem::MaybeUninit::uninit();
                {
                    // SAFETY: Bounds of slice match value slot
                    // and value lifetime exceed slice lifetime.
                    let s = unsafe {
                        slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u8, mem::size_of::<$t>())
                    };
                    mem.read(&mut ctx, p as _, s)?;
                }

                // SAFETY: At this point, value is initialized.
                let v = unsafe { v.assume_init() };
                Ok(ctx.data_mut().get_registry_mut()?.register(v.to_variant()) as _)
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
                    let v = reg.get_with_err(i as _)?;
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
                    let v = reg.get_with_err(s as _)?;
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

        readwrite_value!(
            linker,
            ("bool" => bool),
            ("int" => i64),
            ("float" => f64),
            ("vector2" => Vector2),
            ("vector3" => Vector3),
            ("quat" => Quat),
            ("rect2" => Rect2),
            ("transform2d" => Transform2D),
            ("plane" => Plane),
            ("aabb" => Aabb),
            ("basis" => Basis),
            ("transform" => Transform),
            ("color" => Color),
        );

        linker
    };
}
