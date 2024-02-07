#[cfg(not(feature = "new-host-import"))]
use std::collections::HashMap;
#[cfg(not(feature = "new-host-import"))]
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(feature = "object-registry-extern")]
use std::ptr;
#[cfg(feature = "epoch-timeout")]
use std::time;

use anyhow::{anyhow, Error};
use cfg_if::cfg_if;
use godot::builtin::meta::{ConvertError, GodotConvert};
use godot::engine::WeakRef;
use godot::prelude::*;
use godot::register::property::PropertyHintInfo;
use once_cell::sync::Lazy;
#[cfg(feature = "object-registry-extern")]
use wasmtime::ExternRef;
#[cfg(feature = "new-host-import")]
use wasmtime::Linker;
#[cfg(feature = "epoch-timeout")]
use wasmtime::UpdateDeadline;
use wasmtime::{AsContextMut, Caller, Extern, Func, FuncType, Store, ValRaw, ValType};

#[cfg(feature = "epoch-timeout")]
use crate::wasm_config::Config;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_engine::{ENGINE, EPOCH};
#[cfg(feature = "object-registry-extern")]
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;

#[cfg(all(feature = "epoch-timeout", not(feature = "more-precise-timer")))]
pub const EPOCH_MULTIPLIER: u64 = 1000;
#[cfg(all(feature = "epoch-timeout", feature = "more-precise-timer"))]
pub const EPOCH_MULTIPLIER: u64 = 50;
#[cfg(feature = "epoch-timeout")]
pub const EPOCH_DEADLINE: u64 = 5u64.saturating_mul(EPOCH_MULTIPLIER);
#[cfg(feature = "epoch-timeout")]
pub const EPOCH_INTERVAL: time::Duration = time::Duration::from_millis(1000 / EPOCH_MULTIPLIER);

#[cfg(feature = "wasi")]
pub const FILE_NOTEXIST: u32 = 0;
#[cfg(feature = "wasi")]
pub const FILE_FILE: u32 = 1;
#[cfg(feature = "wasi")]
pub const FILE_DIR: u32 = 2;
#[cfg(feature = "wasi")]
pub const FILE_LINK: u32 = 3;

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;
#[cfg(feature = "object-registry-extern")]
pub const TYPE_VARIANT: u32 = 6;

#[cfg(not(feature = "new-host-import"))]
pub const HOST_MODULE: &str = "host";
#[cfg(feature = "object-registry-compat")]
pub const OBJREGISTRY_MODULE: &str = "godot_object_v1";
#[cfg(feature = "object-registry-extern")]
pub const EXTERNREF_MODULE: &str = "godot_object_v2";

pub const MODULE_INCLUDES: &[&str] = &[
    #[cfg(not(feature = "new-host-import"))]
    HOST_MODULE,
    #[cfg(feature = "object-registry-compat")]
    OBJREGISTRY_MODULE,
    #[cfg(feature = "object-registry-extern")]
    EXTERNREF_MODULE,
    #[cfg(feature = "wasi")]
    "wasi_unstable",
    #[cfg(feature = "wasi")]
    "wasi_snapshot_preview0",
    #[cfg(feature = "wasi")]
    "wasi_snapshot_preview1",
];

pub const MEMORY_EXPORT: &str = "memory";

#[macro_export]
macro_rules! bail_with_site {
    ($($t:tt)*) => {
        /*
        return Err(anyhow::anyhow!($($t)*).context(gdnative::log::godot_site!()))
        */
        return Err(anyhow::anyhow!($($t)*))
    };
}

#[macro_export]
macro_rules! site_context {
    ($e:expr) => {
        /*
        $e.map_err(|e| {
            $crate::wasm_util::add_site(anyhow::Error::from(e), gdnative::log::godot_site!())
        })
        */
        $e.map_err(anyhow::Error::from)
    };
}

/*
pub fn add_site(e: Error, site: Site<'static>) -> Error {
    if e.is::<Site>() {
        e
    } else {
        e.context(site)
    }
}
*/

#[macro_export]
macro_rules! func_registry{
    ($head:literal, $($t:tt)*) => {
        $crate::func_registry!{(Funcs, $head), $($t)*}
    };
    (($fi:ident, $head:literal) $(, $i:ident => $e:expr)* $(,)?) => {
        #[derive(Default)]
        pub struct $fi {
            $($i: Option<Func>),*
        }

        impl $fi {
            pub fn get_func<T>(&mut self, store: &mut StoreContextMut<'_, T>, name: &str) -> Option<Func>
            where
                T: AsRef<StoreData> + AsMut<StoreData>,
            {
                match name {
                    $(concat!($head, stringify!($i)) => Some(self.$i.get_or_insert_with(move || Func::wrap(store, $e)).clone()),)*
                    _ => None,
                }
            }
        }
    };
}

pub fn option_to_variant<T: ToGodot>(t: Option<T>) -> Variant {
    t.map_or_else(Variant::nil, |t| t.to_variant())
}

pub fn variant_to_option<T: FromGodot>(v: Variant) -> Result<Option<T>, ConvertError> {
    if v.is_nil() {
        Ok(None)
    } else {
        Some(T::try_from_variant(&v)).transpose()
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

pub fn from_signature(sig: &FuncType) -> Result<(PackedByteArray, PackedByteArray), Error> {
    let p = sig.params();
    let r = sig.results();

    let mut pr = <Vec<u8>>::new();
    let mut rr = <Vec<u8>>::new();

    pr.resize(p.len() as _, 0);
    rr.resize(r.len() as _, 0);

    for (s, d) in p.zip(pr.iter_mut()).chain(r.zip(rr.iter_mut())) {
        *d = match s {
            ValType::I32 => TYPE_I32,
            ValType::I64 => TYPE_I64,
            ValType::F32 => TYPE_F32,
            ValType::F64 => TYPE_F64,
            #[cfg(feature = "object-registry-extern")]
            ValType::ExternRef => TYPE_VARIANT,
            _ => bail_with_site!("Unconvertible signture"),
        } as _;
    }

    Ok((
        PackedByteArray::from(&pr[..]),
        PackedByteArray::from(&rr[..]),
    ))
}

pub fn to_signature(params: Variant, results: Variant) -> Result<FuncType, Error> {
    fn f(it: impl Iterator<Item = Result<u32, Error>>) -> Result<Vec<ValType>, Error> {
        let mut ret = match it.size_hint() {
            (_, Some(n)) => Vec::with_capacity(n),
            (n, None) => Vec::with_capacity(n),
        };

        for i in it {
            ret.push(match site_context!(i)? {
                TYPE_I32 => ValType::I32,
                TYPE_I64 => ValType::I64,
                TYPE_F32 => ValType::F32,
                TYPE_F64 => ValType::F64,
                #[cfg(feature = "object-registry-extern")]
                TYPE_VARIANT => ValType::ExternRef,
                v => bail_with_site!("Unknown enumeration value {}", v),
            });
        }

        Ok(ret)
    }

    let p = match VariantDispatch::from(&params) {
        VariantDispatch::Array(v) => f(v
            .iter_shared()
            .map(|v| u32::try_from_variant(&v).map_err(Error::from)))?,
        VariantDispatch::PackedByteArray(v) => f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?,
        VariantDispatch::PackedInt32Array(v) => f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?,
        _ => bail_with_site!("Unconvertible value {}", params),
    };

    let r = match VariantDispatch::from(&results) {
        VariantDispatch::Array(v) => f(v
            .iter_shared()
            .map(|v| u32::try_from_variant(&v).map_err(Error::from)))?,
        VariantDispatch::PackedByteArray(v) => f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?,
        VariantDispatch::PackedInt32Array(v) => f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?,
        _ => bail_with_site!("Unconvertible value {}", results),
    };

    Ok(FuncType::new(p, r))
}

// Mark this unsafe for future proofing
pub unsafe fn to_raw(_store: impl AsContextMut, t: ValType, v: Variant) -> Result<ValRaw, Error> {
    Ok(match t {
        ValType::I32 => ValRaw::i32(site_context!(i32::try_from_variant(&v))?),
        ValType::I64 => ValRaw::i64(site_context!(i64::try_from_variant(&v))?),
        ValType::F32 => ValRaw::f32(site_context!(f32::try_from_variant(&v))?.to_bits()),
        ValType::F64 => ValRaw::f64(site_context!(f64::try_from_variant(&v))?.to_bits()),
        #[cfg(feature = "object-registry-extern")]
        ValType::ExternRef => ValRaw::externref(match variant_to_externref(v) {
            Some(v) => v.to_raw(_store),
            None => ptr::null_mut(),
        }),
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

// Mark this unsafe for future proofing
pub unsafe fn from_raw(_store: impl AsContextMut, t: ValType, v: ValRaw) -> Result<Variant, Error> {
    Ok(match t {
        ValType::I32 => v.get_i32().to_variant(),
        ValType::I64 => v.get_i64().to_variant(),
        ValType::F32 => f32::from_bits(v.get_f32()).to_variant(),
        ValType::F64 => f64::from_bits(v.get_f64()).to_variant(),
        #[cfg(feature = "object-registry-extern")]
        ValType::ExternRef => externref_to_variant(ExternRef::from_raw(v.get_externref())),
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

/// WARNING: Incredibly unsafe.
/// It's just used as workaround to pass Godot objects across closure.
/// (At least until it supports multi-threading)
pub(crate) struct SendSyncWrapper<T>(T);

unsafe impl<T> Send for SendSyncWrapper<T> {}
unsafe impl<T> Sync for SendSyncWrapper<T> {}

impl<T> SendSyncWrapper<T> {
    #[allow(dead_code)]
    pub(crate) fn new(v: T) -> Self {
        Self(v)
    }
}

impl<T> Deref for SendSyncWrapper<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for SendSyncWrapper<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

enum CallableEnum {
    ObjectMethod(Gd<Object>, StringName),
    Callable(Callable),
}

fn wrap_godot_method<T>(
    store: impl AsContextMut<Data = T>,
    ty: FuncType,
    callable: CallableEnum,
) -> Func
where
    T: AsRef<StoreData> + AsMut<StoreData>,
{
    let callable = SendSyncWrapper(callable);
    let ty_cloned = ty.clone();
    let f = move |mut ctx: Caller<T>, args: &mut [ValRaw]| -> Result<(), Error> {
        let pi = ty.params();
        let mut p = Vec::with_capacity(pi.len());
        for (ix, t) in pi.enumerate() {
            p.push(unsafe { from_raw(&mut ctx, t, args[ix])? });
        }

        let r = ctx.data_mut().as_mut().release_store(|| {
            site_context!(catch_unwind(AssertUnwindSafe(|| match &*callable {
                CallableEnum::ObjectMethod(obj, method) => {
                    match obj.clone().try_cast::<WeakRef>() {
                        Ok(obj) => <_>::from_variant(&obj.get_ref()),
                        Err(obj) => obj,
                    }
                    .call(method.clone(), &p)
                }
                CallableEnum::Callable(c) => c.callv(p.into_iter().collect()),
            }))
            .map_err(|_| anyhow!("Error trying to call")))
        })?;

        if let Some(msg) = ctx.data_mut().as_mut().error_signal.take() {
            return Err(Error::msg(msg));
        }

        let mut ri = ty.results();
        if ri.len() == 0 {
        } else if let Ok(r) = <Array<Variant>>::try_from_variant(&r) {
            for (ix, t) in ri.enumerate() {
                let v = r.get(ix as _);
                args[ix] = unsafe { to_raw(&mut ctx, t, v)? };
            }
        } else if ri.len() == 1 {
            args[0] = unsafe { to_raw(&mut ctx, ri.next().unwrap(), r)? };
        } else {
            bail_with_site!("Unconvertible return value {}", r);
        }

        #[cfg(feature = "epoch-timeout")]
        if let Config {
            with_epoch: true,
            epoch_autoreset: true,
            epoch_timeout,
            ..
        } = ctx.data().as_ref().config
        {
            ctx.as_context_mut().set_epoch_deadline(epoch_timeout);
        }

        Ok(())
    };

    unsafe { Func::new_unchecked(store, ty_cloned, f) }
}

static DATA_STRS: Lazy<(StringName, StringName, StringName, StringName, StringName)> =
    Lazy::new(|| {
        (
            StringName::from_latin1_with_nul(b"params\0"),
            StringName::from_latin1_with_nul(b"results\0"),
            StringName::from_latin1_with_nul(b"object\0"),
            StringName::from_latin1_with_nul(b"method\0"),
            StringName::from_latin1_with_nul(b"callable\0"),
        )
    });

fn process_func(dict: Dictionary) -> Result<(FuncType, CallableEnum), Error> {
    let Some(params) = dict.get(DATA_STRS.0.clone()) else {
        bail_with_site!("Key \"params\" does not exist")
    };
    let Some(results) = dict.get(DATA_STRS.1.clone()) else {
        bail_with_site!("Key \"results\" does not exist")
    };

    let callable = if let Some(c) = dict.get(DATA_STRS.4.clone()) {
        CallableEnum::Callable(site_context!(Callable::try_from_variant(&c))?)
    } else {
        let Some(object) = dict.get(DATA_STRS.2.clone()) else {
            bail_with_site!("Key \"object\" does not exist")
        };
        let Some(method) = dict.get(DATA_STRS.3.clone()) else {
            bail_with_site!("Key \"method\" does not exist")
        };

        CallableEnum::ObjectMethod(
            site_context!(<Gd<Object>>::try_from_variant(&object))?,
            match VariantDispatch::from(&method) {
                VariantDispatch::String(s) => StringName::from(&s),
                VariantDispatch::StringName(s) => s,
                _ => bail_with_site!("Unknown method name {}", method),
            },
        )
    };

    Ok((to_signature(params, results)?, callable))
}

cfg_if! {
    if #[cfg(feature = "new-host-import")] {
        pub struct HostModuleCache<T> {
            cache: Linker<T>,
            host: Dictionary,
        }
    } else {
        pub struct HostModuleCache<T> {
            cache: HashMap<String, Extern>,
            host: Dictionary,
            phantom: PhantomData<T>,
        }
    }
}

impl<T: AsRef<StoreData> + AsMut<StoreData>> HostModuleCache<T> {
    pub fn new(host: Dictionary) -> Self {
        cfg_if! {
            if #[cfg(feature = "new-host-import")] {
                Self {
                    cache: Linker::new(&ENGINE),
                    host,
                }
            } else {
                Self {
                    cache: HashMap::new(),
                    host,
                    phantom: PhantomData,
                }
            }
        }
    }

    pub fn get_extern<S: AsContextMut<Data = T>>(
        &mut self,
        store: &mut S,
        module: &str,
        name: &str,
    ) -> Result<Option<Extern>, Error> {
        cfg_if! {
            if #[cfg(feature = "new-host-import")] {
                if let r @ Some(_) = self.cache.get(&mut *store, module, name) {
                    Ok(r)
                } else if let Some(data) = self
                    .host
                    .get(module)
                    .map(|d| site_context!(Dictionary::try_from_variant(&d)))
                    .transpose()?
                    .and_then(|d| d.get(name))
                {
                    let (sig, callable) =
                        process_func(site_context!(Dictionary::try_from_variant(&data))?)?;

                    let v = Extern::from(wrap_godot_method(&mut *store, sig, callable));
                    self.cache.define(store, module, name, v.clone())?;
                    Ok(Some(v))
                } else {
                    Ok(None)
                }
            } else {
                if module != HOST_MODULE {
                    Ok(None)
                } else if let r @ Some(_) = self.cache.get(name).cloned() {
                    Ok(r)
                } else if let Some(data) = self.host.get(name) {
                    let (sig, callable) =
                        process_func(site_context!(Dictionary::try_from_variant(&data))?)?;

                    let v = Extern::from(wrap_godot_method(&mut *store, sig, callable));
                    self.cache.insert(name.to_string(), v.clone());
                    Ok(Some(v))
                } else {
                    Ok(None)
                }
            }
        }
    }
}

pub fn config_store_common<T>(store: &mut Store<T>) -> Result<(), Error>
where
    T: AsRef<StoreData> + AsMut<StoreData>,
{
    #[cfg(feature = "epoch-timeout")]
    if store.data().as_ref().config.with_epoch {
        store.epoch_deadline_trap();
        EPOCH.spawn_thread(|| ENGINE.increment_epoch());
    } else {
        store.epoch_deadline_callback(|_| Ok(UpdateDeadline::Continue(EPOCH_DEADLINE)));
    }

    #[cfg(feature = "memory-limiter")]
    {
        let data = store.data_mut().as_mut();
        if let Some(v) = data.config.max_memory {
            data.memory_limits.max_memory = v;
        }
        if let Some(v) = data.config.max_entries {
            data.memory_limits.max_table_entries = v;
        }
        store.limiter(|data| &mut data.as_mut().memory_limits);
    }

    Ok(())
}
