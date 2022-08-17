use anyhow::{bail, Error};
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

        let r = unsafe { wrap_err(this.obj.clone().call(this.method.clone(), &p))? };

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
            params: VariantArray,
            results: VariantArray,
            object: Variant,
            method: GodotString,
        }

        let data = Data::from_variant(&v)?;
        if !data.object.has_method(GodotString::clone(&data.method)) {
            bail!("Object {} has no method {}", data.object, data.method);
        }

        let mut params = Vec::with_capacity(data.params.len() as _);
        let mut results = Vec::with_capacity(data.results.len() as _);

        for v in &data.params {
            params.push(match u32::from_variant(&v)? {
                TYPE_I32 => Type::I32,
                TYPE_I64 => Type::I64,
                TYPE_F32 => Type::F32,
                TYPE_F64 => Type::F64,
                v => bail!("Unknown enumeration value {}", v),
            });
        }

        for v in &data.results {
            results.push(match u32::from_variant(&v)? {
                TYPE_I32 => Type::I32,
                TYPE_I64 => Type::I64,
                TYPE_F32 => Type::F32,
                TYPE_F64 => Type::F64,
                v => bail!("Unknown enumeration value {}", v),
            });
        }

        ret.insert(
            k,
            wrap_godot_method(FunctionType::new(params, results), data.object, data.method),
        );
    }

    Ok(ret)
}
