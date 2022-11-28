use std::collections::HashMap;

use anyhow::{bail, Error};
use gdnative::api::WeakRef;
use gdnative::prelude::*;
use wasmtime::{AsContextMut, Caller, Extern, Func, FuncType, Store, ValRaw, ValType};

use crate::wasm_instance::StoreData;

pub const EPOCH_DEADLINE: u64 = 30;

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;
pub const TYPE_VARIANT: u32 = 6;

pub const HOST_MODULE: &str = "host";

pub const MODULE_INCLUDES: &[&str] = &[HOST_MODULE];

pub const MEMORY_EXPORT: &str = "memory";

pub fn from_signature(sig: &FuncType) -> Result<(ByteArray, ByteArray), Error> {
    let p = sig.params();
    let r = sig.results();

    let mut pr = ByteArray::new();
    let mut rr = ByteArray::new();

    pr.resize(p.len() as _);
    rr.resize(r.len() as _);

    for (s, d) in p
        .zip(pr.write().iter_mut())
        .chain(r.zip(rr.write().iter_mut()))
    {
        *d = match s {
            ValType::I32 => TYPE_I32,
            ValType::I64 => TYPE_I64,
            ValType::F32 => TYPE_F32,
            ValType::F64 => TYPE_F64,
            _ => bail!("Unconvertible signture"),
        } as _;
    }

    Ok((pr, rr))
}

pub fn to_signature(params: Variant, results: Variant) -> Result<FuncType, Error> {
    let p;
    let r;

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
                v => bail!("Unknown enumeration value {}", v),
            });
        }

        Ok(ret)
    }

    p = match VariantDispatch::from(&params) {
        VariantDispatch::VariantArray(v) => f(v.into_iter().map(|v| Ok(u32::from_variant(&v)?))),
        VariantDispatch::ByteArray(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        VariantDispatch::Int32Array(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        _ => bail!("Unconvertible value {}", params),
    }?;

    r = match VariantDispatch::from(&results) {
        VariantDispatch::VariantArray(v) => f(v.into_iter().map(|v| Ok(u32::from_variant(&v)?))),
        VariantDispatch::ByteArray(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        VariantDispatch::Int32Array(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        _ => bail!("Unconvertible value {}", results),
    }?;

    Ok(FuncType::new(p, r))
}

// Mark this unsafe for future proofing
pub unsafe fn to_raw(t: ValType, v: Variant) -> Result<ValRaw, Error> {
    Ok(match t {
        ValType::I32 => ValRaw::i32(i32::from_variant(&v)?),
        ValType::I64 => ValRaw::i64(i64::from_variant(&v)?),
        ValType::F32 => ValRaw::f32(f32::from_variant(&v)?.to_bits()),
        ValType::F64 => ValRaw::f64(f64::from_variant(&v)?.to_bits()),
        _ => bail!("Unsupported WASM type conversion {}", t),
    })
}

// Mark this unsafe for future proofing
pub unsafe fn from_raw(t: ValType, v: ValRaw) -> Result<Variant, Error> {
    Ok(match t {
        ValType::I32 => v.get_i32().to_variant(),
        ValType::I64 => v.get_i64().to_variant(),
        ValType::F32 => f32::from_bits(v.get_f32()).to_variant(),
        ValType::F64 => f64::from_bits(v.get_f64()).to_variant(),
        _ => bail!("Unsupported WASM type conversion {}", t),
    })
}

fn wrap_godot_method(
    store: impl AsContextMut<Data = StoreData>,
    ty: FuncType,
    obj: Variant,
    method: GodotString,
) -> Func {
    let ty_cloned = ty.clone();
    let f = move |mut ctx: Caller<StoreData>, args: &mut [ValRaw]| -> Result<(), Error> {
        let pi = ty.params();
        let mut p = Vec::with_capacity(pi.len());
        for (ix, t) in pi.enumerate() {
            p.push(unsafe { from_raw(t, args[ix])? });
        }

        let mut obj = match <Ref<WeakRef, Shared>>::from_variant(&obj) {
            Ok(obj) => unsafe { obj.assume_safe().get_ref() },
            Err(_) => obj.clone(),
        };
        let r = ctx
            .data_mut()
            .release_store(|| unsafe { obj.call(method.clone(), &p) })?;

        let mut ri = ty.results();
        if ri.len() == 0 {
        } else if let Ok(r) = VariantArray::from_variant(&r) {
            for (ix, t) in ri.enumerate() {
                let v = r.get(ix as _);
                args[ix] = unsafe { to_raw(t, v)? };
            }
        } else if ri.len() == 1 {
            args[0] = unsafe { to_raw(ri.next().unwrap(), r)? };
        } else {
            bail!("Unconvertible return value {}", r);
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
    for (k, v) in dict.iter() {
        let k = GodotString::from_variant(&k)?.to_string();

        #[derive(FromVariant)]
        struct Data {
            params: Variant,
            results: Variant,
            object: Variant,
            method: GodotString,
        }

        let data = Data::from_variant(&v)?;
        let obj = match <Ref<WeakRef, Shared>>::from_variant(&data.object) {
            Ok(obj) => unsafe { obj.assume_safe().get_ref() },
            Err(_) => data.object.clone(),
        };
        if !obj.has_method(data.method.clone()) {
            bail!("Object {} has no method {}", obj, data.method);
        }

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
