use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult, Write as _};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Seek, Write};

use anyhow::{Error as AnyError, Result as AnyResult};
use godot::prelude::*;
use nom::character::complete::{anychar, satisfy, u32 as u32_};
use nom::combinator::{map, opt};
use nom::error::{ContextError, ErrorKind, ParseError, context};
use nom::sequence::pair;
use nom::{AsChar, Compare, CompareResult, Err as NomErr, IResult, Input, Needed, Offset, Parser};

use crate::godot_util::{StructPacking, from_var_any};
use crate::{bail_with_site, site_context};

#[derive(Clone)]
pub struct CharSlice<'a>(pub &'a [char]);

impl Display for CharSlice<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        for &c in self.0 {
            f.write_char(c)?;
        }
        Ok(())
    }
}

impl Debug for CharSlice<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_char('"')?;
        for &c in self.0 {
            Display::fmt(&c.escape_debug(), f)?;
        }
        f.write_char('"')
    }
}

impl Offset for CharSlice<'_> {
    #[inline]
    fn offset(&self, o: &Self) -> usize {
        // SAFETY: First and second should be coming from same slice
        unsafe { o.0.as_ptr().offset_from(self.0.as_ptr()) as _ }
    }
}

#[inline]
fn cmp_char_iter(
    a: impl IntoIterator<Item = char>,
    b: impl IntoIterator<Item = char>,
) -> CompareResult {
    let (mut a, mut b) = (a.into_iter(), b.into_iter());
    loop {
        match (a.next(), b.next()) {
            (_, None) => break CompareResult::Ok,
            (Some(a), Some(b)) if a != b => break CompareResult::Error,
            (None, Some(_)) => break CompareResult::Incomplete,
            _ => (),
        }
    }
}

impl<'b> Compare<CharSlice<'b>> for CharSlice<'_> {
    fn compare(&self, o: CharSlice<'b>) -> CompareResult {
        let (s, o) = (self.0, o.0);
        let l = s.len().min(o.len());
        if s[..l] == o[..l] {
            if l == o.len() {
                CompareResult::Ok
            } else {
                CompareResult::Incomplete
            }
        } else {
            CompareResult::Error
        }
    }

    fn compare_no_case(&self, o: CharSlice<'b>) -> CompareResult {
        cmp_char_iter(
            self.0.iter().flat_map(|c| c.to_lowercase()),
            o.0.iter().flat_map(|c| c.to_lowercase()),
        )
    }
}

impl<'b> Compare<&'b str> for CharSlice<'_> {
    fn compare(&self, o: &'b str) -> CompareResult {
        cmp_char_iter(self.0.iter().copied(), o.chars())
    }

    fn compare_no_case(&self, o: &'b str) -> CompareResult {
        cmp_char_iter(
            self.0.iter().flat_map(|c| c.to_lowercase()),
            o.chars().flat_map(|c| c.to_lowercase()),
        )
    }
}

impl<'a> Input for CharSlice<'a> {
    type Item = char;
    type Iter = std::iter::Copied<std::slice::Iter<'a, char>>;
    type IterIndices = std::iter::Enumerate<Self::Iter>;

    fn input_len(&self) -> usize {
        self.0.len()
    }

    fn take(&self, index: usize) -> Self {
        Self(&self.0[..index])
    }

    fn take_from(&self, index: usize) -> Self {
        Self(&self.0[index..])
    }

    fn take_split(&self, index: usize) -> (Self, Self) {
        let (a, b) = self.0.split_at(index);
        (Self(b), Self(a))
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.0.iter().position(|&c| predicate(c))
    }

    fn iter_elements(&self) -> Self::Iter {
        self.0.iter().copied()
    }

    fn iter_indices(&self) -> Self::IterIndices {
        self.iter_elements().enumerate()
    }

    fn slice_index(&self, count: usize) -> Result<usize, Needed> {
        if let Some(v @ 1..) = count.checked_sub(self.0.len()) {
            Err(Needed::new(v))
        } else {
            Ok(count)
        }
    }
}

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

fn parse_vector_subtype<I, E>(i: I) -> IResult<I, VectorSubtype, E>
where
    E: ParseError<I> + ContextError<I>,
    I: Clone + Input,
    <I as Input>::Item: AsChar,
{
    match anychar(i.clone())? {
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

fn parse_color_subtype<I, E>(i: I) -> IResult<I, ColorSubtype, E>
where
    E: ParseError<I> + ContextError<I>,
    I: Clone + Input,
    <I as Input>::Item: AsChar,
{
    match anychar(i.clone())? {
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

fn parse_float_subtype<I, E>(i: I) -> IResult<I, FloatSubtype, E>
where
    E: ParseError<I> + ContextError<I>,
    I: Clone + Input,
    <I as Input>::Item: AsChar,
{
    match anychar(i.clone())? {
        (i, 'f') => Ok((i, FloatSubtype::Float)),
        (i, 'd') => Ok((i, FloatSubtype::Double)),
        _ => Err(NomErr::Error(E::from_error_kind(i, ErrorKind::OneOf))),
    }
}

fn parse_datatype<I, E>(i: I) -> IResult<I, DataType, E>
where
    E: ParseError<I> + ContextError<I>,
    I: Clone + Input,
    <I as Input>::Item: AsChar,
{
    match satisfy(|c| c.is_ascii_alphabetic())(i.clone())? {
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
            let e = |e: NomErr<E>| e.map(|e| E::add_context(i.clone(), "vector size", e));
            let f = context("vector element type", parse_vector_subtype);
            match anychar(i.clone()).map_err(e)? {
                (i, '2') => map(f, DataType::Vector2).parse_complete(i),
                (i, '3') => map(f, DataType::Vector3).parse_complete(i),
                (i, '4') => map(f, DataType::Vector4).parse_complete(i),
                _ => Err(e(NomErr::Error(E::from_error_kind(
                    i.clone(),
                    ErrorKind::OneOf,
                )))),
            }
        }
        (i, 'p') => context(
            "plane element type",
            map(parse_float_subtype, DataType::Plane),
        )
        .parse_complete(i),
        (i, 'q') => context(
            "quaternion element type",
            map(parse_float_subtype, DataType::Quaternion),
        )
        .parse_complete(i),
        (i, 'C') => context(
            "color element type",
            map(parse_color_subtype, DataType::Color),
        )
        .parse_complete(i),
        (i, 'r') => context(
            "rect2 element type",
            map(parse_vector_subtype, DataType::Rect2),
        )
        .parse_complete(i),
        (i, 'a') => context(
            "aabb element type",
            map(parse_float_subtype, DataType::Aabb),
        )
        .parse_complete(i),
        (i, 'm') => context(
            "basis element type",
            map(parse_float_subtype, DataType::Basis),
        )
        .parse_complete(i),
        (i, 'M') => context(
            "projection element type",
            map(parse_float_subtype, DataType::Projection),
        )
        .parse_complete(i),
        (i, 't') => context(
            "transform2d element type",
            map(parse_float_subtype, DataType::Transform2D),
        )
        .parse_complete(i),
        (i, 'T') => context(
            "transform element type",
            map(parse_float_subtype, DataType::Transform3D),
        )
        .parse_complete(i),
        _ => Err(NomErr::Error(E::from_error_kind(i, ErrorKind::OneOf))),
    }
}

pub struct SingleError<I> {
    input: I,
    kind: ErrorKind,
    context: Option<&'static str>,
}

impl<I: Debug> Display for SingleError<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "error {:?} at: {:?}", self.kind, self.input)?;
        if let Some(ctx) = self.context {
            write!(f, "in section '{}', at: {:?}", ctx, self.input)?;
        }
        Ok(())
    }
}

impl<I: Debug> Debug for SingleError<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Display>::fmt(self, f)
    }
}

impl<I: Debug> Error for SingleError<I> {}

impl SingleError<CharSlice<'_>> {
    pub fn into_owned(self) -> SingleError<String> {
        SingleError {
            input: self.input.0.iter().collect(),
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

fn io_to_any(err: IoError) -> AnyError {
    if !matches!(err.kind(), IoErrorKind::Other) {
        err.into()
    } else if let Some(e) = err.into_inner() {
        AnyError::from_boxed(e)
    } else {
        IoError::from(IoErrorKind::Other).into()
    }
}

pub fn read_struct(data: impl Read + Seek, format: &[char]) -> AnyResult<VarArray> {
    fn f<const N: usize, T: ToGodot>(
        (data, a): &mut (impl Read, VarArray),
        n: usize,
        f: impl Fn(&[u8; N]) -> T,
    ) -> AnyResult<()> {
        let mut temp = [0; N];
        for _ in 0..n {
            site_context!(data.read_exact(&mut temp).map_err(io_to_any))?;
            a.push(&f(&temp).to_variant());
        }

        Ok(())
    }

    let mut format = CharSlice(format);
    let mut r = (data, Array::new());
    let mut p_ = pair(opt(u32_), parse_datatype);
    while !format.0.is_empty() {
        let (i, (n, t)) = p_
            .parse_complete(format)
            .map_err(|e| e.map(SingleError::into_owned))?;
        format = i;
        let n = n.unwrap_or(1) as usize;

        match t {
            DataType::Padding => site_context!(r.0.seek_relative(n as _).map_err(io_to_any)),
            DataType::SignedByte => f::<1, _>(&mut r, n, |v| v[0] as i8 as i64),
            DataType::UnsignedByte => f::<1, _>(&mut r, n, |v| v[0] as i64),
            DataType::SignedShort => f::<2, _>(&mut r, n, |v| i16::from_le_bytes(*v) as i64),
            DataType::UnsignedShort => f::<2, _>(&mut r, n, |v| u16::from_le_bytes(*v) as i64),
            DataType::SignedInt => f::<4, _>(&mut r, n, |v| i32::from_le_bytes(*v) as i64),
            DataType::UnsignedInt => f::<4, _>(&mut r, n, |v| u32::from_le_bytes(*v) as i64),
            DataType::SignedLong => f::<8, _>(&mut r, n, |v| i64::from_le_bytes(*v)),
            DataType::UnsignedLong => f::<8, _>(&mut r, n, |v| u64::from_le_bytes(*v) as i64),
            DataType::Float => f::<4, _>(&mut r, n, |v| f32::from_le_bytes(*v)),
            DataType::Double => f::<8, _>(&mut r, n, |v| f64::from_le_bytes(*v)),
            DataType::Vector2(VectorSubtype::Float) => {
                f(&mut r, n, <Vector2 as StructPacking<f32>>::read_array)
            }
            DataType::Vector2(VectorSubtype::Double) => {
                f(&mut r, n, <Vector2 as StructPacking<f64>>::read_array)
            }
            DataType::Vector2(VectorSubtype::Int) => {
                f(&mut r, n, <Vector2i as StructPacking<i32>>::read_array)
            }
            DataType::Vector2(VectorSubtype::Long) => {
                f(&mut r, n, <Vector2i as StructPacking<i64>>::read_array)
            }
            DataType::Vector3(VectorSubtype::Float) => {
                f(&mut r, n, <Vector3 as StructPacking<f32>>::read_array)
            }
            DataType::Vector3(VectorSubtype::Double) => {
                f(&mut r, n, <Vector3 as StructPacking<f64>>::read_array)
            }
            DataType::Vector3(VectorSubtype::Int) => {
                f(&mut r, n, <Vector3i as StructPacking<i32>>::read_array)
            }
            DataType::Vector3(VectorSubtype::Long) => {
                f(&mut r, n, <Vector3i as StructPacking<i64>>::read_array)
            }
            DataType::Vector4(VectorSubtype::Float) => {
                f(&mut r, n, <Vector4 as StructPacking<f32>>::read_array)
            }
            DataType::Vector4(VectorSubtype::Double) => {
                f(&mut r, n, <Vector4 as StructPacking<f64>>::read_array)
            }
            DataType::Vector4(VectorSubtype::Int) => {
                f(&mut r, n, <Vector4i as StructPacking<i32>>::read_array)
            }
            DataType::Vector4(VectorSubtype::Long) => {
                f(&mut r, n, <Vector4i as StructPacking<i64>>::read_array)
            }
            DataType::Plane(FloatSubtype::Float) => {
                f(&mut r, n, <Plane as StructPacking<f32>>::read_array)
            }
            DataType::Plane(FloatSubtype::Double) => {
                f(&mut r, n, <Plane as StructPacking<f64>>::read_array)
            }
            DataType::Quaternion(FloatSubtype::Float) => {
                f(&mut r, n, <Quaternion as StructPacking<f32>>::read_array)
            }
            DataType::Quaternion(FloatSubtype::Double) => {
                f(&mut r, n, <Quaternion as StructPacking<f64>>::read_array)
            }
            DataType::Color(ColorSubtype::Float) => {
                f(&mut r, n, <Color as StructPacking<f32>>::read_array)
            }
            DataType::Color(ColorSubtype::Double) => {
                f(&mut r, n, <Color as StructPacking<f64>>::read_array)
            }
            DataType::Color(ColorSubtype::Byte) => {
                f(&mut r, n, <Color as StructPacking<u8>>::read_array)
            }
            DataType::Rect2(VectorSubtype::Float) => {
                f(&mut r, n, <Rect2 as StructPacking<f32>>::read_array)
            }
            DataType::Rect2(VectorSubtype::Double) => {
                f(&mut r, n, <Rect2 as StructPacking<f64>>::read_array)
            }
            DataType::Rect2(VectorSubtype::Int) => {
                f(&mut r, n, <Rect2i as StructPacking<i32>>::read_array)
            }
            DataType::Rect2(VectorSubtype::Long) => {
                f(&mut r, n, <Rect2i as StructPacking<i64>>::read_array)
            }
            DataType::Aabb(FloatSubtype::Float) => {
                f(&mut r, n, <Aabb as StructPacking<f32>>::read_array)
            }
            DataType::Aabb(FloatSubtype::Double) => {
                f(&mut r, n, <Aabb as StructPacking<f64>>::read_array)
            }
            DataType::Basis(FloatSubtype::Float) => {
                f(&mut r, n, <Basis as StructPacking<f32>>::read_array)
            }
            DataType::Basis(FloatSubtype::Double) => {
                f(&mut r, n, <Basis as StructPacking<f64>>::read_array)
            }
            DataType::Projection(FloatSubtype::Float) => {
                f(&mut r, n, <Projection as StructPacking<f32>>::read_array)
            }
            DataType::Projection(FloatSubtype::Double) => {
                f(&mut r, n, <Projection as StructPacking<f64>>::read_array)
            }
            DataType::Transform2D(FloatSubtype::Float) => {
                f(&mut r, n, <Transform2D as StructPacking<f32>>::read_array)
            }
            DataType::Transform2D(FloatSubtype::Double) => {
                f(&mut r, n, <Transform2D as StructPacking<f64>>::read_array)
            }
            DataType::Transform3D(FloatSubtype::Float) => {
                f(&mut r, n, <Transform3D as StructPacking<f32>>::read_array)
            }
            DataType::Transform3D(FloatSubtype::Double) => {
                f(&mut r, n, <Transform3D as StructPacking<f64>>::read_array)
            }
        }?;
    }

    Ok(r.1)
}

pub fn write_struct(data: impl Write + Seek, format: &[char], arr: VarArray) -> AnyResult<usize> {
    fn f<const N: usize, T: FromGodot>(
        (data, total, a): &mut (impl Write, usize, impl Iterator<Item = Variant>),
        n: usize,
        f: impl Fn(&T, &mut [u8; N]),
    ) -> AnyResult<()> {
        let mut temp = [0; N];
        for _ in 0..n {
            let Some(v) = a
                .next()
                .map(|v| site_context!(from_var_any::<T>(v)))
                .transpose()?
            else {
                bail_with_site!("Input array too small")
            };
            f(&v, &mut temp);
            site_context!(data.write_all(&temp).map_err(io_to_any))?;
            *total += N;
        }

        Ok(())
    }

    let mut format = CharSlice(format);
    let mut r = (data, 0, arr.iter_shared());
    let mut p_ = pair(opt(u32_), parse_datatype);
    while !format.0.is_empty() {
        let (i, (n, t)) = p_
            .parse_complete(format)
            .map_err(|e| e.map(SingleError::into_owned))?;
        format = i;
        let n = n.unwrap_or(1) as usize;

        match t {
            DataType::Padding => {
                r.1 += n;
                site_context!(r.0.seek_relative(n as _).map_err(io_to_any))
            }
            DataType::SignedByte => f::<1, i64>(&mut r, n, |d, s| s[0] = *d as i8 as u8),
            DataType::UnsignedByte => f::<1, i64>(&mut r, n, |d, s| s[0] = *d as u8),
            DataType::SignedShort => f::<2, i64>(&mut r, n, |d, s| *s = (*d as i16).to_le_bytes()),
            DataType::UnsignedShort => {
                f::<2, i64>(&mut r, n, |d, s| *s = (*d as u16).to_le_bytes())
            }
            DataType::SignedInt => f::<4, i64>(&mut r, n, |d, s| *s = (*d as i32).to_le_bytes()),
            DataType::UnsignedInt => f::<4, i64>(&mut r, n, |d, s| *s = (*d as u32).to_le_bytes()),
            DataType::SignedLong | DataType::UnsignedLong => {
                f(&mut r, n, |d: &i64, s| *s = d.to_le_bytes())
            }
            DataType::Float => f::<4, f32>(&mut r, n, |d, s| *s = d.to_le_bytes()),
            DataType::Double => f::<8, f64>(&mut r, n, |d, s| *s = d.to_le_bytes()),
            DataType::Vector2(VectorSubtype::Float) => {
                f(&mut r, n, <Vector2 as StructPacking<f32>>::write_array)
            }
            DataType::Vector2(VectorSubtype::Double) => {
                f(&mut r, n, <Vector2 as StructPacking<f64>>::write_array)
            }
            DataType::Vector2(VectorSubtype::Int) => {
                f(&mut r, n, <Vector2i as StructPacking<i32>>::write_array)
            }
            DataType::Vector2(VectorSubtype::Long) => {
                f(&mut r, n, <Vector2i as StructPacking<i64>>::write_array)
            }
            DataType::Vector3(VectorSubtype::Float) => {
                f(&mut r, n, <Vector3 as StructPacking<f32>>::write_array)
            }
            DataType::Vector3(VectorSubtype::Double) => {
                f(&mut r, n, <Vector3 as StructPacking<f64>>::write_array)
            }
            DataType::Vector3(VectorSubtype::Int) => {
                f(&mut r, n, <Vector3i as StructPacking<i32>>::write_array)
            }
            DataType::Vector3(VectorSubtype::Long) => {
                f(&mut r, n, <Vector3i as StructPacking<i64>>::write_array)
            }
            DataType::Vector4(VectorSubtype::Float) => {
                f(&mut r, n, <Vector4 as StructPacking<f32>>::write_array)
            }
            DataType::Vector4(VectorSubtype::Double) => {
                f(&mut r, n, <Vector4 as StructPacking<f64>>::write_array)
            }
            DataType::Vector4(VectorSubtype::Int) => {
                f(&mut r, n, <Vector4i as StructPacking<i32>>::write_array)
            }
            DataType::Vector4(VectorSubtype::Long) => {
                f(&mut r, n, <Vector4i as StructPacking<i64>>::write_array)
            }
            DataType::Plane(FloatSubtype::Float) => {
                f(&mut r, n, <Plane as StructPacking<f32>>::write_array)
            }
            DataType::Plane(FloatSubtype::Double) => {
                f(&mut r, n, <Plane as StructPacking<f64>>::write_array)
            }
            DataType::Quaternion(FloatSubtype::Float) => {
                f(&mut r, n, <Quaternion as StructPacking<f32>>::write_array)
            }
            DataType::Quaternion(FloatSubtype::Double) => {
                f(&mut r, n, <Quaternion as StructPacking<f64>>::write_array)
            }
            DataType::Color(ColorSubtype::Float) => {
                f(&mut r, n, <Color as StructPacking<f32>>::write_array)
            }
            DataType::Color(ColorSubtype::Double) => {
                f(&mut r, n, <Color as StructPacking<f64>>::write_array)
            }
            DataType::Color(ColorSubtype::Byte) => {
                f(&mut r, n, <Color as StructPacking<u8>>::write_array)
            }
            DataType::Rect2(VectorSubtype::Float) => {
                f(&mut r, n, <Rect2 as StructPacking<f32>>::write_array)
            }
            DataType::Rect2(VectorSubtype::Double) => {
                f(&mut r, n, <Rect2 as StructPacking<f64>>::write_array)
            }
            DataType::Rect2(VectorSubtype::Int) => {
                f(&mut r, n, <Rect2i as StructPacking<i32>>::write_array)
            }
            DataType::Rect2(VectorSubtype::Long) => {
                f(&mut r, n, <Rect2i as StructPacking<i64>>::write_array)
            }
            DataType::Aabb(FloatSubtype::Float) => {
                f(&mut r, n, <Aabb as StructPacking<f32>>::write_array)
            }
            DataType::Aabb(FloatSubtype::Double) => {
                f(&mut r, n, <Aabb as StructPacking<f64>>::write_array)
            }
            DataType::Basis(FloatSubtype::Float) => {
                f(&mut r, n, <Basis as StructPacking<f32>>::write_array)
            }
            DataType::Basis(FloatSubtype::Double) => {
                f(&mut r, n, <Basis as StructPacking<f64>>::write_array)
            }
            DataType::Projection(FloatSubtype::Float) => {
                f(&mut r, n, <Projection as StructPacking<f32>>::write_array)
            }
            DataType::Projection(FloatSubtype::Double) => {
                f(&mut r, n, <Projection as StructPacking<f64>>::write_array)
            }
            DataType::Transform2D(FloatSubtype::Float) => {
                f(&mut r, n, <Transform2D as StructPacking<f32>>::write_array)
            }
            DataType::Transform2D(FloatSubtype::Double) => {
                f(&mut r, n, <Transform2D as StructPacking<f64>>::write_array)
            }
            DataType::Transform3D(FloatSubtype::Float) => {
                f(&mut r, n, <Transform3D as StructPacking<f32>>::write_array)
            }
            DataType::Transform3D(FloatSubtype::Double) => {
                f(&mut r, n, <Transform3D as StructPacking<f64>>::write_array)
            }
        }?;
    }

    Ok(r.1)
}
