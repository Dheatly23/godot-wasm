use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use anyhow::Result as AnyResult;
use godot::builtin::meta::GodotConvert;
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

pub fn from_var_any<T: FromGodot>(v: &Variant) -> AnyResult<T> {
    v.try_to::<T>().map_err(|e| e.into_erased().into())
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
        v.try_to().map(Some).map_err(|e| e.into_erased().into())
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
    Array(Array<Variant>),
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
            VariantType::Nil => Self::Nil,
            VariantType::Bool => Self::Bool(var.to()),
            VariantType::Int => Self::Int(var.to()),
            VariantType::Float => Self::Float(var.to()),
            VariantType::String => Self::String(var.to()),
            VariantType::Vector2 => Self::Vector2(var.to()),
            VariantType::Vector2i => Self::Vector2i(var.to()),
            VariantType::Rect2 => Self::Rect2(var.to()),
            VariantType::Rect2i => Self::Rect2i(var.to()),
            VariantType::Vector3 => Self::Vector3(var.to()),
            VariantType::Vector3i => Self::Vector3i(var.to()),
            VariantType::Transform2D => Self::Transform2D(var.to()),
            VariantType::Vector4 => Self::Vector4(var.to()),
            VariantType::Vector4i => Self::Vector4i(var.to()),
            VariantType::Plane => Self::Plane(var.to()),
            VariantType::Quaternion => Self::Quaternion(var.to()),
            VariantType::Aabb => Self::Aabb(var.to()),
            VariantType::Basis => Self::Basis(var.to()),
            VariantType::Transform3D => Self::Transform3D(var.to()),
            VariantType::Projection => Self::Projection(var.to()),
            VariantType::Color => Self::Color(var.to()),
            VariantType::StringName => Self::StringName(var.to()),
            VariantType::NodePath => Self::NodePath(var.to()),
            VariantType::Rid => Self::Rid(var.to()),
            VariantType::Object => Self::Object(var.to()),
            VariantType::Callable => Self::Callable(var.to()),
            VariantType::Signal => Self::Signal(var.to()),
            VariantType::Dictionary => Self::Dictionary(var.to()),
            VariantType::Array => Self::Array(var.to()),
            VariantType::PackedByteArray => Self::PackedByteArray(var.to()),
            VariantType::PackedInt32Array => Self::PackedInt32Array(var.to()),
            VariantType::PackedInt64Array => Self::PackedInt64Array(var.to()),
            VariantType::PackedFloat32Array => Self::PackedFloat32Array(var.to()),
            VariantType::PackedFloat64Array => Self::PackedFloat64Array(var.to()),
            VariantType::PackedStringArray => Self::PackedStringArray(var.to()),
            VariantType::PackedVector2Array => Self::PackedVector2Array(var.to()),
            VariantType::PackedVector3Array => Self::PackedVector3Array(var.to()),
            VariantType::PackedColorArray => Self::PackedColorArray(var.to()),
        }
    }
}
