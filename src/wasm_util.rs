use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::ptr;
#[cfg(feature = "epoch-timeout")]
use std::time;

use anyhow::{Error, Result as AnyResult};

use godot::classes::WeakRef;
use godot::prelude::*;

#[cfg(feature = "epoch-timeout")]
use wasmtime::UpdateDeadline;
use wasmtime::{
    AsContextMut, Caller, Extern, Func, FuncType, Linker, RootScope, Store, ValRaw, ValType,
};
#[cfg(feature = "object-registry-extern")]
use wasmtime::{ExternRef, HeapType, RefType};

use crate::godot_util::{from_var_any, SendSyncWrapper, VariantDispatch};
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

/*
#[cfg(feature = "wasi")]
pub const FILE_NOTEXIST: u32 = 0;
#[cfg(feature = "wasi")]
pub const FILE_FILE: u32 = 1;
#[cfg(feature = "wasi")]
pub const FILE_DIR: u32 = 2;
#[cfg(feature = "wasi")]
pub const FILE_LINK: u32 = 3;
*/

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;
#[cfg(feature = "object-registry-extern")]
pub const TYPE_VARIANT: u32 = 6;
pub const TYPE_V128: u32 = 7;

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

pub fn from_signature(sig: &FuncType) -> AnyResult<(PackedByteArray, PackedByteArray)> {
    fn f(v: ValType) -> Result<u8, Error> {
        Ok(match v {
            ValType::I32 => TYPE_I32,
            ValType::I64 => TYPE_I64,
            ValType::F32 => TYPE_F32,
            ValType::F64 => TYPE_F64,
            ValType::V128 => TYPE_V128,
            #[cfg(feature = "object-registry-extern")]
            ValType::Ref(r) if RefType::eq(&r, &RefType::EXTERNREF) => TYPE_VARIANT,
            _ => bail_with_site!("Unconvertible value type {v}"),
        } as _)
    }

    Ok((
        sig.params().map(f).collect::<Result<_, _>>()?,
        sig.results().map(f).collect::<Result<_, _>>()?,
    ))
}

pub fn to_signature(params: Variant, results: Variant) -> AnyResult<FuncType> {
    fn f(it: impl Iterator<Item = Result<u32, Error>>) -> AnyResult<Vec<ValType>> {
        it.map(|i| {
            Ok(match i? {
                TYPE_I32 => ValType::I32,
                TYPE_I64 => ValType::I64,
                TYPE_F32 => ValType::F32,
                TYPE_F64 => ValType::F64,
                TYPE_V128 => ValType::V128,
                #[cfg(feature = "object-registry-extern")]
                TYPE_VARIANT => ValType::Ref(RefType::EXTERNREF),
                v => bail_with_site!("Unknown enumeration value {v}"),
            })
        })
        .collect()
    }

    let p = match VariantDispatch::from(&params) {
        VariantDispatch::Array(v) => f(v.iter_shared().map(|v| site_context!(from_var_any(v))))?,
        VariantDispatch::PackedByteArray(v) => f(v.as_slice().iter().map(|&v| Ok(v as u32)))?,
        VariantDispatch::PackedInt32Array(v) => f(v.as_slice().iter().map(|&v| Ok(v as u32)))?,
        VariantDispatch::PackedInt64Array(v) => f(v.as_slice().iter().map(|&v| Ok(v as u32)))?,
        _ => bail_with_site!("Unconvertible value {params}"),
    };

    let r = match VariantDispatch::from(&results) {
        VariantDispatch::Array(v) => f(v.iter_shared().map(|v| site_context!(from_var_any(v))))?,
        VariantDispatch::PackedByteArray(v) => f(v.as_slice().iter().map(|&v| Ok(v as u32)))?,
        VariantDispatch::PackedInt32Array(v) => f(v.as_slice().iter().map(|&v| Ok(v as u32)))?,
        VariantDispatch::PackedInt64Array(v) => f(v.as_slice().iter().map(|&v| Ok(v as u32)))?,
        _ => bail_with_site!("Unconvertible value {results}"),
    };

    Ok(FuncType::new(&site_context!(get_engine())?, p, r))
}

// Mark this unsafe for future proofing
pub unsafe fn to_raw(mut _store: impl AsContextMut, t: ValType, v: &Variant) -> AnyResult<ValRaw> {
    Ok(match t {
        ValType::I32 => ValRaw::i32(site_context!(from_var_any(v))?),
        ValType::I64 => ValRaw::i64(site_context!(from_var_any(v))?),
        ValType::F32 => ValRaw::f32(site_context!(from_var_any::<f32>(v))?.to_bits()),
        ValType::F64 => ValRaw::f64(site_context!(from_var_any::<f64>(v))?.to_bits()),
        ValType::V128 => ValRaw::v128(match VariantDispatch::from(v) {
            VariantDispatch::Int(v) => v as u128,
            VariantDispatch::PackedByteArray(v) => {
                let Some(s) = v.as_slice().get(..16) else {
                    bail_with_site!("Value too short for 128-bit integer")
                };
                u128::from_le_bytes(s.try_into().unwrap())
            }
            VariantDispatch::PackedInt32Array(v) => {
                let Some(s) = v.as_slice().get(..4) else {
                    bail_with_site!("Value too short for 128-bit integer")
                };
                s[0] as u128 | (s[1] as u128) << 32 | (s[2] as u128) << 64 | (s[3] as u128) << 96
            }
            VariantDispatch::PackedInt64Array(v) => {
                let Some(s) = v.as_slice().get(..2) else {
                    bail_with_site!("Value too short for 128-bit integer")
                };
                s[0] as u128 | (s[1] as u128) << 64
            }
            VariantDispatch::Array(v) => {
                let v0 = site_context!(from_var_any::<u64>(v.get(0).unwrap_or_default()))?;
                let v1 = site_context!(from_var_any::<u64>(v.get(1).unwrap_or_default()))?;
                v0 as u128 | (v1 as u128) << 64
            }
            _ => bail_with_site!("Unknown value type {:?}", v.get_type()),
        }),
        #[cfg(feature = "object-registry-extern")]
        ValType::Ref(r) if matches!(r.heap_type(), HeapType::Extern) => {
            ValRaw::externref(match variant_to_externref(&mut _store, v.clone())? {
                Some(v) => v.to_raw(_store)?,
                None if r.is_nullable() => 0,
                None => bail_with_site!("Converting null into non-nullable WASM type"),
            })
        }
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

// Mark this unsafe for future proofing
pub unsafe fn from_raw(mut _store: impl AsContextMut, t: ValType, v: ValRaw) -> AnyResult<Variant> {
    Ok(match t {
        ValType::I32 => v.get_i32().to_variant(),
        ValType::I64 => v.get_i64().to_variant(),
        ValType::F32 => f32::from_bits(v.get_f32()).to_variant(),
        ValType::F64 => f64::from_bits(v.get_f64()).to_variant(),
        ValType::V128 => {
            let v = v.get_v128();
            [v as u64, (v >> 64) as u64]
                .into_iter()
                .map(|v| v.to_variant())
                .collect::<VariantArray>()
                .to_variant()
        }
        #[cfg(feature = "object-registry-extern")]
        ValType::Ref(r) if matches!(r.heap_type(), HeapType::Extern) => {
            let v = ExternRef::from_raw(&mut _store, v.get_externref());
            return externref_to_variant(_store, v);
        }
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

thread_local! {
    static CALLVEC: UnsafeCell<Vec<ValRaw>> = const { UnsafeCell::new(Vec::new()) };
}

pub unsafe fn raw_call<It>(
    ctx: impl AsContextMut,
    f: &Func,
    ty: &FuncType,
    args: It,
) -> AnyResult<VariantArray>
where
    It: IntoIterator,
    It::Item: Borrow<Variant>,
{
    struct F(ManuallyDrop<Vec<ValRaw>>);

    impl Drop for F {
        fn drop(&mut self) {
            // SAFETY: The container will be forgotten.
            let mut t = unsafe { ManuallyDrop::take(&mut self.0) };
            t.clear();
            let _ = CALLVEC.try_with(move |v| {
                // SAFETY: The cell contains valid thread-local value.
                let o = unsafe { &mut *v.get() };
                if t.capacity() > o.capacity() {
                    *o = t;
                }
            });
        }
    }

    impl Deref for F {
        type Target = Vec<ValRaw>;
        fn deref(&self) -> &Vec<ValRaw> {
            &self.0
        }
    }

    impl DerefMut for F {
        fn deref_mut(&mut self) -> &mut Vec<ValRaw> {
            &mut self.0
        }
    }

    let mut v = CALLVEC.with(|v| F(ManuallyDrop::new(ptr::replace(v.get(), Vec::new()))));
    v.clear();

    let mut ctx = RootScope::new(ctx);
    ctx.as_context_mut().gc();

    let pi = ty.params();
    let ri = ty.results();

    let pl = pi.len();
    v.resize(pl.max(ri.len()), ValRaw::i32(0));

    let mut args = args.into_iter();
    for (p, (i, o)) in pi.zip(v.iter_mut().enumerate()) {
        let Some(v) = args.next() else {
            bail_with_site!("Too few parameters (expected {pl}, got {i})")
        };
        *o = to_raw(&mut ctx, p, v.borrow())?;
    }
    drop(args);

    f.call_unchecked(&mut ctx, v.as_mut_ptr(), v.len())?;

    ri.zip(v.iter())
        .map(|(t, v)| from_raw(&mut ctx, t, *v))
        .collect()
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
    let callable = SendSyncWrapper::new(callable);
    let ty_cloned = ty.clone();
    let f = move |mut ctx: Caller<T>, args: &mut [ValRaw]| -> AnyResult<()> {
        let pi = ty.params();
        let mut p = Vec::with_capacity(pi.len());
        for (ix, t) in pi.enumerate() {
            p.push(unsafe { from_raw(&mut ctx, t, args[ix])? });
        }

        let r = ctx.data_mut().as_mut().release_store(|| {
            site_context!(match &*callable {
                CallableEnum::ObjectMethod(obj, method) => {
                    match obj.clone().try_cast::<WeakRef>() {
                        Ok(obj) => site_context!(from_var_any(obj.get_ref()))?,
                        Err(obj) => obj,
                    }
                    .try_call(method.clone(), &p)
                }
                CallableEnum::Callable(c) => Ok(c.callv(p.into_iter().collect())),
            })
        })?;

        if let Some(msg) = ctx.data_mut().as_mut().error_signal.take() {
            return Err(Error::msg(msg));
        }

        let mut ri = ty.results();
        if ri.len() == 0 {
        } else if let Ok(r) = r.try_to::<VariantArray>() {
            let rl = ri.len();
            for (t, (i, o)) in ri.zip(args.iter_mut().enumerate()) {
                let Some(v) = r.get(i) else {
                    bail_with_site!("Too few return value (expected {rl}, got {i})")
                };
                *o = unsafe { to_raw(&mut ctx, t, &v)? };
            }
        } else if ri.len() == 1 {
            args[0] = unsafe { to_raw(&mut ctx, ri.next().unwrap(), &r)? };
        } else {
            bail_with_site!("Unconvertible return value {}", r);
        }

        #[cfg(feature = "epoch-timeout")]
        if let StoreData {
            epoch_autoreset: true,
            epoch_timeout: v @ 1..,
            ..
        } = *ctx.data().as_ref()
        {
            ctx.as_context_mut().set_epoch_deadline(v);
        }

        Ok(())
    };

    unsafe { Func::new_unchecked(store, ty_cloned, f) }
}

fn process_func(dict: Dictionary) -> AnyResult<(FuncType, CallableEnum)> {
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
            match method.get_type() {
                VariantType::STRING => method.to::<GString>().into(),
                VariantType::STRING_NAME => method.to(),
                _ => bail_with_site!("Unknown method name {method}"),
            },
        )
    };

    Ok((to_signature(params, results)?, callable))
}

pub struct HostModuleCache<T> {
    cache: Linker<T>,
    host: Dictionary,
}

impl<T: AsRef<StoreData> + AsMut<StoreData>> HostModuleCache<T> {
    pub fn new(host: Dictionary) -> AnyResult<Self> {
        Ok(Self {
            cache: Linker::new(&site_context!(get_engine())?),
            host,
        })
    }

    pub fn get_extern<S: AsContextMut<Data = T>>(
        &mut self,
        store: &mut S,
        module: &str,
        name: &str,
    ) -> AnyResult<Option<Extern>> {
        if let r @ Some(_) = self.cache.get(&mut *store, module, name) {
            Ok(r)
        } else if let Some(data) = self
            .host
            .get(module)
            .map(|d| site_context!(from_var_any::<Dictionary>(d)))
            .transpose()?
            .and_then(|d| d.get(name))
        {
            let (sig, callable) = process_func(site_context!(from_var_any::<Dictionary>(data))?)?;

            let v = Extern::from(wrap_godot_method(&mut *store, sig, callable));
            self.cache.define(store, module, name, v.clone())?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }
}

#[cfg(feature = "epoch-timeout")]
pub fn config_store_epoch<T>(store: &mut Store<T>, config: &Config) -> AnyResult<()> {
    if config.with_epoch {
        store.epoch_deadline_trap();
        site_context!(start_epoch())?;
    } else {
        store.epoch_deadline_callback(|_| Ok(UpdateDeadline::Continue(EPOCH_DEADLINE)));
    }
    store.set_epoch_deadline(config.epoch_timeout);
    Ok(())
}

pub fn config_store_common<T>(_store: &mut Store<T>, _config: &Config) -> AnyResult<()>
where
    T: AsRef<StoreData> + AsMut<StoreData>,
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
