use std::borrow::Borrow;
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::rc::Rc;
#[cfg(feature = "epoch-timeout")]
use std::time;

use anyhow::{Error, Result as AnyResult};
use cfg_if::cfg_if;
use godot::classes::WeakRef;
use godot::prelude::*;
use tracing::{debug, info_span, instrument, Level};
#[cfg(feature = "wasi")]
use wasi_isolated_fs::context::WasiContext as WasiCtx;
#[cfg(feature = "epoch-timeout")]
use wasmtime::UpdateDeadline;
use wasmtime::{
    AsContext, AsContextMut, Caller, Extern, Func, FuncType, Linker, RootScope, Store,
    StoreContextMut, ValRaw, ValType,
};
#[cfg(feature = "object-registry-extern")]
use wasmtime::{ExternRef, HeapType, RefType};

use crate::godot_util::{from_var_any, SendSyncWrapper};
use crate::variant_dispatch;
use crate::wasm_config::Config;
use crate::wasm_engine::get_engine;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_engine::start_epoch;
#[cfg(feature = "object-registry-extern")]
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
#[cfg(feature = "memory-limiter")]
use crate::wasm_instance::MemoryLimit;
use crate::wasm_instance::StoreData;

#[cfg(all(feature = "epoch-timeout", feature = "more-precise-timer"))]
pub const EPOCH_MULTIPLIER: u64 = 1000;
#[cfg(all(feature = "epoch-timeout", not(feature = "more-precise-timer")))]
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

pub const TYPE_I32: i64 = 1;
pub const TYPE_I64: i64 = 2;
pub const TYPE_F32: i64 = 3;
pub const TYPE_F64: i64 = 4;
pub const TYPE_VARIANT: i64 = 6;
pub const TYPE_V128: i64 = 7;
pub const TYPE_UNKNOWN: i64 = -1;

#[cfg(feature = "object-registry-compat")]
pub const OBJREGISTRY_MODULE: &str = "godot_object_v1";
#[cfg(feature = "object-registry-extern")]
pub const EXTERNREF_MODULE: &str = "godot_object_v2";

pub const MEMORY_EXPORT: &str = "memory";

#[macro_export]
macro_rules! bail_with_site {
    ($($t:tt)*) => {
        /*
        return Err(anyhow::anyhow!($($t)*).context(gdnative::log::godot_site!()))
        */
        return Err(anyhow::anyhow!($($t)*)).inspect_err(|err| tracing::error!(%err, "Error happened"))
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
        $e.map_err(|err| {
            tracing::error!(%err, "Error happened");
            anyhow::Error::from(err)
        })
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

#[instrument(level = Level::TRACE)]
pub fn from_signature(sig: &FuncType) -> (PackedByteArray, PackedByteArray) {
    fn f(v: ValType) -> u8 {
        (match v {
            ValType::I32 => TYPE_I32,
            ValType::I64 => TYPE_I64,
            ValType::F32 => TYPE_F32,
            ValType::F64 => TYPE_F64,
            ValType::V128 => TYPE_V128,
            #[cfg(feature = "object-registry-extern")]
            ValType::Ref(r) if RefType::eq(&r, &RefType::EXTERNREF) => TYPE_VARIANT,
            _ => TYPE_UNKNOWN,
        }) as _
    }

    let p = sig.params();
    let r = sig.results();
    let mut v = Vec::with_capacity(p.len().max(r.len()));

    v.extend(p.map(f));
    let params = PackedByteArray::from(&*v);

    v.clear();
    v.extend(r.map(f));
    let results = PackedByteArray::from(&*v);

    (params, results)
}

#[instrument(level = Level::TRACE, skip(params, results), ret)]
pub fn to_signature(params: Variant, results: Variant, use_extern: bool) -> AnyResult<FuncType> {
    fn f(
        it: impl Iterator<Item = Result<i64, Error>>,
        _use_extern: bool,
    ) -> AnyResult<Vec<ValType>> {
        it.map(|i| {
            Ok(match i? {
                TYPE_I32 => ValType::I32,
                TYPE_I64 => ValType::I64,
                TYPE_F32 => ValType::F32,
                TYPE_F64 => ValType::F64,
                TYPE_V128 => ValType::V128,
                #[cfg(feature = "object-registry-extern")]
                TYPE_VARIANT if _use_extern => ValType::Ref(RefType::EXTERNREF),
                v => bail_with_site!(
                    "Unknown enumeration value {v}.{}",
                    if v == TYPE_VARIANT {
                        " Enable native Godot object API to be able to pass Variant type."
                    } else {
                        ""
                    }
                ),
            })
        })
        .collect()
    }

    let p = variant_dispatch!(params {
        ARRAY => f(params.iter_shared().map(|v| site_context!(from_var_any(v))), use_extern),
        PACKED_BYTE_ARRAY => f(params.as_slice().iter().map(|&v| Ok(v as _)), use_extern),
        PACKED_INT32_ARRAY => f(params.as_slice().iter().map(|&v| Ok(v as _)), use_extern),
        PACKED_INT64_ARRAY => f(params.as_slice().iter().map(|&v| Ok(v)), use_extern),
        _ => bail_with_site!("Unconvertible value {params}"),
    })?;

    let r = variant_dispatch!(results {
        ARRAY => f(results.iter_shared().map(|v| site_context!(from_var_any(v))), use_extern),
        PACKED_BYTE_ARRAY => f(results.as_slice().iter().map(|&v| Ok(v as _)), use_extern),
        PACKED_INT32_ARRAY => f(results.as_slice().iter().map(|&v| Ok(v as _)), use_extern),
        PACKED_INT64_ARRAY => f(results.as_slice().iter().map(|&v| Ok(v)), use_extern),
        _ => bail_with_site!("Unconvertible value {results}"),
    })?;

    Ok(FuncType::new(&site_context!(get_engine())?, p, r))
}

// Mark this unsafe for future proofing.
pub unsafe fn to_raw<T: AsRef<StoreData>>(
    mut _ctx: StoreContextMut<'_, T>,
    t: ValType,
    v: &Variant,
) -> AnyResult<ValRaw> {
    Ok(match t {
        ValType::I32 => ValRaw::i32(site_context!(from_var_any(v))?),
        ValType::I64 => ValRaw::i64(site_context!(from_var_any(v))?),
        ValType::F32 => ValRaw::f32(site_context!(from_var_any::<f32>(v))?.to_bits()),
        ValType::F64 => ValRaw::f64(site_context!(from_var_any::<f64>(v))?.to_bits()),
        ValType::V128 => ValRaw::v128(variant_dispatch!(v {
            INT => v as u128,
            VECTOR4I => (0..4).zip(v.to_array()).fold(0u128, |t, (s, v)| t | (v as u32 as u128) << (s * 32)),
            PACKED_BYTE_ARRAY => (0..16).zip(v.as_slice()).fold(0u128, |t, (s, &v)| t | (v as u128) << (s * 8)),
            PACKED_INT32_ARRAY => (0..4).zip(v.as_slice()).fold(0u128, |t, (s, &v)| t | (v as u128) << (s * 32)),
            PACKED_INT64_ARRAY => (0..2).zip(v.as_slice()).fold(0u128, |t, (s, &v)| t | (v as u128) << (s * 64)),
            ARRAY => {
                let v0 = site_context!(from_var_any::<u64>(v.get(0).unwrap_or_default()))?;
                let v1 = site_context!(from_var_any::<u64>(v.get(1).unwrap_or_default()))?;
                v0 as u128 | (v1 as u128) << 64
            },
            _ => bail_with_site!("Unknown value type {:?}", v.get_type()),
        })),
        #[cfg(feature = "object-registry-extern")]
        ValType::Ref(r)
            if matches!(r.heap_type(), HeapType::Extern) && _ctx.data().as_ref().use_extern =>
        {
            ValRaw::externref(
                match variant_to_externref(_ctx.as_context_mut(), v.clone())? {
                    Some(v) => v.to_raw(_ctx)?,
                    None if r.is_nullable() => 0,
                    None => bail_with_site!("Converting null into non-nullable WASM type"),
                },
            )
        }
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

// Mark this unsafe for future proofing.
pub unsafe fn from_raw<T: AsRef<StoreData>>(
    mut _ctx: StoreContextMut<'_, T>,
    t: ValType,
    v: ValRaw,
) -> AnyResult<Variant> {
    Ok(match t {
        ValType::I32 => v.get_i32().to_variant(),
        ValType::I64 => v.get_i64().to_variant(),
        ValType::F32 => f32::from_bits(v.get_f32()).to_variant(),
        ValType::F64 => f64::from_bits(v.get_f64()).to_variant(),
        ValType::V128 => {
            let v = v.get_v128();
            Vector4i::new(v as _, (v >> 32) as _, (v >> 64) as _, (v >> 96) as _).to_variant()
        }
        #[cfg(feature = "object-registry-extern")]
        ValType::Ref(r)
            if _ctx.data().as_ref().use_extern && matches!(r.heap_type(), HeapType::Extern) =>
        {
            let v = ExternRef::from_raw(_ctx.as_context_mut(), v.get_externref());
            return externref_to_variant(_ctx.as_context(), v);
        }
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

struct ParamCache<T> {
    len: Cell<usize>,
    data: Box<[MaybeUninit<T>]>,
}

struct ParamCacheGuard<T> {
    parent: Rc<ParamCache<T>>,
    old_len: usize,
    data: NonNull<[T]>,
    _p: PhantomData<*mut [T]>,
}

impl<T> Drop for ParamCacheGuard<T> {
    fn drop(&mut self) {
        let data_len;
        // SAFETY: We have exclusive access to data.
        unsafe {
            data_len = self.data.as_mut().len();
            self.data.drop_in_place();
        }
        let v = self.parent.len.replace(self.parent.len.get() - data_len);
        debug_assert_eq!(v, self.old_len);
    }
}

impl<T> Deref for ParamCacheGuard<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        // SAFETY: We have exclusive access to data.
        unsafe { self.data.as_ref() }
    }
}

impl<T> DerefMut for ParamCacheGuard<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        // SAFETY: We have exclusive access to data.
        unsafe { self.data.as_mut() }
    }
}

impl<T> ParamCache<T> {
    fn new(len: usize) -> Self {
        Self {
            len: Cell::new(0),
            data: Box::new_uninit_slice(len),
        }
    }

    fn get(this: &mut Rc<Self>, n: usize, fill: impl Fn() -> T) -> ParamCacheGuard<T> {
        let this = if this.len.get() + n > this.data.len() {
            let mut l = this.data.len() * 2;
            while l < n {
                l *= 2;
            }
            let o = Rc::new(Self {
                len: Cell::new(0),
                data: Box::new_uninit_slice(l),
            });
            *this = o.clone();
            o
        } else {
            this.clone()
        };
        let len = this.len.get();
        let v = this.len.replace(len + n);
        debug_assert_eq!(v, len);
        // SAFETY: We have exclusive right to that portion of data.
        let data = unsafe {
            let mut p = NonNull::from(&this.data[len..len + n]);
            for i in p.as_mut() {
                // If fill panics some data might be leaked, but that's okay.
                i.write(fill());
            }
            NonNull::new_unchecked(p.as_ptr() as *mut [T])
        };
        ParamCacheGuard {
            parent: this,
            old_len: len + n,
            data,
            _p: PhantomData,
        }
    }
}

thread_local! {
    static PARAM_CACHE: UnsafeCell<Rc<ParamCache<ValRaw>>> = UnsafeCell::new(Rc::new(ParamCache::new(64)));
    static GODOT_PARAM_CACHE: UnsafeCell<Rc<ParamCache<Variant>>> = UnsafeCell::new(Rc::new(ParamCache::new(64)));
}

pub fn get_godot_param_cache(len: usize) -> impl DerefMut<Target = [Variant]> {
    GODOT_PARAM_CACHE.with(|v| {
        // SAFETY: We have exclusive right to value.
        unsafe { ParamCache::get(&mut *v.get(), len, Variant::nil) }
    })
}

pub unsafe fn raw_call<T, It>(
    ctx: StoreContextMut<'_, T>,
    f: &Func,
    ty: &FuncType,
    args: It,
) -> AnyResult<VariantArray>
where
    T: AsRef<StoreData>,
    It: IntoIterator,
    It::Item: Borrow<Variant>,
{
    let pi = ty.params();
    let ri = ty.results();
    let pl = pi.len();
    let l = pl.max(ri.len());

    let mut v = PARAM_CACHE.with(|v| {
        // SAFETY: We have exclusive right to value.
        unsafe { ParamCache::get(&mut *v.get(), l, || ValRaw::i32(0)) }
    });

    let mut ctx = RootScope::new(ctx);
    ctx.as_context_mut().gc();

    let mut args = args.into_iter();
    for (p, (i, o)) in pi.zip(v.iter_mut().enumerate()) {
        let Some(v) = args.next() else {
            bail_with_site!("Too few parameters (expected {pl}, got {i})")
        };
        *o = to_raw(ctx.as_context_mut(), p, v.borrow())?;
    }
    drop(args);

    f.call_unchecked(ctx.as_context_mut(), &mut *v)?;

    ri.zip(v.iter())
        .map(|(t, v)| from_raw(ctx.as_context_mut(), t, *v))
        .collect()
}

#[derive(Debug)]
enum CallableEnum {
    ObjectMethod(Gd<Object>, StringName),
    Callable(Callable),
}

#[instrument(level = Level::DEBUG, skip(ctx))]
fn wrap_godot_method<T>(ctx: StoreContextMut<'_, T>, ty: FuncType, callable: CallableEnum) -> Func
where
    T: AsRef<StoreData> + AsMut<StoreData> + HasEpochTimeout,
{
    let callable = SendSyncWrapper::new(callable);
    let ty_cloned = ty.clone();
    let _s = info_span!("wrap_godot_method.inner", ?callable);
    let f = move |mut ctx: Caller<T>, args: &mut [ValRaw]| -> AnyResult<()> {
        let _s = _s.enter();

        let mut p = get_godot_param_cache(args.len());
        for (ix, t) in ty.params().enumerate() {
            p[ix] = unsafe { from_raw(ctx.as_context_mut(), t, args[ix])? };
        }

        let r = match &*callable {
            CallableEnum::ObjectMethod(obj, method) => {
                let mut obj = match obj.clone().try_cast::<WeakRef>() {
                    Ok(obj) => site_context!(from_var_any(obj.get_ref()))?,
                    Err(obj) => obj,
                };
                ctx.data_mut()
                    .as_mut()
                    .release_store(move || site_context!(obj.try_call(method, &p)))?
            }
            CallableEnum::Callable(c) => ctx.data_mut().as_mut().release_store(move || c.call(&p)),
        };

        if let Some(msg) = ctx.data_mut().as_mut().error_signal.take() {
            return Err(Error::msg(msg));
        }

        let mut ri = ty.results();
        let rl = ri.len();
        if rl == 0 {
        } else if let Ok(r) = r.try_to::<VariantArray>() {
            for (t, (i, o)) in ri.zip(args.iter_mut().enumerate()) {
                let Some(v) = r.get(i) else {
                    bail_with_site!("Too few return value (expected {rl}, got {i})")
                };
                *o = unsafe { to_raw(ctx.as_context_mut(), t, &v)? };
            }
        } else if rl == 1 {
            args[0] = unsafe { to_raw(ctx.as_context_mut(), ri.next().unwrap(), &r)? };
        } else {
            bail_with_site!("Unconvertible return value {}", r);
        }

        #[cfg(feature = "epoch-timeout")]
        if ctx.data().as_ref().epoch_autoreset {
            reset_epoch(ctx.as_context_mut());
        }

        Ok(())
    };

    unsafe { Func::new_unchecked(ctx, ty_cloned, f) }
}

fn process_func(dict: Dictionary, use_extern: bool) -> AnyResult<(FuncType, CallableEnum)> {
    let Some(params) = dict.get(StringName::from(c"params")) else {
        bail_with_site!("Key \"params\" does not exist")
    };
    let Some(results) = dict.get(StringName::from(c"results")) else {
        bail_with_site!("Key \"results\" does not exist")
    };

    let callable = if let Some(c) = dict.get(StringName::from(c"callable")) {
        CallableEnum::Callable(site_context!(from_var_any(c))?)
    } else {
        let Some(object) = dict.get(StringName::from(c"object")) else {
            bail_with_site!("Key \"object\" does not exist")
        };
        let Some(method) = dict.get(StringName::from(c"method")) else {
            bail_with_site!("Key \"method\" does not exist")
        };

        CallableEnum::ObjectMethod(
            site_context!(from_var_any(object))?,
            variant_dispatch!(method {
                STRING => method.into(),
                STRING_NAME => method,
                _ => bail_with_site!("Unknown method name type {:?}", method.get_type()),
            }),
        )
    };

    Ok((to_signature(params, results, use_extern)?, callable))
}

pub struct HostModuleCache<T> {
    cache: Linker<T>,
    host: Dictionary,
}

impl<T: AsRef<StoreData> + AsMut<StoreData> + HasEpochTimeout> HostModuleCache<T> {
    pub fn new(host: Dictionary) -> AnyResult<Self> {
        Ok(Self {
            cache: Linker::new(&site_context!(get_engine())?),
            host,
        })
    }

    pub fn get_extern(
        &mut self,
        mut ctx: StoreContextMut<'_, T>,
        module: &str,
        name: &str,
    ) -> AnyResult<Option<Extern>> {
        if let r @ Some(_) = self.cache.get(ctx.as_context_mut(), module, name) {
            Ok(r)
        } else if let Some(data) = self
            .host
            .get(module)
            .map(|d| site_context!(from_var_any::<Dictionary>(d)))
            .transpose()?
            .and_then(|d| d.get(name))
        {
            cfg_if! {
                if #[cfg(feature = "object-registry-extern")] {
                    let use_extern = ctx.as_context_mut().data().as_ref().use_extern;
                } else {
                    let use_extern = false;
                }
            }
            let (sig, callable) =
                process_func(site_context!(from_var_any::<Dictionary>(data))?, use_extern)?;

            let v = Extern::from(wrap_godot_method(ctx.as_context_mut(), sig, callable));
            self.cache.define(ctx, module, name, v.clone())?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }
}

#[cfg(feature = "epoch-timeout")]
#[instrument(level = Level::TRACE, skip_all)]
pub fn config_store_epoch<T: HasEpochTimeout>(
    store: &mut Store<T>,
    config: &Config,
) -> AnyResult<()> {
    if config.with_epoch {
        store.epoch_deadline_trap();
        site_context!(start_epoch())?;
    } else {
        store.epoch_deadline_callback(|_| Ok(UpdateDeadline::Continue(EPOCH_DEADLINE)));
    }
    reset_epoch(store.as_context_mut());
    Ok(())
}

#[instrument(level = Level::TRACE, skip_all)]
pub fn config_store_common<T>(_store: &mut Store<T>, _config: &Config) -> AnyResult<()>
where
    T: AsRef<StoreData> + AsMut<StoreData> + HasEpochTimeout,
{
    #[cfg(feature = "epoch-timeout")]
    {
        config_store_epoch(&mut *_store, _config)?;
        let data = _store.data_mut().as_mut();
        data.epoch_timeout = if _config.with_epoch {
            _config.epoch_timeout
        } else {
            0
        };
        data.epoch_autoreset = _config.epoch_autoreset;
    }

    #[cfg(feature = "memory-limiter")]
    {
        _store.data_mut().as_mut().memory_limits = MemoryLimit::from_config(_config);
        _store.limiter(|data| &mut data.as_mut().memory_limits);
    }

    Ok(())
}

pub trait HasEpochTimeout {
    #[cfg(feature = "epoch-timeout")]
    fn get_epoch_timeout(&self) -> u64;
    #[cfg(feature = "wasi")]
    fn get_wasi_ctx(&mut self) -> Option<&mut WasiCtx>;
}

#[cfg(feature = "epoch-timeout")]
#[instrument(level = Level::DEBUG, skip_all)]
pub fn reset_epoch<T: HasEpochTimeout>(mut ctx: StoreContextMut<'_, T>) {
    let v = ctx.data_mut();
    let t @ 1.. = v.get_epoch_timeout() else {
        return;
    };

    let d = EPOCH_INTERVAL * u32::try_from(t).unwrap_or(u32::MAX);
    debug!(ticks = t, delta = ?d, "Reset epoch");

    #[cfg(feature = "wasi")]
    if let Some(ctx) = v.get_wasi_ctx() {
        ctx.set_timeout(time::Instant::now() + (d + EPOCH_INTERVAL));
    }

    ctx.set_epoch_deadline(t);
}
