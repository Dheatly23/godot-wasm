use std::borrow::{Borrow, Cow};
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use anyhow::Result as AnyResult;
use godot::builtin::meta::GodotConvert;
use godot::engine::global::Error as GError;
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
