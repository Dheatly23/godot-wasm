use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, Linker};

use crate::site_context;
use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

macro_rules! setget_value {
    (#getter $x:ident as $ex:expr) => {$ex};
    (#getter $x:ident) => {$x};
    (
        $linker:ident,
        $(($name:literal =>
            ($($x:ident : $tx:ty $(as $ex:expr)?),* $(,)?) $($v:tt)*
        )),* $(,)?
    ) => {$(
        #[allow(unused_parens)]
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".get"),
            |ctx: Caller<StoreData>, i: u32| -> Result<($($tx),*), Error> {
                let v = ctx.data().get_registry()?.get_or_nil(i as _);
                let $($v)* = site_context!(<_>::from_variant(&v))?;
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
            [$($sz:literal | $($i:ident $([$ix:literal])?).+ : $g:ty);* $(;)?]
        )),* $(,)?
    ) => {$(
        #[allow(unused_assignments)]
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".read"),
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let $v = site_context!(<$t>::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _)))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                $(
                    site_context!(mem.write(
                        &mut ctx,
                        p,
                        &<$g>::from($($i $([$ix])?).+).to_le_bytes(),
                    ))?;
                    p += $sz;
                )*
                Ok(1)
            }
        ).unwrap();

        #[allow(unused_assignments)]
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".write"),
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                #[allow(unused_assignments)]
                let mut $v: $t = $c;
                $({
                    let mut s = [0u8; $sz];
                    site_context!(mem.read(&mut ctx, p, &mut s))?;
                    $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                    p += $sz;
                })*

                ctx.data_mut().get_registry_mut()?.replace(i as _, $v.to_variant());
                Ok(1)
            }
        ).unwrap();

        #[allow(unused_assignments)]
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!($name, ".write_new"),
            |mut ctx: Caller<StoreData>, p: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                #[allow(unused_assignments)]
                let mut $v: $t = $c;
                $({
                    let mut s = [0u8; $sz];
                    site_context!(mem.read(&mut ctx, p, &mut s))?;
                    $($i $([$ix])?).+ = <$g>::from_le_bytes(s).into();
                    p += $sz;
                })*

                Ok(ctx.data_mut().get_registry_mut()?.register($v.to_variant()) as _)
            }
        ).unwrap();
    )*};
}

#[derive(Clone, Copy)]
struct BoolWrapper(u8);

impl BoolWrapper {
    #[inline]
    fn from_le_bytes(s: [u8; 1]) -> Self {
        Self(s[0])
    }

    #[inline]
    fn to_le_bytes(self) -> [u8; 1] {
        [self.0]
    }
}

impl From<bool> for BoolWrapper {
    #[inline]
    fn from(b: bool) -> Self {
        Self(b.into())
    }
}

impl Into<bool> for BoolWrapper {
    #[inline]
    fn into(self) -> bool {
        self.0 != 0
    }
}

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
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
                ox: f32, oy: f32, oz: f32,
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
        ("bool" => v: bool [false] [1 | v: BoolWrapper]),
        ("int" => v: i64 [0i64] [8 | v: i64]),
        ("float" => v: f64 [0f64] [8 | v: f64]),
        ("vector2" => v: Vector2 [Vector2::ZERO] [4 | v.x: f32; 4 | v.y: f32]),
        ("vector3" => v: Vector3 [Vector3::ZERO] [
            4 | v.x: f32;
            4 | v.y: f32;
            4 | v.z: f32;
        ]),
        ("quat" => v: Quat [Quat {x: 0.0, y: 0.0, z: 0.0, w: 0.0}] [
            4 | v.x: f32;
            4 | v.y: f32;
            4 | v.z: f32;
            4 | v.w: f32;
        ]),
        ("rect2" => v: Rect2 [Rect2 {position: Vector2::ZERO, size: Vector2::ZERO}] [
            4 | v.position.x: f32;
            4 | v.position.y: f32;
            4 | v.size.x: f32;
            4 | v.size.y: f32;
        ]),
        ("transform2d" => v: Transform2D [Transform2D {
            a: Vector2::ZERO,
            b: Vector2::ZERO,
            origin: Vector2::ZERO,
        }] [
            4 | v.a.x: f32;
            4 | v.a.y: f32;
            4 | v.b.x: f32;
            4 | v.b.y: f32;
            4 | v.origin.x: f32;
            4 | v.origin.y: f32;
        ]),
        ("plane" => v: Plane [Plane {normal: Vector3::ZERO, d: 0.0}] [
            4 | v.normal.x: f32;
            4 | v.normal.y: f32;
            4 | v.normal.z: f32;
            4 | v.d: f32;
        ]),
        ("aabb" => v: Aabb [Aabb {position: Vector3::ZERO, size: Vector3::ZERO}] [
            4 | v.position.x: f32;
            4 | v.position.y: f32;
            4 | v.position.z: f32;
            4 | v.size.x: f32;
            4 | v.size.y: f32;
            4 | v.size.z: f32;
        ]),
        ("basis" => v: Basis [Basis {elements: [Vector3::ZERO; 3]}] [
            4 | v.elements[0].x: f32;
            4 | v.elements[0].y: f32;
            4 | v.elements[0].z: f32;
            4 | v.elements[1].x: f32;
            4 | v.elements[1].y: f32;
            4 | v.elements[1].z: f32;
            4 | v.elements[2].x: f32;
            4 | v.elements[2].y: f32;
            4 | v.elements[2].z: f32;
        ]),
        ("transform" => v: Transform [Transform {
            basis: Basis {elements: [Vector3::ZERO; 3]},
            origin: Vector3::ZERO,
        }] [
            4 | v.basis.elements[0].x: f32;
            4 | v.basis.elements[0].y: f32;
            4 | v.basis.elements[0].z: f32;
            4 | v.basis.elements[1].x: f32;
            4 | v.basis.elements[1].y: f32;
            4 | v.basis.elements[1].z: f32;
            4 | v.basis.elements[2].x: f32;
            4 | v.basis.elements[2].y: f32;
            4 | v.basis.elements[2].z: f32;
            4 | v.origin.x: f32;
            4 | v.origin.y: f32;
            4 | v.origin.z: f32;
        ]),
        ("color" => v: Color [Color {r: 0.0, g: 0.0, b: 0.0, a: 0.0}] [
            4 | v.r: f32;
            4 | v.g: f32;
            4 | v.b: f32;
            4 | v.a: f32;
        ]),
    );
}
