use anyhow::{bail, Error};
use gdnative::api::WeakRef;
use gdnative::prelude::*;
use wasmer::{Exports, Function, FunctionType, RuntimeError, Type, Val, WasmerEnv};

use crate::wasm_engine::ENGINE;

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;
pub const TYPE_VARIANT: u32 = 6;

pub const HOST_MODULE: &str = "host";

pub const MODULE_INCLUDES: &[&str] = &[HOST_MODULE];

pub const MEMORY_EXPORT: &str = "memory";

#[doc(hidden)]
#[macro_export]
macro_rules! variant_typecast {
    (($e:expr) { $($i:ident : $t:ty => $v:expr ,)+ _ @ $ei:ident => $else:expr $(,)? }) => {{
        let $ei = $e;
        $(
            if let Ok($i) = <$t>::from_variant(&$ei) {
                $v
            } else
        )+ {
            $else
        }
    }};
    (($e:expr) { $($i:ident : $t:ty => $v:expr ,)+ _ => $else:expr $(,)? }) => {{
        let r#__variant = $e;
        $(
            if let Ok($i) = <$t>::from_variant(&r#__variant) {
                $v
            } else
        )+ {
            $else
        }
    }};
}

pub fn from_signature(sig: &FunctionType) -> Result<(ByteArray, ByteArray), Error> {
    let p = sig.params().iter();
    let r = sig.results().iter();

    let mut pr = ByteArray::new();
    let mut rr = ByteArray::new();

    pr.resize(p.len() as _);
    rr.resize(r.len() as _);

    for (s, d) in p
        .zip(pr.write().iter_mut())
        .chain(r.zip(rr.write().iter_mut()))
    {
        *d = match s {
            Type::I32 => TYPE_I32,
            Type::I64 => TYPE_I64,
            Type::F32 => TYPE_F32,
            Type::F64 => TYPE_F64,
            _ => bail!("Unconvertible signture"),
        } as _;
    }

    Ok((pr, rr))
}

pub fn to_signature(params: Variant, results: Variant) -> Result<FunctionType, Error> {
    let p;
    let r;

    fn f(it: impl Iterator<Item = Result<u32, Error>>) -> Result<Vec<Type>, Error> {
        let mut ret = match it.size_hint() {
            (_, Some(n)) => Vec::with_capacity(n),
            (n, None) => Vec::with_capacity(n),
        };

        for i in it {
            ret.push(match i? {
                TYPE_I32 => Type::I32,
                TYPE_I64 => Type::I64,
                TYPE_F32 => Type::F32,
                TYPE_F64 => Type::F64,
                v => bail!("Unknown enumeration value {}", v),
            });
        }

        Ok(ret)
    }

    p = variant_typecast!((params) {
        v: VariantArray => f(v.into_iter().map(|v| Ok(u32::from_variant(&v)?)))?,
        v: ByteArray => f(v.read().as_slice().iter().map(|v| Ok(*v as u32)))?,
        v: Int32Array => f(v.read().as_slice().iter().map(|v| Ok(*v as u32)))?,
        v: Float32Array => f(v.read().as_slice().iter().map(|v| Ok(*v as u32)))?,
        _ @ params => bail!("Unconvertible value {}", params),
    });

    r = variant_typecast!((results) {
        v: VariantArray => f(v.into_iter().map(|v| Ok(u32::from_variant(&v)?)))?,
        v: ByteArray => f(v.read().as_slice().iter().map(|v| Ok(*v as u32)))?,
        v: Int32Array => f(v.read().as_slice().iter().map(|v| Ok(*v as u32)))?,
        v: Float32Array => f(v.read().as_slice().iter().map(|v| Ok(*v as u32)))?,
        _ @ results => bail!("Unconvertible value {}", results),
    });

    Ok(FunctionType::new(p, r))
}

#[derive(WasmerEnv, Clone)]
struct GodotMethodEnv {
    ty: FunctionType,
    obj: Variant,
    method: GodotString,
}

impl GodotMethodEnv {
    fn call_method(this: &Self, args: &[Val]) -> Result<Vec<Val>, RuntimeError> {
        fn wrap_err<T, E: std::error::Error + Sync + Send + 'static>(
            v: Result<T, E>,
        ) -> Result<T, RuntimeError> {
            match v {
                Ok(v) => Ok(v),
                Err(e) => Err(RuntimeError::user(Box::new(e))),
            }
        }

        let mut p = Vec::with_capacity(args.len());
        for v in args {
            p.push(match v {
                Val::I32(v) => v.to_variant(),
                Val::I64(v) => v.to_variant(),
                Val::F32(v) => v.to_variant(),
                Val::F64(v) => v.to_variant(),
                _ => {
                    return Err(RuntimeError::new(format!(
                        "Cannot format WASM value {:?}",
                        v
                    )))
                }
            });
        }

        let mut obj = match <Ref<WeakRef, Shared>>::from_variant(&this.obj) {
            Ok(obj) => unsafe { obj.assume_safe().get_ref() },
            Err(_) => this.obj.clone(),
        };
        let r = unsafe { wrap_err(obj.call(this.method.clone(), &p))? };

        let results = this.ty.results();
        let mut ret = Vec::with_capacity(results.len());
        if results.len() == 0 {
        } else if let Ok(r) = VariantArray::from_variant(&r) {
            for (ix, t) in results.iter().enumerate() {
                let v = r.get(ix as _);
                ret.push(match t {
                    Type::I32 => Val::I32(wrap_err(i32::from_variant(&v))?),
                    Type::I64 => Val::I64(wrap_err(i64::from_variant(&v))?),
                    Type::F32 => Val::F32(wrap_err(f32::from_variant(&v))?),
                    Type::F64 => Val::F64(wrap_err(f64::from_variant(&v))?),
                    _ => {
                        return Err(RuntimeError::new(format!(
                            "Unsupported WASM type conversion {}",
                            t
                        )))
                    }
                });
            }
        } else if results.len() == 1 {
            ret.push(match results[0] {
                Type::I32 => Val::I32(wrap_err(i32::from_variant(&r))?),
                Type::I64 => Val::I64(wrap_err(i64::from_variant(&r))?),
                Type::F32 => Val::F32(wrap_err(f32::from_variant(&r))?),
                Type::F64 => Val::F64(wrap_err(f64::from_variant(&r))?),
                t => {
                    return Err(RuntimeError::new(format!(
                        "Unsupported WASM type conversion {}",
                        t
                    )))
                }
            });
        } else {
            return Err(RuntimeError::new(format!(
                "Unconvertible return value {}",
                r
            )));
        }

        Ok(ret)
    }
}

pub fn wrap_godot_method(ty: FunctionType, obj: Variant, method: GodotString) -> Function {
    Function::new_with_env(
        &ENGINE,
        FunctionType::clone(&ty),
        GodotMethodEnv { ty, obj, method },
        GodotMethodEnv::call_method,
    )
}

pub fn make_host_module(dict: Dictionary) -> Result<Exports, Error> {
    let mut ret = Exports::new();
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
        if !obj.has_method(GodotString::clone(&data.method)) {
            bail!("Object {} has no method {}", obj, data.method);
        }

        ret.insert(
            k,
            wrap_godot_method(
                to_signature(data.params, data.results)?,
                data.object,
                data.method,
            ),
        );
    }

    Ok(ret)
}
