use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(feature = "epoch-timeout")]
use std::time;

use anyhow::{anyhow, bail, Error};
use godot::prelude::*;
#[cfg(feature = "object-registry-extern")]
use wasmtime::ExternRef;
use wasmtime::{AsContextMut, Caller, Extern, Func, FuncType, Store, ValRaw, ValType};

#[cfg(feature = "epoch-timeout")]
use crate::wasm_config::Config;
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

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;
#[cfg(feature = "object-registry-extern")]
pub const TYPE_VARIANT: u32 = 6;

pub const HOST_MODULE: &str = "host";
#[cfg(feature = "object-registry-compat")]
pub const OBJREGISTRY_MODULE: &str = "godot_object_v1";
#[cfg(feature = "object-registry-extern")]
pub const EXTERNREF_MODULE: &str = "godot_object_v2";

pub const MODULE_INCLUDES: &[&str] = &[
    HOST_MODULE,
    #[cfg(feature = "object-registry-compat")]
    OBJREGISTRY_MODULE,
    #[cfg(feature = "object-registry-extern")]
    EXTERNREF_MODULE,
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
        $e
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

pub fn option_to_variant<T: ToVariant>(t: Option<T>) -> Variant {
    t.map_or_else(Variant::nil, |t| t.to_variant())
}

pub fn variant_to_option<T: FromVariant>(v: Variant) -> Result<Option<T>, VariantConversionError> {
    if v.is_nil() {
        Ok(None)
    } else {
        Some(T::try_from_variant(&v)).transpose()
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
            ret.push(match i? {
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

    let p = if let Ok(v) = <Array<Variant>>::try_from_variant(&params) {
        f(v.iter_shared().map(|v| {
            Ok(site_context!(
                u32::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e))
            )?)
        }))?
    } else if let Ok(v) = PackedByteArray::try_from_variant(&params) {
        f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?
    } else if let Ok(v) = PackedInt32Array::try_from_variant(&params) {
        f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?
    } else {
        bail!("Unconvertible value {}", params)
    };

    let r = if let Ok(v) = <Array<Variant>>::try_from_variant(&results) {
        f(v.iter_shared().map(|v| {
            Ok(site_context!(
                u32::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e))
            )?)
        }))?
    } else if let Ok(v) = PackedByteArray::try_from_variant(&results) {
        f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?
    } else if let Ok(v) = PackedInt32Array::try_from_variant(&results) {
        f(v.to_vec().into_iter().map(|v| Ok(v as u32)))?
    } else {
        bail!("Unconvertible value {}", results)
    };

    Ok(FuncType::new(p, r))
}

// Mark this unsafe for future proofing
pub unsafe fn to_raw(_store: impl AsContextMut, t: ValType, v: Variant) -> Result<ValRaw, Error> {
    Ok(match t {
        ValType::I32 => ValRaw::i32(site_context!(
            i32::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e))
        )?),
        ValType::I64 => ValRaw::i64(site_context!(
            i64::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e))
        )?),
        ValType::F32 => ValRaw::f32(
            site_context!(f32::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e)))?.to_bits(),
        ),
        ValType::F64 => ValRaw::f64(
            site_context!(f64::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e)))?.to_bits(),
        ),
        #[cfg(feature = "object-registry-extern")]
        ValType::ExternRef => ValRaw::externref(match variant_to_externref(v) {
            Some(v) => v.to_raw(_store),
            None => 0,
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
struct SendSyncWrapper<T>(T);

unsafe impl<T> Send for SendSyncWrapper<T> {}
unsafe impl<T> Sync for SendSyncWrapper<T> {}

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

fn wrap_godot_method(
    store: impl AsContextMut<Data = StoreData>,
    ty: FuncType,
    obj: Variant,
    method: GodotString,
) -> Func {
    let obj = SendSyncWrapper(obj);
    let method = SendSyncWrapper(method);
    let ty_cloned = ty.clone();
    let f = move |mut ctx: Caller<StoreData>, args: &mut [ValRaw]| -> Result<(), Error> {
        let pi = ty.params();
        let mut p = Vec::with_capacity(pi.len());
        for (ix, t) in pi.enumerate() {
            p.push(unsafe { from_raw(&mut ctx, t, args[ix])? });
        }

        // XXX: Weakref is currently broken (i think it's upstream?)
        //let obj = match <Gd<WeakRef>>::try_from_variant(&obj) {
        //    Ok(obj) => obj.get_ref(),
        //    Err(_) => obj.clone(),
        //};
        let mut obj = match <Gd<Object>>::try_from_variant(&obj) {
            Ok(v) => v,
            Err(_) => bail!("Cannot convert object"),
        };
        let r = ctx.data_mut().release_store(|| {
            site_context!(catch_unwind(AssertUnwindSafe(
                || obj.call(StringName::from(&*method), &p)
            ))
            .map_err(|_| anyhow!("Error trying to call")))
        })?;

        if let Some(msg) = ctx.data_mut().error_signal.take() {
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
        } = ctx.data().config
        {
            ctx.as_context_mut().set_epoch_deadline(epoch_timeout);
        }

        Ok(())
    };

    unsafe { Func::new_unchecked(store, ty_cloned, f) }
}

pub fn make_host_module(
    store: &mut Store<StoreData>,
    dict: Dictionary,
) -> Result<HashMap<String, Extern>, Error> {
    let mut ret = HashMap::new();
    for (k, v) in dict.iter_shared() {
        let k = site_context!(GodotString::try_from_variant(&k).map_err(|e| anyhow!("{:?}", e)))?
            .to_string();

        struct Data {
            params: Variant,
            results: Variant,
            object: Variant,
            method: GodotString,
        }

        let data = {
            let v = Dictionary::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e))?;
            let Some(params) = v.get(StringName::from("params")) else { bail_with_site!("Key \"params\" does not exist") };
            let Some(results) = v.get(StringName::from("results")) else { bail_with_site!("Key \"params\" does not exist") };
            let Some(object) = v.get(StringName::from("object")) else { bail_with_site!("Key \"params\" does not exist") };
            let Some(method) = v.get(StringName::from("method")) else { bail_with_site!("Key \"params\" does not exist") };

            Data {
                params,
                results,
                object,
                method: <_>::try_from_variant(&method).map_err(|e| anyhow!("{:?}", e))?,
            }
        };

        ret.insert(
            k,
            wrap_godot_method(
                &mut *store,
                to_signature(data.params, data.results)?,
                data.object,
                data.method,
            )
            .into(),
        );
    }

    Ok(ret)
}
