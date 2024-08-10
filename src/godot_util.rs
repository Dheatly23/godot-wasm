use std::borrow::{Borrow, Cow};
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use anyhow::Result as AnyResult;
use godot::global::Error as GError;
use godot::prelude::*;
use godot::register::property::PropertyHintInfo;

/// WARNING: Incredibly unsafe.
/// It's just used as workaround to pass Godot objects across closure.
/// (At least until it supports multi-threading)
#[derive(Clone)]
#[repr(transparent)]
pub(crate) struct SendSyncWrapper<T: ?Sized>(T);

unsafe impl<T: ?Sized> Send for SendSyncWrapper<T> {}
unsafe impl<T: ?Sized> Sync for SendSyncWrapper<T> {}

#[allow(dead_code)]
impl<T> SendSyncWrapper<T> {
    pub(crate) fn new(v: T) -> Self {
        Self(v)
    }

    pub(crate) fn into_inner(self) -> T {
        self.0
    }
}

impl<T: ?Sized> Deref for SendSyncWrapper<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for SendSyncWrapper<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

pub fn from_var_any<T: FromGodot>(v: impl Borrow<Variant>) -> AnyResult<T> {
    v.borrow().try_to::<T>().map_err(|e| e.into_erased().into())
}

#[allow(dead_code)]
pub fn gstring_from_maybe_utf8(buf: &[u8]) -> GString {
    match String::from_utf8_lossy(buf) {
        Cow::Owned(v) => GString::from(v),
        Cow::Borrowed(v) => GString::from(v),
    }
}

pub fn option_to_variant<T: ToGodot>(t: Option<T>) -> Variant {
    match t {
        Some(v) => v.to_variant(),
        None => Variant::nil(),
    }
}

pub fn variant_to_option<T: FromGodot>(v: Variant) -> AnyResult<Option<T>> {
    if v.is_nil() {
        Ok(None)
    } else {
        match v.try_to() {
            Ok(v) => Ok(Some(v)),
            Err(e) => Err(e.into_erased().into()),
        }
    }
}

pub struct PhantomProperty<T>(PhantomData<T>);

impl<T: Default> Default for PhantomProperty<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> GodotConvert for PhantomProperty<T>
where
    T: GodotConvert,
    T::Via: Default,
{
    type Via = T::Via;
}

impl<T> Var for PhantomProperty<T>
where
    T: Var,
    T::Via: Default,
{
    fn get_property(&self) -> Self::Via {
        Self::Via::default()
    }

    fn set_property(&mut self, _: Self::Via) {}

    fn property_hint() -> PropertyHintInfo {
        T::property_hint()
    }
}

#[macro_export]
macro_rules! variant_dispatch {
    ($v:ident { $($t:tt => $e:expr),* $(,)? }) => {
        match $v.get_type() {
            $($crate::variant_dispatch!(#p $t) => $crate::variant_dispatch!(#el $v $t $e)),*
        }
    };
    (#p _) => { _ };
    (#p $t:ident) => { godot::builtin::VariantType::$t };
    (#el $v:ident _ $e:expr) => { $e };
    (#el $v:ident NIL $e:expr) => { $e };
    (#el $v:ident BOOL $e:expr) => {{ let $v: bool = $v.to(); $e }};
    (#el $v:ident INT $e:expr) => {{ let $v: i64 = $v.to(); $e }};
    (#el $v:ident FLOAT $e:expr) => {{ let $v: f64 = $v.to(); $e }};
    (#el $v:ident STRING $e:expr) => {{ let $v: godot::builtin::GString = $v.to(); $e }};
    (#el $v:ident VECTOR2 $e:expr) => {{ let $v: godot::builtin::Vector2 = $v.to(); $e }};
    (#el $v:ident VECTOR2I $e:expr) => {{ let $v: godot::builtin::Vector2i = $v.to(); $e }};
    (#el $v:ident RECT2 $e:expr) => {{ let $v: godot::builtin::Rect2 = $v.to(); $e }};
    (#el $v:ident RECT2I $e:expr) => {{ let $v: godot::builtin::Rect2i = $v.to(); $e }};
    (#el $v:ident VECTOR3 $e:expr) => {{ let $v: godot::builtin::Vector3 = $v.to(); $e }};
    (#el $v:ident VECTOR3I $e:expr) => {{ let $v: godot::builtin::Vector3i = $v.to(); $e }};
    (#el $v:ident TRANSFORM2D $e:expr) => {{ let $v: godot::builtin::Transform2D = $v.to(); $e }};
    (#el $v:ident VECTOR4 $e:expr) => {{ let $v: godot::builtin::Vector4 = $v.to(); $e }};
    (#el $v:ident VECTOR4I $e:expr) => {{ let $v: godot::builtin::Vector4i = $v.to(); $e }};
    (#el $v:ident PLANE $e:expr) => {{ let $v: godot::builtin::Plane = $v.to(); $e }};
    (#el $v:ident QUATERNION $e:expr) => {{ let $v: godot::builtin::Quaternion = $v.to(); $e }};
    (#el $v:ident AABB $e:expr) => {{ let $v: godot::builtin::Aabb = $v.to(); $e }};
    (#el $v:ident BASIS $e:expr) => {{ let $v: godot::builtin::Basis = $v.to(); $e }};
    (#el $v:ident TRANSFORM3D $e:expr) => {{ let $v: godot::builtin::Transform3D = $v.to(); $e }};
    (#el $v:ident PROJECTION $e:expr) => {{ let $v: godot::builtin::Projection = $v.to(); $e }};
    (#el $v:ident COLOR $e:expr) => {{ let $v: godot::builtin::Color = $v.to(); $e }};
    (#el $v:ident STRING_NAME $e:expr) => {{ let $v: godot::builtin::StringName = $v.to(); $e }};
    (#el $v:ident NODE_PATH $e:expr) => {{ let $v: godot::builtin::NodePath = $v.to(); $e }};
    (#el $v:ident RID $e:expr) => {{ let $v: godot::builtin::Rid = $v.to(); $e }};
    (#el $v:ident OBJECT $e:expr) => {{ let $v: godot::obj::Gd<godot::classes::Object> = $v.to(); $e }};
    (#el $v:ident CALLABLE $e:expr) => {{ let $v: godot::builtin::Callable = $v.to(); $e }};
    (#el $v:ident SIGNAL $e:expr) => {{ let $v: godot::builtin::Signal = $v.to(); $e }};
    (#el $v:ident DICTIONARY $e:expr) => {{ let $v: godot::builtin::Dictionary = $v.to(); $e }};
    (#el $v:ident ARRAY $e:expr) => {{ let $v: godot::builtin::VariantArray = $v.to(); $e }};
    (#el $v:ident PACKED_BYTE_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedByteArray = $v.to(); $e }};
    (#el $v:ident PACKED_INT32_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedInt32Array = $v.to(); $e }};
    (#el $v:ident PACKED_INT64_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedInt64Array = $v.to(); $e }};
    (#el $v:ident PACKED_FLOAT32_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedFloat32Array = $v.to(); $e }};
    (#el $v:ident PACKED_FLOAT64_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedFloat64Array = $v.to(); $e }};
    (#el $v:ident PACKED_STRING_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedStringArray = $v.to(); $e }};
    (#el $v:ident PACKED_VECTOR2_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedVector2Array = $v.to(); $e }};
    (#el $v:ident PACKED_VECTOR3_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedVector3Array = $v.to(); $e }};
    (#el $v:ident PACKED_COLOR_ARRAY $e:expr) => {{ let $v: godot::builtin::PackedColorArray = $v.to(); $e }};
}

// Keep until gdext implement this
#[allow(dead_code)]
#[derive(Clone)]
pub enum VariantDispatch {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(GString),
    Vector2(Vector2),
    Vector2i(Vector2i),
    Rect2(Rect2),
    Rect2i(Rect2i),
    Vector3(Vector3),
    Vector3i(Vector3i),
    Transform2D(Transform2D),
    Vector4(Vector4),
    Vector4i(Vector4i),
    Plane(Plane),
    Quaternion(Quaternion),
    Aabb(Aabb),
    Basis(Basis),
    Transform3D(Transform3D),
    Projection(Projection),
    Color(Color),
    StringName(StringName),
    NodePath(NodePath),
    Rid(Rid),
    Object(Gd<Object>),
    Callable(Callable),
    Signal(Signal),
    Dictionary(Dictionary),
    Array(VariantArray),
    PackedByteArray(PackedByteArray),
    PackedInt32Array(PackedInt32Array),
    PackedInt64Array(PackedInt64Array),
    PackedFloat32Array(PackedFloat32Array),
    PackedFloat64Array(PackedFloat64Array),
    PackedStringArray(PackedStringArray),
    PackedVector2Array(PackedVector2Array),
    PackedVector3Array(PackedVector3Array),
    PackedColorArray(PackedColorArray),
}

impl From<&'_ Variant> for VariantDispatch {
    fn from(var: &Variant) -> Self {
        match var.get_type() {
            VariantType::NIL => Self::Nil,
            VariantType::BOOL => Self::Bool(var.to()),
            VariantType::INT => Self::Int(var.to()),
            VariantType::FLOAT => Self::Float(var.to()),
            VariantType::STRING => Self::String(var.to()),
            VariantType::VECTOR2 => Self::Vector2(var.to()),
            VariantType::VECTOR2I => Self::Vector2i(var.to()),
            VariantType::RECT2 => Self::Rect2(var.to()),
            VariantType::RECT2I => Self::Rect2i(var.to()),
            VariantType::VECTOR3 => Self::Vector3(var.to()),
            VariantType::VECTOR3I => Self::Vector3i(var.to()),
            VariantType::TRANSFORM2D => Self::Transform2D(var.to()),
            VariantType::VECTOR4 => Self::Vector4(var.to()),
            VariantType::VECTOR4I => Self::Vector4i(var.to()),
            VariantType::PLANE => Self::Plane(var.to()),
            VariantType::QUATERNION => Self::Quaternion(var.to()),
            VariantType::AABB => Self::Aabb(var.to()),
            VariantType::BASIS => Self::Basis(var.to()),
            VariantType::TRANSFORM3D => Self::Transform3D(var.to()),
            VariantType::PROJECTION => Self::Projection(var.to()),
            VariantType::COLOR => Self::Color(var.to()),
            VariantType::STRING_NAME => Self::StringName(var.to()),
            VariantType::NODE_PATH => Self::NodePath(var.to()),
            VariantType::RID => Self::Rid(var.to()),
            VariantType::OBJECT => Self::Object(var.to()),
            VariantType::CALLABLE => Self::Callable(var.to()),
            VariantType::SIGNAL => Self::Signal(var.to()),
            VariantType::DICTIONARY => Self::Dictionary(var.to()),
            VariantType::ARRAY => Self::Array(var.to()),
            VariantType::PACKED_BYTE_ARRAY => Self::PackedByteArray(var.to()),
            VariantType::PACKED_INT32_ARRAY => Self::PackedInt32Array(var.to()),
            VariantType::PACKED_INT64_ARRAY => Self::PackedInt64Array(var.to()),
            VariantType::PACKED_FLOAT32_ARRAY => Self::PackedFloat32Array(var.to()),
            VariantType::PACKED_FLOAT64_ARRAY => Self::PackedFloat64Array(var.to()),
            VariantType::PACKED_STRING_ARRAY => Self::PackedStringArray(var.to()),
            VariantType::PACKED_VECTOR2_ARRAY => Self::PackedVector2Array(var.to()),
            VariantType::PACKED_VECTOR3_ARRAY => Self::PackedVector3Array(var.to()),
            VariantType::PACKED_COLOR_ARRAY => Self::PackedColorArray(var.to()),
            v => panic!("Unknown variant type {v:?}"),
        }
    }
}

/// Helper trait for common PackedArray operations.
pub trait PackedArrayLike: Default {
    type Elem;

    fn resize(&mut self, size: usize);
    fn as_mut_slice(&mut self) -> &mut [Self::Elem];
}

macro_rules! impl_packed_array {
    ($($t:ty : $el:ty),* $(,)?) => {$(
        impl PackedArrayLike for $t {
            type Elem = $el;

            fn resize(&mut self, size: usize) {
                self.resize(size);
            }

            fn as_mut_slice(&mut self) -> &mut [$el] {
                self.as_mut_slice()
            }
        }
    )*};
}

impl_packed_array! {
    PackedByteArray : u8,
    PackedInt32Array : i32,
    PackedInt64Array : i64,
    PackedFloat32Array : f32,
    PackedFloat64Array : f64,
    PackedVector2Array : Vector2,
    PackedVector3Array : Vector3,
    PackedColorArray : Color,
}

pub struct ErrorWrapper {
    error: GError,
    msg: Option<String>,
}

impl fmt::Debug for ErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            if let Some(m) = &self.msg {
                writeln!(f, "godot error {m}:")
            } else {
                writeln!(f, "godot error:")
            }?;
            write!(f, "{:#?}", self.error)
        } else {
            if let Some(m) = &self.msg {
                write!(f, "godot error {m}:")
            } else {
                write!(f, "godot error: ")
            }?;
            write!(f, "{:?}", self.error)
        }
    }
}

impl fmt::Display for ErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            if let Some(m) = &self.msg {
                writeln!(f, "godot error {m}:")
            } else {
                writeln!(f, "godot error:")
            }?;
            write!(f, "{:#?}", self.error)
        } else {
            if let Some(m) = &self.msg {
                write!(f, "godot error {m}:")
            } else {
                write!(f, "godot error: ")
            }?;
            write!(f, "{:?}", self.error)
        }
    }
}

impl Error for ErrorWrapper {}

impl From<GError> for ErrorWrapper {
    fn from(error: GError) -> Self {
        Self { error, msg: None }
    }
}

impl ErrorWrapper {
    #[allow(dead_code)]
    pub fn new(error: GError, msg: String) -> Self {
        Self {
            error,
            msg: Some(msg),
        }
    }
}

/// Helper trait for byte array packing.
pub trait StructPacking<ValType> {
    type Arr;

    fn read_array(arr: &Self::Arr) -> Self;
    fn write_array(&self, arr: &mut Self::Arr);
}

impl StructPacking<f32> for Vector2 {
    type Arr = [u8; 4 * 2];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f32(f32::from_le_bytes(a[..4].try_into().unwrap())),
            y: real::from_f32(f32::from_le_bytes(a[4..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [self.x.as_f32().to_le_bytes(), self.y.as_f32().to_le_bytes()].as_flattened(),
        );
    }
}

impl StructPacking<f64> for Vector2 {
    type Arr = [u8; 8 * 2];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f64(f64::from_le_bytes(a[..8].try_into().unwrap())),
            y: real::from_f64(f64::from_le_bytes(a[8..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [self.x.as_f64().to_le_bytes(), self.y.as_f64().to_le_bytes()].as_flattened(),
        );
    }
}

impl StructPacking<i32> for Vector2i {
    type Arr = [u8; 4 * 2];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: i32::from_le_bytes(a[..4].try_into().unwrap()),
            y: i32::from_le_bytes(a[4..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice([self.x.to_le_bytes(), self.y.to_le_bytes()].as_flattened());
    }
}

impl StructPacking<i64> for Vector2i {
    type Arr = [u8; 8 * 2];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: i64::from_le_bytes(a[..8].try_into().unwrap()) as _,
            y: i64::from_le_bytes(a[8..].try_into().unwrap()) as _,
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [(self.x as i64).to_le_bytes(), (self.y as i64).to_le_bytes()].as_flattened(),
        );
    }
}

impl StructPacking<f32> for Vector3 {
    type Arr = [u8; 4 * 3];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f32(f32::from_le_bytes(a[..4].try_into().unwrap())),
            y: real::from_f32(f32::from_le_bytes(a[4..8].try_into().unwrap())),
            z: real::from_f32(f32::from_le_bytes(a[8..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.as_f32().to_le_bytes(),
                self.y.as_f32().to_le_bytes(),
                self.z.as_f32().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<f64> for Vector3 {
    type Arr = [u8; 8 * 3];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f64(f64::from_le_bytes(a[..8].try_into().unwrap())),
            y: real::from_f64(f64::from_le_bytes(a[8..16].try_into().unwrap())),
            z: real::from_f64(f64::from_le_bytes(a[16..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.as_f64().to_le_bytes(),
                self.y.as_f64().to_le_bytes(),
                self.z.as_f64().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<i32> for Vector3i {
    type Arr = [u8; 4 * 3];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: i32::from_le_bytes(a[..4].try_into().unwrap()),
            y: i32::from_le_bytes(a[4..8].try_into().unwrap()),
            z: i32::from_le_bytes(a[8..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.to_le_bytes(),
                self.y.to_le_bytes(),
                self.z.to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<i64> for Vector3i {
    type Arr = [u8; 8 * 3];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: i64::from_le_bytes(a[..8].try_into().unwrap()) as _,
            y: i64::from_le_bytes(a[8..16].try_into().unwrap()) as _,
            z: i64::from_le_bytes(a[16..].try_into().unwrap()) as _,
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                (self.x as i64).to_le_bytes(),
                (self.y as i64).to_le_bytes(),
                (self.z as i64).to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<f32> for Vector4 {
    type Arr = [u8; 4 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f32(f32::from_le_bytes(a[..4].try_into().unwrap())),
            y: real::from_f32(f32::from_le_bytes(a[4..8].try_into().unwrap())),
            z: real::from_f32(f32::from_le_bytes(a[8..12].try_into().unwrap())),
            w: real::from_f32(f32::from_le_bytes(a[12..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.as_f32().to_le_bytes(),
                self.y.as_f32().to_le_bytes(),
                self.z.as_f32().to_le_bytes(),
                self.w.as_f32().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<f64> for Vector4 {
    type Arr = [u8; 8 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f64(f64::from_le_bytes(a[..8].try_into().unwrap())),
            y: real::from_f64(f64::from_le_bytes(a[8..16].try_into().unwrap())),
            z: real::from_f64(f64::from_le_bytes(a[16..24].try_into().unwrap())),
            w: real::from_f64(f64::from_le_bytes(a[24..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.as_f64().to_le_bytes(),
                self.y.as_f64().to_le_bytes(),
                self.z.as_f64().to_le_bytes(),
                self.w.as_f64().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<i32> for Vector4i {
    type Arr = [u8; 4 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: i32::from_le_bytes(a[..4].try_into().unwrap()),
            y: i32::from_le_bytes(a[4..8].try_into().unwrap()),
            z: i32::from_le_bytes(a[8..12].try_into().unwrap()),
            w: i32::from_le_bytes(a[12..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.to_le_bytes(),
                self.y.to_le_bytes(),
                self.z.to_le_bytes(),
                self.w.to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<i64> for Vector4i {
    type Arr = [u8; 8 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: i64::from_le_bytes(a[..8].try_into().unwrap()) as _,
            y: i64::from_le_bytes(a[8..16].try_into().unwrap()) as _,
            z: i64::from_le_bytes(a[16..24].try_into().unwrap()) as _,
            w: i64::from_le_bytes(a[24..].try_into().unwrap()) as _,
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                (self.x as i64).to_le_bytes(),
                (self.y as i64).to_le_bytes(),
                (self.z as i64).to_le_bytes(),
                (self.w as i64).to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<f32> for Plane {
    type Arr = [u8; 4 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            normal: <_ as StructPacking<f32>>::read_array(a[..12].try_into().unwrap()),
            d: real::from_f32(f32::from_le_bytes(a[12..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.normal, (&mut a[..12]).try_into().unwrap());
        *<&mut _>::try_from(&mut a[12..]).unwrap() = self.d.as_f32().to_le_bytes();
    }
}

impl StructPacking<f64> for Plane {
    type Arr = [u8; 8 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            normal: <_ as StructPacking<f64>>::read_array(a[..24].try_into().unwrap()),
            d: real::from_f64(f64::from_le_bytes(a[24..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f64>>::write_array(&self.normal, (&mut a[..24]).try_into().unwrap());
        *<&mut _>::try_from(&mut a[24..]).unwrap() = self.d.as_f64().to_le_bytes();
    }
}

impl StructPacking<f32> for Quaternion {
    type Arr = [u8; 4 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f32(f32::from_le_bytes(a[..4].try_into().unwrap())),
            y: real::from_f32(f32::from_le_bytes(a[4..8].try_into().unwrap())),
            z: real::from_f32(f32::from_le_bytes(a[8..12].try_into().unwrap())),
            w: real::from_f32(f32::from_le_bytes(a[12..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.as_f32().to_le_bytes(),
                self.y.as_f32().to_le_bytes(),
                self.z.as_f32().to_le_bytes(),
                self.w.as_f32().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<f64> for Quaternion {
    type Arr = [u8; 8 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            x: real::from_f64(f64::from_le_bytes(a[..8].try_into().unwrap())),
            y: real::from_f64(f64::from_le_bytes(a[8..16].try_into().unwrap())),
            z: real::from_f64(f64::from_le_bytes(a[16..24].try_into().unwrap())),
            w: real::from_f64(f64::from_le_bytes(a[24..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.x.as_f64().to_le_bytes(),
                self.y.as_f64().to_le_bytes(),
                self.z.as_f64().to_le_bytes(),
                self.w.as_f64().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<f32> for Color {
    type Arr = [u8; 4 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            r: real::from_f32(f32::from_le_bytes(a[..4].try_into().unwrap())),
            g: real::from_f32(f32::from_le_bytes(a[4..8].try_into().unwrap())),
            b: real::from_f32(f32::from_le_bytes(a[8..12].try_into().unwrap())),
            a: real::from_f32(f32::from_le_bytes(a[12..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.r.as_f32().to_le_bytes(),
                self.g.as_f32().to_le_bytes(),
                self.b.as_f32().to_le_bytes(),
                self.a.as_f32().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<f64> for Color {
    type Arr = [u8; 8 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            r: real::from_f64(f64::from_le_bytes(a[..8].try_into().unwrap())),
            g: real::from_f64(f64::from_le_bytes(a[8..16].try_into().unwrap())),
            b: real::from_f64(f64::from_le_bytes(a[16..24].try_into().unwrap())),
            a: real::from_f64(f64::from_le_bytes(a[24..].try_into().unwrap())),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        a.copy_from_slice(
            [
                self.r.as_f64().to_le_bytes(),
                self.g.as_f64().to_le_bytes(),
                self.b.as_f64().to_le_bytes(),
                self.a.as_f64().to_le_bytes(),
            ]
            .as_flattened(),
        );
    }
}

impl StructPacking<u8> for Color {
    type Arr = [u8; 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self::from_rgba8(a[0], a[1], a[2], a[3])
    }

    fn write_array(&self, a: &mut Self::Arr) {
        *a = [
            (self.r * 255.).round() as _,
            (self.g * 255.).round() as _,
            (self.b * 255.).round() as _,
            (self.a * 255.).round() as _,
        ];
    }
}

impl StructPacking<f32> for Rect2 {
    type Arr = [u8; 4 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            position: <_ as StructPacking<f32>>::read_array(a[..8].try_into().unwrap()),
            size: <_ as StructPacking<f32>>::read_array(a[8..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.position, (&mut a[..8]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.size, (&mut a[8..]).try_into().unwrap());
    }
}

impl StructPacking<f64> for Rect2 {
    type Arr = [u8; 8 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            position: <_ as StructPacking<f64>>::read_array(a[..16].try_into().unwrap()),
            size: <_ as StructPacking<f64>>::read_array(a[16..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f64>>::write_array(&self.position, (&mut a[..16]).try_into().unwrap());
        <_ as StructPacking<f64>>::write_array(&self.size, (&mut a[16..]).try_into().unwrap());
    }
}

impl StructPacking<i32> for Rect2i {
    type Arr = [u8; 4 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            position: <_ as StructPacking<i32>>::read_array(a[..8].try_into().unwrap()),
            size: <_ as StructPacking<i32>>::read_array(a[8..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<i32>>::write_array(&self.position, (&mut a[..8]).try_into().unwrap());
        <_ as StructPacking<i32>>::write_array(&self.size, (&mut a[8..]).try_into().unwrap());
    }
}

impl StructPacking<i64> for Rect2i {
    type Arr = [u8; 8 * 4];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            position: <_ as StructPacking<i64>>::read_array(a[..16].try_into().unwrap()),
            size: <_ as StructPacking<i64>>::read_array(a[16..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<i64>>::write_array(&self.position, (&mut a[..16]).try_into().unwrap());
        <_ as StructPacking<i64>>::write_array(&self.size, (&mut a[16..]).try_into().unwrap());
    }
}

impl StructPacking<f32> for Aabb {
    type Arr = [u8; 4 * 6];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            position: <_ as StructPacking<f32>>::read_array(a[..12].try_into().unwrap()),
            size: <_ as StructPacking<f32>>::read_array(a[12..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.position, (&mut a[..12]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.size, (&mut a[12..]).try_into().unwrap());
    }
}

impl StructPacking<f64> for Aabb {
    type Arr = [u8; 8 * 6];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            position: <_ as StructPacking<f64>>::read_array(a[..24].try_into().unwrap()),
            size: <_ as StructPacking<f64>>::read_array(a[24..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f64>>::write_array(&self.position, (&mut a[..24]).try_into().unwrap());
        <_ as StructPacking<f64>>::write_array(&self.size, (&mut a[24..]).try_into().unwrap());
    }
}

impl StructPacking<f32> for Basis {
    type Arr = [u8; 4 * 9];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            rows: [
                <_ as StructPacking<f32>>::read_array(a[..12].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[12..24].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[24..].try_into().unwrap()),
            ],
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.rows[0], (&mut a[..12]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.rows[1], (&mut a[12..24]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.rows[2], (&mut a[24..]).try_into().unwrap());
    }
}

impl StructPacking<f64> for Basis {
    type Arr = [u8; 8 * 9];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            rows: [
                <_ as StructPacking<f32>>::read_array(a[..24].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[24..48].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[48..].try_into().unwrap()),
            ],
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.rows[0], (&mut a[..24]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.rows[1], (&mut a[24..48]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.rows[2], (&mut a[48..]).try_into().unwrap());
    }
}

impl StructPacking<f32> for Projection {
    type Arr = [u8; 4 * 16];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            cols: [
                <_ as StructPacking<f32>>::read_array(a[..16].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[16..32].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[32..48].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[48..].try_into().unwrap()),
            ],
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.cols[0], (&mut a[..16]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.cols[1], (&mut a[16..32]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.cols[2], (&mut a[32..48]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.cols[3], (&mut a[48..]).try_into().unwrap());
    }
}

impl StructPacking<f64> for Projection {
    type Arr = [u8; 8 * 16];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            cols: [
                <_ as StructPacking<f32>>::read_array(a[..32].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[32..64].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[64..96].try_into().unwrap()),
                <_ as StructPacking<f32>>::read_array(a[96..].try_into().unwrap()),
            ],
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.cols[0], (&mut a[..32]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.cols[1], (&mut a[32..64]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.cols[2], (&mut a[64..96]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.cols[3], (&mut a[96..]).try_into().unwrap());
    }
}

impl StructPacking<f32> for Transform2D {
    type Arr = [u8; 4 * 6];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            a: <_ as StructPacking<f32>>::read_array(a[..8].try_into().unwrap()),
            b: <_ as StructPacking<f32>>::read_array(a[8..16].try_into().unwrap()),
            origin: <_ as StructPacking<f32>>::read_array(a[16..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.a, (&mut a[..8]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.b, (&mut a[8..16]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.origin, (&mut a[16..]).try_into().unwrap());
    }
}

impl StructPacking<f64> for Transform2D {
    type Arr = [u8; 8 * 6];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            a: <_ as StructPacking<f64>>::read_array(a[..16].try_into().unwrap()),
            b: <_ as StructPacking<f64>>::read_array(a[16..32].try_into().unwrap()),
            origin: <_ as StructPacking<f64>>::read_array(a[32..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f64>>::write_array(&self.a, (&mut a[..16]).try_into().unwrap());
        <_ as StructPacking<f64>>::write_array(&self.b, (&mut a[16..32]).try_into().unwrap());
        <_ as StructPacking<f64>>::write_array(&self.origin, (&mut a[32..]).try_into().unwrap());
    }
}

impl StructPacking<f32> for Transform3D {
    type Arr = [u8; 4 * 12];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            basis: <_ as StructPacking<f32>>::read_array(a[..36].try_into().unwrap()),
            origin: <_ as StructPacking<f32>>::read_array(a[36..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f32>>::write_array(&self.basis, (&mut a[..36]).try_into().unwrap());
        <_ as StructPacking<f32>>::write_array(&self.origin, (&mut a[36..]).try_into().unwrap());
    }
}

impl StructPacking<f64> for Transform3D {
    type Arr = [u8; 8 * 12];

    fn read_array(a: &Self::Arr) -> Self {
        Self {
            basis: <_ as StructPacking<f64>>::read_array(a[..72].try_into().unwrap()),
            origin: <_ as StructPacking<f64>>::read_array(a[72..].try_into().unwrap()),
        }
    }

    fn write_array(&self, a: &mut Self::Arr) {
        <_ as StructPacking<f64>>::write_array(&self.basis, (&mut a[..72]).try_into().unwrap());
        <_ as StructPacking<f64>>::write_array(&self.origin, (&mut a[72..]).try_into().unwrap());
    }
}
