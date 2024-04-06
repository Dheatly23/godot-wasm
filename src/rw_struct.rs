use std::error;
use std::fmt;

use anyhow::Result as AnyResult;
use godot::prelude::*;
use nom::character::complete::{anychar, satisfy, u32 as u32_};
use nom::combinator::{map, opt};
use nom::error::{context, ContextError, ErrorKind, ParseError};
use nom::sequence::pair;
use nom::{Err as NomErr, IResult};

use crate::{bail_with_site, site_context};

enum DataType {
    Padding,
    SignedByte,
    UnsignedByte,
    SignedShort,
    UnsignedShort,
    SignedInt,
    UnsignedInt,
    SignedLong,
    UnsignedLong,
    Float,
    Double,
    Vector2(VectorSubtype),
    Vector3(VectorSubtype),
    Vector4(VectorSubtype),
    Plane(FloatSubtype),
    Quaternion(FloatSubtype),
    Color(ColorSubtype),
    Rect2(VectorSubtype),
    Aabb(FloatSubtype),
    Basis(FloatSubtype),
    Projection(FloatSubtype),
    Transform2D(FloatSubtype),
    Transform3D(FloatSubtype),
}

enum VectorSubtype {
    Float,
    Double,
    Int,
    Long,
}

fn parse_vector_subtype<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    i: &'a str,
) -> IResult<&'a str, VectorSubtype, E> {
    match anychar(i)? {
        (i, 'f') => Ok((i, VectorSubtype::Float)),
        (i, 'd') => Ok((i, VectorSubtype::Double)),
        (i, 'i') => Ok((i, VectorSubtype::Int)),
        (i, 'l') => Ok((i, VectorSubtype::Long)),
        _ => Err(NomErr::Error(E::from_error_kind(i, ErrorKind::OneOf))),
    }
}

enum ColorSubtype {
    Float,
    Double,
    Byte,
}

fn parse_color_subtype<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    i: &'a str,
) -> IResult<&'a str, ColorSubtype, E> {
    match anychar(i)? {
        (i, 'f') => Ok((i, ColorSubtype::Float)),
        (i, 'd') => Ok((i, ColorSubtype::Double)),
        (i, 'b') => Ok((i, ColorSubtype::Byte)),
        _ => Err(NomErr::Error(E::from_error_kind(i, ErrorKind::OneOf))),
    }
}

enum FloatSubtype {
    Float,
    Double,
}

fn parse_float_subtype<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    i: &'a str,
) -> IResult<&'a str, FloatSubtype, E> {
    match anychar(i)? {
        (i, 'f') => Ok((i, FloatSubtype::Float)),
        (i, 'd') => Ok((i, FloatSubtype::Double)),
        _ => Err(NomErr::Error(E::from_error_kind(i, ErrorKind::OneOf))),
    }
}

fn parse_datatype<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    i: &'a str,
) -> IResult<&'a str, DataType, E> {
    match satisfy(|c| c.is_ascii_alphabetic())(i)? {
        (i, 'x') => Ok((i, DataType::Padding)),
        (i, 'b') => Ok((i, DataType::SignedByte)),
        (i, 'B') => Ok((i, DataType::UnsignedByte)),
        (i, 'h') => Ok((i, DataType::SignedShort)),
        (i, 'H') => Ok((i, DataType::UnsignedShort)),
        (i, 'i') => Ok((i, DataType::SignedInt)),
        (i, 'I') => Ok((i, DataType::UnsignedInt)),
        (i, 'l') => Ok((i, DataType::SignedLong)),
        (i, 'L') => Ok((i, DataType::UnsignedLong)),
        (i, 'f') => Ok((i, DataType::Float)),
        (i, 'd') => Ok((i, DataType::Double)),
        (i, 'v') => {
            let e = |e: NomErr<E>| e.map(|e| E::add_context(i, "vector size", e));
            let f = context("vector element type", parse_vector_subtype);
            match anychar(i).map_err(e)? {
                (i, '2') => map(f, DataType::Vector2)(i),
                (i, '3') => map(f, DataType::Vector3)(i),
                (i, '4') => map(f, DataType::Vector4)(i),
                _ => Err(e(NomErr::Error(E::from_error_kind(i, ErrorKind::OneOf)))),
            }
        }
        (i, 'p') => context(
            "plane element type",
            map(parse_float_subtype, DataType::Plane),
        )(i),
        (i, 'q') => context(
            "quaternion element type",
            map(parse_float_subtype, DataType::Quaternion),
        )(i),
        (i, 'C') => context(
            "color element type",
            map(parse_color_subtype, DataType::Color),
        )(i),
        (i, 'r') => context(
            "rect2 element type",
            map(parse_vector_subtype, DataType::Rect2),
        )(i),
        (i, 'a') => context(
            "aabb element type",
            map(parse_float_subtype, DataType::Aabb),
        )(i),
        (i, 'm') => context(
            "basis element type",
            map(parse_float_subtype, DataType::Basis),
        )(i),
        (i, 'M') => context(
            "projection element type",
            map(parse_float_subtype, DataType::Projection),
        )(i),
        (i, 't') => context(
            "transform2d element type",
            map(parse_float_subtype, DataType::Transform2D),
        )(i),
        (i, 'T') => context(
            "transform element type",
            map(parse_float_subtype, DataType::Transform3D),
        )(i),
        _ => Err(NomErr::Error(E::from_error_kind(i, ErrorKind::OneOf))),
    }
}

struct SingleError<I> {
    input: I,
    kind: ErrorKind,
    context: Option<&'static str>,
}

impl<I: fmt::Display> fmt::Display for SingleError<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error {:?} at: {}", self.kind, self.input)?;
        if let Some(ctx) = self.context {
            write!(f, "in section '{}', at: {}", ctx, self.input)?;
        }
        Ok(())
    }
}

impl<I: fmt::Display> fmt::Debug for SingleError<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Display>::fmt(self, f)
    }
}

impl<I: fmt::Display> error::Error for SingleError<I> {}

impl<I: ToOwned + ?Sized> SingleError<&I> {
    fn into_owned(self) -> SingleError<I::Owned> {
        SingleError {
            input: self.input.to_owned(),
            kind: self.kind,
            context: self.context,
        }
    }
}

impl<I> ParseError<I> for SingleError<I> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        Self {
            input,
            kind,
            context: None,
        }
    }

    fn append(_: I, _: ErrorKind, other: Self) -> Self {
        other
    }
}

impl<I> ContextError<I> for SingleError<I> {
    fn add_context(_: I, ctx: &'static str, other: Self) -> Self {
        Self {
            context: Some(ctx),
            ..other
        }
    }
}

pub fn read_struct(data: &[u8], p: usize, mut format: &str) -> AnyResult<Array<Variant>> {
    fn f<const N: usize, T: ToGodot>(
        (data, p, a): &mut (&[u8], usize, Array<Variant>),
        n: usize,
        f: impl Fn(&[u8; N]) -> T,
    ) -> AnyResult<()> {
        for _ in 0..n {
            let s = *p;
            let e = s + N;
            let Some(data) = data.get(s..e) else {
                bail_with_site!("Index out of range ({s}..{e})")
            };
            a.push(f(data.try_into().unwrap()).to_variant());
            *p += N;
        }

        Ok(())
    }

    let mut r = (data, p, Array::new());
    let mut p_ = pair(opt(u32_), parse_datatype);
    while !format.is_empty() {
        let (i, (n, t)) = p_(format).map_err(|e| e.map(SingleError::into_owned))?;
        format = i;
        let n = n.unwrap_or(1) as usize;

        match t {
            DataType::Padding => {
                r.1 += n;
                Ok(())
            }
            DataType::SignedByte => f::<1, _>(&mut r, n, |v| v[0] as i8 as i64),
            DataType::UnsignedByte => f::<1, _>(&mut r, n, |v| v[0] as i64),
            DataType::SignedShort => f::<2, _>(&mut r, n, |v| i16::from_le_bytes(*v) as i64),
            DataType::UnsignedShort => f::<2, _>(&mut r, n, |v| u16::from_le_bytes(*v) as i64),
            DataType::SignedInt => f::<4, _>(&mut r, n, |v| i32::from_le_bytes(*v) as i64),
            DataType::UnsignedInt => f::<4, _>(&mut r, n, |v| u32::from_le_bytes(*v) as i64),
            DataType::SignedLong => f::<8, _>(&mut r, n, |v| i64::from_le_bytes(*v)),
            DataType::UnsignedLong => f::<8, _>(&mut r, n, |v| u64::from_le_bytes(*v)),
            DataType::Float => f::<4, _>(&mut r, n, |v| f32::from_le_bytes(*v)),
            DataType::Double => f::<8, _>(&mut r, n, |v| f64::from_le_bytes(*v)),
            DataType::Vector2(VectorSubtype::Float) => f::<8, _>(&mut r, n, |v| Vector2 {
                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                y: f32::from_le_bytes(v[4..].try_into().unwrap()),
            }),
            DataType::Vector2(VectorSubtype::Double) => f::<16, _>(&mut r, n, |v| Vector2 {
                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                y: f64::from_le_bytes(v[8..].try_into().unwrap()) as _,
            }),
            DataType::Vector2(VectorSubtype::Int) => f::<8, _>(&mut r, n, |v| Vector2i {
                x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                y: i32::from_le_bytes(v[4..].try_into().unwrap()),
            }),
            DataType::Vector2(VectorSubtype::Long) => f::<16, _>(&mut r, n, |v| Vector2i {
                x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                y: i64::from_le_bytes(v[8..].try_into().unwrap()) as _,
            }),
            DataType::Vector3(VectorSubtype::Float) => f::<12, _>(&mut r, n, |v| Vector3 {
                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                z: f32::from_le_bytes(v[8..].try_into().unwrap()),
            }),
            DataType::Vector3(VectorSubtype::Double) => f::<24, _>(&mut r, n, |v| Vector3 {
                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                z: f64::from_le_bytes(v[16..].try_into().unwrap()) as _,
            }),
            DataType::Vector3(VectorSubtype::Int) => f::<12, _>(&mut r, n, |v| Vector3i {
                x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                y: i32::from_le_bytes(v[4..8].try_into().unwrap()),
                z: i32::from_le_bytes(v[8..].try_into().unwrap()),
            }),
            DataType::Vector3(VectorSubtype::Long) => f::<24, _>(&mut r, n, |v| Vector3i {
                x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                y: i64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                z: i64::from_le_bytes(v[16..].try_into().unwrap()) as _,
            }),
            DataType::Vector4(VectorSubtype::Float) => f::<16, _>(&mut r, n, |v| Vector4 {
                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                w: f32::from_le_bytes(v[12..].try_into().unwrap()),
            }),
            DataType::Vector4(VectorSubtype::Double) => f::<32, _>(&mut r, n, |v| Vector4 {
                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                w: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
            }),
            DataType::Vector4(VectorSubtype::Int) => f::<16, _>(&mut r, n, |v| Vector4i {
                x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                y: i32::from_le_bytes(v[4..8].try_into().unwrap()),
                z: i32::from_le_bytes(v[8..12].try_into().unwrap()),
                w: i32::from_le_bytes(v[12..].try_into().unwrap()),
            }),
            DataType::Vector4(VectorSubtype::Long) => f::<32, _>(&mut r, n, |v| Vector4i {
                x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                y: i64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                z: i64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                w: i64::from_le_bytes(v[24..].try_into().unwrap()) as _,
            }),
            DataType::Plane(FloatSubtype::Float) => f::<16, _>(&mut r, n, |v| Plane {
                normal: Vector3 {
                    x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                },
                d: f32::from_le_bytes(v[12..].try_into().unwrap()),
            }),
            DataType::Plane(FloatSubtype::Double) => f::<32, _>(&mut r, n, |v| Plane {
                normal: Vector3 {
                    x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                    z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                },
                d: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
            }),
            DataType::Quaternion(FloatSubtype::Float) => f::<16, _>(&mut r, n, |v| Quaternion {
                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                w: f32::from_le_bytes(v[12..].try_into().unwrap()),
            }),
            DataType::Quaternion(FloatSubtype::Double) => f::<32, _>(&mut r, n, |v| Quaternion {
                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                w: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
            }),
            DataType::Color(ColorSubtype::Float) => f::<16, _>(&mut r, n, |v| Color {
                r: f32::from_le_bytes(v[..4].try_into().unwrap()),
                g: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                b: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                a: f32::from_le_bytes(v[12..].try_into().unwrap()),
            }),
            DataType::Color(ColorSubtype::Double) => f::<32, _>(&mut r, n, |v| Color {
                r: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                g: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                b: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                a: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
            }),
            DataType::Color(ColorSubtype::Byte) => {
                f::<4, _>(&mut r, n, |&[r, g, b, a]| Color::from_rgba8(r, g, b, a))
            }
            DataType::Rect2(VectorSubtype::Float) => f::<16, _>(&mut r, n, |v| Rect2 {
                position: Vector2 {
                    x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                },
                size: Vector2 {
                    x: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    y: f32::from_le_bytes(v[12..].try_into().unwrap()),
                },
            }),
            DataType::Rect2(VectorSubtype::Double) => f::<32, _>(&mut r, n, |v| Rect2 {
                position: Vector2 {
                    x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                },
                size: Vector2 {
                    x: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                },
            }),
            DataType::Rect2(VectorSubtype::Int) => f::<16, _>(&mut r, n, |v| Rect2i {
                position: Vector2i {
                    x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                    y: i32::from_le_bytes(v[4..8].try_into().unwrap()),
                },
                size: Vector2i {
                    x: i32::from_le_bytes(v[8..12].try_into().unwrap()),
                    y: i32::from_le_bytes(v[12..].try_into().unwrap()),
                },
            }),
            DataType::Rect2(VectorSubtype::Long) => f::<32, _>(&mut r, n, |v| Rect2i {
                position: Vector2i {
                    x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                    y: i64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                },
                size: Vector2i {
                    x: i64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    y: i64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                },
            }),
            DataType::Aabb(FloatSubtype::Float) => f::<24, _>(&mut r, n, |v| Aabb {
                position: Vector3 {
                    x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                },
                size: Vector3 {
                    x: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                    y: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                    z: f32::from_le_bytes(v[20..].try_into().unwrap()),
                },
            }),
            DataType::Aabb(FloatSubtype::Double) => f::<48, _>(&mut r, n, |v| Aabb {
                position: Vector3 {
                    x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                    z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                },
                size: Vector3 {
                    x: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                    z: f64::from_le_bytes(v[40..].try_into().unwrap()) as _,
                },
            }),
            DataType::Basis(FloatSubtype::Float) => f::<36, _>(&mut r, n, |v| Basis {
                rows: [
                    Vector3 {
                        x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                        z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    },
                    Vector3 {
                        x: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                        y: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                        z: f32::from_le_bytes(v[20..24].try_into().unwrap()),
                    },
                    Vector3 {
                        x: f32::from_le_bytes(v[24..28].try_into().unwrap()),
                        y: f32::from_le_bytes(v[28..32].try_into().unwrap()),
                        z: f32::from_le_bytes(v[32..].try_into().unwrap()),
                    },
                ],
            }),
            DataType::Basis(FloatSubtype::Double) => f::<72, _>(&mut r, n, |v| Basis {
                rows: [
                    Vector3 {
                        x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    },
                    Vector3 {
                        x: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[40..48].try_into().unwrap()) as _,
                    },
                    Vector3 {
                        x: f64::from_le_bytes(v[48..56].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[56..64].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[64..].try_into().unwrap()) as _,
                    },
                ],
            }),
            DataType::Projection(FloatSubtype::Float) => f::<64, _>(&mut r, n, |v| Projection {
                cols: [
                    Vector4 {
                        x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                        z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                        w: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                    },
                    Vector4 {
                        x: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                        y: f32::from_le_bytes(v[20..24].try_into().unwrap()),
                        z: f32::from_le_bytes(v[24..28].try_into().unwrap()),
                        w: f32::from_le_bytes(v[28..32].try_into().unwrap()),
                    },
                    Vector4 {
                        x: f32::from_le_bytes(v[32..36].try_into().unwrap()),
                        y: f32::from_le_bytes(v[36..40].try_into().unwrap()),
                        z: f32::from_le_bytes(v[40..44].try_into().unwrap()),
                        w: f32::from_le_bytes(v[44..48].try_into().unwrap()),
                    },
                    Vector4 {
                        x: f32::from_le_bytes(v[48..52].try_into().unwrap()),
                        y: f32::from_le_bytes(v[52..56].try_into().unwrap()),
                        z: f32::from_le_bytes(v[56..60].try_into().unwrap()),
                        w: f32::from_le_bytes(v[60..].try_into().unwrap()),
                    },
                ],
            }),
            DataType::Projection(FloatSubtype::Double) => f::<128, _>(&mut r, n, |v| Projection {
                cols: [
                    Vector4 {
                        x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                        w: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                    },
                    Vector4 {
                        x: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[40..48].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[48..56].try_into().unwrap()) as _,
                        w: f64::from_le_bytes(v[56..64].try_into().unwrap()) as _,
                    },
                    Vector4 {
                        x: f64::from_le_bytes(v[64..72].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[72..80].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[80..88].try_into().unwrap()) as _,
                        w: f64::from_le_bytes(v[88..96].try_into().unwrap()) as _,
                    },
                    Vector4 {
                        x: f64::from_le_bytes(v[96..104].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[104..112].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[112..120].try_into().unwrap()) as _,
                        w: f64::from_le_bytes(v[120..].try_into().unwrap()) as _,
                    },
                ],
            }),
            DataType::Transform2D(FloatSubtype::Float) => f::<24, _>(&mut r, n, |v| Transform2D {
                a: Vector2 {
                    x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                },
                b: Vector2 {
                    x: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    y: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                },
                origin: Vector2 {
                    x: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                    y: f32::from_le_bytes(v[20..].try_into().unwrap()),
                },
            }),
            DataType::Transform2D(FloatSubtype::Double) => f::<48, _>(&mut r, n, |v| Transform2D {
                a: Vector2 {
                    x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                },
                b: Vector2 {
                    x: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                },
                origin: Vector2 {
                    x: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[40..].try_into().unwrap()) as _,
                },
            }),
            DataType::Transform3D(FloatSubtype::Float) => f::<48, _>(&mut r, n, |v| Transform3D {
                basis: Basis {
                    rows: [
                        Vector3 {
                            x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                            y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                            z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                        },
                        Vector3 {
                            x: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                            y: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                            z: f32::from_le_bytes(v[20..24].try_into().unwrap()),
                        },
                        Vector3 {
                            x: f32::from_le_bytes(v[24..28].try_into().unwrap()),
                            y: f32::from_le_bytes(v[28..32].try_into().unwrap()),
                            z: f32::from_le_bytes(v[32..36].try_into().unwrap()),
                        },
                    ],
                },
                origin: Vector3 {
                    x: f32::from_le_bytes(v[36..40].try_into().unwrap()),
                    y: f32::from_le_bytes(v[40..44].try_into().unwrap()),
                    z: f32::from_le_bytes(v[44..].try_into().unwrap()),
                },
            }),
            DataType::Transform3D(FloatSubtype::Double) => f::<96, _>(&mut r, n, |v| Transform3D {
                basis: Basis {
                    rows: [
                        Vector3 {
                            x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                        },
                        Vector3 {
                            x: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[40..48].try_into().unwrap()) as _,
                        },
                        Vector3 {
                            x: f64::from_le_bytes(v[48..56].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[56..64].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[64..72].try_into().unwrap()) as _,
                        },
                    ],
                },
                origin: Vector3 {
                    x: f64::from_le_bytes(v[72..80].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[80..88].try_into().unwrap()) as _,
                    z: f64::from_le_bytes(v[88..].try_into().unwrap()) as _,
                },
            }),
        }?;
    }

    Ok(r.2)
}

pub fn write_struct(
    data: &mut [u8],
    p: usize,
    mut format: &str,
    arr: Array<Variant>,
) -> AnyResult<usize> {
    fn f<const N: usize, T: FromGodot>(
        (data, p, a): &mut (&mut [u8], usize, impl Iterator<Item = Variant>),
        n: usize,
        f: impl Fn(&mut [u8; N], T),
    ) -> AnyResult<()> {
        for _ in 0..n {
            let Some(v) = a
                .next()
                .map(|v| site_context!(T::try_from_variant(&v)))
                .transpose()?
            else {
                bail_with_site!("Input array too small")
            };
            let s = *p;
            let e = s + N;
            let Some(data) = data.get_mut(s..e) else {
                bail_with_site!("Index out of range ({s}..{e})")
            };
            f(data.try_into().unwrap(), v);
            *p += N;
        }

        Ok(())
    }

    let mut r = (data, p, arr.iter_shared());
    let mut p_ = pair(opt(u32_), parse_datatype);
    while !format.is_empty() {
        let (i, (n, t)) = p_(format).map_err(|e| e.map(SingleError::into_owned))?;
        format = i;
        let n = n.unwrap_or(1) as usize;

        match t {
            DataType::Padding => {
                r.1 += n;
                Ok(())
            }
            DataType::SignedByte => f::<1, i64>(&mut r, n, |s, d| s[0] = d as i8 as u8),
            DataType::UnsignedByte => f::<1, i64>(&mut r, n, |s, d| s[0] = d as u8),
            DataType::SignedShort => f::<2, i64>(&mut r, n, |s, d| *s = (d as i16).to_le_bytes()),
            DataType::UnsignedShort => f::<2, i64>(&mut r, n, |s, d| *s = (d as u16).to_le_bytes()),
            DataType::SignedInt => f::<4, i64>(&mut r, n, |s, d| *s = (d as i32).to_le_bytes()),
            DataType::UnsignedInt => f::<4, i64>(&mut r, n, |s, d| *s = (d as u32).to_le_bytes()),
            DataType::SignedLong | DataType::UnsignedLong => {
                f::<8, i64>(&mut r, n, |s, d| *s = d.to_le_bytes())
            }
            DataType::Float => f::<4, f32>(&mut r, n, |s, d| *s = d.to_le_bytes()),
            DataType::Double => f::<8, f64>(&mut r, n, |s, d| *s = d.to_le_bytes()),
            DataType::Vector2(VectorSubtype::Float) => f::<8, Vector2>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..]).unwrap() = d.y.to_le_bytes();
            }),
            DataType::Vector2(VectorSubtype::Double) => f::<16, Vector2>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..]).unwrap() = (d.y as f64).to_le_bytes();
            }),
            DataType::Vector2(VectorSubtype::Int) => f::<8, Vector2>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = (d.x as i32).to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..]).unwrap() = (d.y as i32).to_le_bytes();
            }),
            DataType::Vector2(VectorSubtype::Long) => f::<16, Vector2>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..]).unwrap() = (d.y as i64).to_le_bytes();
            }),
            DataType::Vector3(VectorSubtype::Float) => f::<12, Vector3>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..]).unwrap() = d.z.to_le_bytes();
            }),
            DataType::Vector3(VectorSubtype::Double) => f::<24, Vector3>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..]).unwrap() = (d.z as f64).to_le_bytes();
            }),
            DataType::Vector3(VectorSubtype::Int) => f::<12, Vector3>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = (d.x as i32).to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = (d.y as i32).to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..]).unwrap() = (d.z as i32).to_le_bytes();
            }),
            DataType::Vector3(VectorSubtype::Long) => f::<24, Vector3>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.y as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..]).unwrap() = (d.z as i64).to_le_bytes();
            }),
            DataType::Vector4(VectorSubtype::Float) => f::<16, Vector4>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.w.to_le_bytes();
            }),
            DataType::Vector4(VectorSubtype::Double) => f::<32, Vector4>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() = (d.z as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.w as f64).to_le_bytes();
            }),
            DataType::Vector4(VectorSubtype::Int) => f::<16, Vector4i>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.w.to_le_bytes();
            }),
            DataType::Vector4(VectorSubtype::Long) => f::<32, Vector4i>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.y as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() = (d.z as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.w as i64).to_le_bytes();
            }),
            DataType::Plane(FloatSubtype::Float) => f::<16, Plane>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.normal.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.normal.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.normal.z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.d.to_le_bytes();
            }),
            DataType::Plane(FloatSubtype::Double) => f::<32, Plane>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.normal.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                    (d.normal.y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                    (d.normal.z as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.d as f64).to_le_bytes();
            }),
            DataType::Quaternion(FloatSubtype::Float) => f::<16, Quaternion>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.w.to_le_bytes();
            }),
            DataType::Quaternion(FloatSubtype::Double) => f::<32, Quaternion>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() = (d.z as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.w as f64).to_le_bytes();
            }),
            DataType::Color(ColorSubtype::Float) => f::<16, Color>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.r.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.g.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.b.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.a.to_le_bytes();
            }),
            DataType::Color(ColorSubtype::Double) => f::<32, Color>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.r as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.g as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() = (d.b as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.a as f64).to_le_bytes();
            }),
            DataType::Color(ColorSubtype::Byte) => f::<4, Color>(&mut r, n, |s, d| {
                *s = [
                    (d.r * 255.) as u8,
                    (d.g * 255.) as u8,
                    (d.b * 255.) as u8,
                    (d.a * 255.) as u8,
                ];
            }),
            DataType::Rect2(VectorSubtype::Float) => f::<16, Rect2>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.position.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.position.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.size.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.size.y.to_le_bytes();
            }),
            DataType::Rect2(VectorSubtype::Double) => f::<32, Rect2>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                    (d.position.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                    (d.position.y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                    (d.size.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.size.y as f64).to_le_bytes();
            }),
            DataType::Rect2(VectorSubtype::Int) => f::<16, Rect2>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() =
                    (d.position.x as i32).to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() =
                    (d.position.y as i32).to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = (d.size.x as i32).to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = (d.size.y as i32).to_le_bytes();
            }),
            DataType::Rect2(VectorSubtype::Long) => f::<32, Rect2>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                    (d.position.x as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                    (d.position.y as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                    (d.size.x as i64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.size.y as i64).to_le_bytes();
            }),
            DataType::Aabb(FloatSubtype::Float) => f::<24, Aabb>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.position.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.position.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.position.z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.size.x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.size.y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[20..]).unwrap() = d.size.z.to_le_bytes();
            }),
            DataType::Aabb(FloatSubtype::Double) => f::<48, Aabb>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                    (d.position.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                    (d.position.y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                    (d.position.z as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                    (d.size.x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                    (d.size.y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[40..]).unwrap() = (d.size.z as f64).to_le_bytes();
            }),
            DataType::Basis(FloatSubtype::Float) => f::<36, Basis>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.rows[0].x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.rows[0].y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.rows[0].z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.rows[1].x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.rows[1].y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[20..24]).unwrap() = d.rows[1].z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[24..28]).unwrap() = d.rows[2].x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[28..32]).unwrap() = d.rows[2].y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[32..]).unwrap() = d.rows[2].z.to_le_bytes();
            }),
            DataType::Basis(FloatSubtype::Double) => f::<72, Basis>(&mut r, n, |s, d| {
                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                    (d.rows[0].x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                    (d.rows[0].y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                    (d.rows[0].z as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                    (d.rows[1].x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                    (d.rows[1].y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[40..48]).unwrap() =
                    (d.rows[1].z as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[48..56]).unwrap() =
                    (d.rows[2].x as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[56..64]).unwrap() =
                    (d.rows[2].y as f64).to_le_bytes();
                *<&mut [u8; 8]>::try_from(&mut s[64..]).unwrap() =
                    (d.rows[2].z as f64).to_le_bytes();
            }),
            DataType::Projection(FloatSubtype::Float) => f::<64, Projection>(&mut r, n, |s, d| {
                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.cols[0].x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.cols[0].y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.cols[0].z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.cols[0].w.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.cols[1].x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[20..24]).unwrap() = d.cols[1].y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[24..28]).unwrap() = d.cols[1].z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[28..32]).unwrap() = d.cols[1].w.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[32..36]).unwrap() = d.cols[2].x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[36..40]).unwrap() = d.cols[2].y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[40..44]).unwrap() = d.cols[2].z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[44..48]).unwrap() = d.cols[2].w.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[48..52]).unwrap() = d.cols[3].x.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[52..56]).unwrap() = d.cols[3].y.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[56..60]).unwrap() = d.cols[3].z.to_le_bytes();
                *<&mut [u8; 4]>::try_from(&mut s[60..]).unwrap() = d.cols[3].w.to_le_bytes();
            }),
            DataType::Projection(FloatSubtype::Double) => {
                f::<128, Projection>(&mut r, n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.cols[0].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.cols[0].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.cols[0].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.cols[0].w as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.cols[1].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..48]).unwrap() =
                        (d.cols[1].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[48..56]).unwrap() =
                        (d.cols[1].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[56..64]).unwrap() =
                        (d.cols[1].w as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[64..72]).unwrap() =
                        (d.cols[2].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[72..80]).unwrap() =
                        (d.cols[2].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[80..88]).unwrap() =
                        (d.cols[2].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[88..96]).unwrap() =
                        (d.cols[2].w as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[96..104]).unwrap() =
                        (d.cols[3].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[104..112]).unwrap() =
                        (d.cols[3].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[112..120]).unwrap() =
                        (d.cols[3].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[120..]).unwrap() =
                        (d.cols[3].w as f64).to_le_bytes();
                })
            }
            DataType::Transform2D(FloatSubtype::Float) => {
                f::<24, Transform2D>(&mut r, n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.a.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.a.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.b.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.b.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.origin.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[20..]).unwrap() = d.origin.y.to_le_bytes();
                })
            }
            DataType::Transform2D(FloatSubtype::Double) => {
                f::<48, Transform2D>(&mut r, n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.a.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.a.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.b.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.b.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.origin.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..]).unwrap() =
                        (d.origin.y as f64).to_le_bytes();
                })
            }
            DataType::Transform3D(FloatSubtype::Float) => {
                f::<48, Transform3D>(&mut r, n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() =
                        d.basis.rows[0].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() =
                        d.basis.rows[0].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() =
                        d.basis.rows[0].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() =
                        d.basis.rows[1].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() =
                        d.basis.rows[1].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[20..24]).unwrap() =
                        d.basis.rows[1].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[24..28]).unwrap() =
                        d.basis.rows[2].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[28..32]).unwrap() =
                        d.basis.rows[2].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[32..36]).unwrap() =
                        d.basis.rows[2].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[36..40]).unwrap() = d.origin.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[40..44]).unwrap() = d.origin.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[44..]).unwrap() = d.origin.z.to_le_bytes();
                })
            }
            DataType::Transform3D(FloatSubtype::Double) => {
                f::<96, Transform3D>(&mut r, n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.basis.rows[0].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.basis.rows[0].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.basis.rows[0].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.basis.rows[1].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.basis.rows[1].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..48]).unwrap() =
                        (d.basis.rows[1].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[48..56]).unwrap() =
                        (d.basis.rows[2].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[56..64]).unwrap() =
                        (d.basis.rows[2].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[64..72]).unwrap() =
                        (d.basis.rows[2].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[72..80]).unwrap() =
                        (d.origin.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[80..88]).unwrap() =
                        (d.origin.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[88..]).unwrap() =
                        (d.origin.z as f64).to_le_bytes();
                })
            }
        }?;
    }

    Ok(r.1 - p)
}
