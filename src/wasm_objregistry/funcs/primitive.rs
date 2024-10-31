#![allow(unused_parens, unused_assignments)]

use std::mem::{size_of, size_of_val};

use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::godot_util::from_var_any;
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

macro_rules! prim_value {
    (#writer $tx:ty as $ti:ty) => {$ti};
    (#writer $tx:ty) => {$tx};
    (#reader $x:ident as $ti:ty) => {<$ti>::from($x)};
    (#reader $x:ident) => {$x};
    ($((
        $head:tt => <$tv:ty>
        ($($x:ident : $tx:ty $(as $ti:ty)?),* $(,)?)
        $($v:tt)*
    )),* $(,)?) => {$(
        func_registry!{
            $head,
            get => |ctx: Caller<'_, T>, i: u32| -> Result<($($tx),*), Error> {
                let v = ctx.data().as_ref().get_registry()?.get_or_nil(i as _);
                let $($v)* = site_context!(from_var_any::<$tv>(&v))?;
                Ok(($($x.into()),*))
            },
            set => |mut ctx: Caller<'_, T>, i: u32, $($x : $tx),*| -> Result<(), Error> {
                let v = $($v)*;
                ctx.data_mut().as_mut().get_registry_mut()?.replace(i as _, <$tv>::from(v).to_variant());
                Ok(())
            },
            new => |mut ctx: Caller<'_, T>, $($x : $tx),*| -> Result<u32, Error> {
                let v = $($v)*;
                Ok(ctx.data_mut().as_mut().get_registry_mut()?.register(v.to_variant()) as _)
            },
            read => |mut ctx: Caller<'_, T>, i: u32, p: u32| -> Result<u32, Error> {
                let $($v)* = site_context!(from_var_any::<$tv>(&ctx.data().as_ref().get_registry()?.get_or_nil(i as _)))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                $({
                    let v = prim_value!(#reader $x $(as $ti)?);
                    site_context!(mem.write(
                        &mut ctx,
                        p,
                        &v.to_le_bytes(),
                    ))?;
                    p += size_of_val(&v);
                })*

                Ok(1)
            },
            write => |mut ctx: Caller<'_, T>, i: u32, p: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let mut p = p as usize;
                $(
                    let $x: $tx = {
                        const SIZE: usize = size_of::<prim_value!(#writer $tx $(as $ti)?)>();
                        let mut s = [0u8; SIZE];
                        site_context!(mem.read(&ctx, p, &mut s))?;
                        p += SIZE;
                        <prim_value!(#writer $tx $(as $ti)?)>::from_le_bytes(s).into()
                    };
                )*

                let v = <$tv>::from($($v)*);
                ctx.data_mut().as_mut().get_registry_mut()?.replace(i as _, v.to_variant());
                Ok(1)
            },
        }
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

impl GodotConvert for BoolWrapper {
    type Via = bool;
}

impl FromGodot for BoolWrapper {
    fn try_from_godot(via: bool) -> Result<Self, ConvertError> {
        Ok(Self(via.into()))
    }
}

impl ToGodot for BoolWrapper {
    type ToVia<'a> = bool;

    fn to_godot(&self) -> bool {
        self.0 != 0
    }
}

impl From<BoolWrapper> for u32 {
    #[inline]
    fn from(v: BoolWrapper) -> Self {
        v.0 as _
    }
}

impl From<u32> for BoolWrapper {
    #[inline]
    fn from(v: u32) -> Self {
        Self((v != 0).into())
    }
}

#[derive(Default)]
pub struct Funcs {
    r#bool: BoolFuncs,
    int: IntFuncs,
    float: FloatFuncs,
    vector2: Vector2Funcs,
    vector2i: Vector2iFuncs,
    vector3: Vector3Funcs,
    vector3i: Vector3iFuncs,
    vector4: Vector4Funcs,
    vector4i: Vector4iFuncs,
    quat: QuatFuncs,
    rect2: Rect2Funcs,
    rect2i: Rect2iFuncs,
    transform2d: Transform2DFuncs,
    plane: PlaneFuncs,
    aabb: AabbFuncs,
    basis: BasisFuncs,
    projection: ProjectionFuncs,
    transform3d: Transform3DFuncs,
    color: ColorFuncs,
}

impl Funcs {
    pub fn get_func<T>(&mut self, store: &mut StoreContextMut<'_, T>, name: &str) -> Option<Func>
    where
        T: AsRef<StoreData> + AsMut<StoreData>,
    {
        if let r @ Some(_) = self.r#bool.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.int.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.float.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector2.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector2i.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector3.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector3i.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector4.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.vector4i.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.quat.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.rect2.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.rect2i.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.transform2d.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.plane.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.aabb.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.basis.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.projection.get_func(&mut *store, name) {
            r
        } else if let r @ Some(_) = self.transform3d.get_func(&mut *store, name) {
            r
        } else {
            self.color.get_func(&mut *store, name)
        }
    }
}

prim_value! {
    ((BoolFuncs, "bool.") => <BoolWrapper> (v: u32 as BoolWrapper) v),
    ((IntFuncs, "int.") => <i64> (v: i64) v),
    ((FloatFuncs, "float.") => <f64> (v: f64) v),
    ((Vector2Funcs, "vector2.") => <Vector2> (x: f32, y: f32) Vector2 {x, y}),
    ((Vector2iFuncs, "vector2i.") => <Vector2i> (x: i32, y: i32) Vector2i {x, y}),
    ((Vector3Funcs, "vector3.") => <Vector3> (x: f32, y: f32, z: f32) Vector3 {x, y, z}),
    ((Vector3iFuncs, "vector3i.") => <Vector3i> (x: i32, y: i32, z: i32) Vector3i {x, y, z}),
    ((Vector4Funcs, "vector4.") => <Vector4> (x: f32, y: f32, z: f32, w: f32) Vector4 {x, y, z, w}),
    ((Vector4iFuncs, "vector4i.") => <Vector4i> (x: i32, y: i32, z: i32, w: i32) Vector4i {x, y, z, w}),
    ((QuatFuncs, "quat.") => <Quaternion> (x: f32, y: f32, z: f32, w: f32) Quaternion {x, y, z, w}),
    ((Rect2Funcs, "rect2.") => <Rect2> (x: f32, y: f32, w: f32, h: f32) Rect2 {
        position: Vector2 {x, y},
        size: Vector2 {x: w, y: h},
    }),
    ((Rect2iFuncs, "rect2.") => <Rect2i> (x: i32, y: i32, w: i32, h: i32) Rect2i {
        position: Vector2i {x, y},
        size: Vector2i {x: w, y: h},
    }),
    ((Transform2DFuncs, "transform2d.") =>
        <Transform2D>
        (ax: f32, ay: f32, bx: f32, by: f32, ox: f32, oy: f32)
        Transform2D {
            a: Vector2 {x: ax, y: ay},
            b: Vector2 {x: bx, y: by},
            origin: Vector2 {x: ox, y: oy},
        }
    ),
    ((PlaneFuncs, "plane.") => <Plane> (a: f32, b: f32, c: f32, d: f32) Plane {
        normal: Vector3 {x: a, y: b, z: c},
        d,
    }),
    ((AabbFuncs, "aabb.") => <Aabb> (x: f32, y: f32, z: f32, w: f32, h: f32, t: f32) Aabb {
        position : Vector3 {x, y, z},
        size: Vector3 {x: w, y: h, z: t},
    }),
    ((BasisFuncs, "basis.") =>
        <Basis>
        (ax: f32, ay: f32, az: f32, bx: f32, by: f32, bz: f32, cx: f32, cy: f32, cz: f32)
        Basis {
            rows: [
                Vector3 {x: ax, y: ay, z: az},
                Vector3 {x: bx, y: by, z: bz},
                Vector3 {x: cx, y: cy, z: cz},
            ],
        }
    ),
    ((Transform3DFuncs, "transform3D.") =>
        <Transform3D>
        (
            ax: f32, ay: f32, az: f32,
            bx: f32, by: f32, bz: f32,
            cx: f32, cy: f32, cz: f32,
            ox: f32, oy: f32, oz: f32,
        )
        Transform3D {
            basis: Basis {
                rows: [
                    Vector3 {x: ax, y: ay, z: az},
                    Vector3 {x: bx, y: by, z: bz},
                    Vector3 {x: cx, y: cy, z: cz},
                ],
            },
            origin: Vector3 {x: ox, y: oy, z: oz},
        }
    ),
    ((ColorFuncs, "color.") => <Color> (r: f32, g: f32, b: f32, a: f32) Color {r, g, b, a}),
}

func_registry! {
    (ProjectionFuncs, "projection."),
    read => |mut ctx: Caller<'_, T>, i: u32, p: u32| -> Result<u32, Error> {
        let v = site_context!(from_var_any::<Projection>(&ctx.data().as_ref().get_registry()?.get_or_nil(i as _)))?;
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let mut p = p as usize;
        for i in v.cols {
            for i in [i.x, i.y, i.z, i.w] {
                site_context!(mem.write(
                    &mut ctx,
                    p,
                    &i.to_le_bytes(),
                ))?;
                p += 4;
            }
        }

        Ok(1)
    },
    write => |mut ctx: Caller<'_, T>, i: u32, p: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let mut p = p as usize;
        let mut cols = [Vector4::default(); 4];
        for Vector4 { x, y, z, w } in &mut cols {
            for i in [x, y, z, w] {
                let mut s = [0u8; 4];
                site_context!(mem.read(
                    &mut ctx,
                    p,
                    &mut s,
                ))?;
                *i = f32::from_le_bytes(s);
                p += 4;
            }
        }

        let v = Projection { cols };
        ctx.data_mut().as_mut().get_registry_mut()?.replace(i as _, v.to_variant());
        Ok(1)
    },
}
