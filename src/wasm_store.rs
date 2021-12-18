use std::error::Error;
use std::fmt::{Display, Formatter};
use std::iter::FromIterator;
use std::mem::drop;

use anyhow::Result;
use gdnative::prelude::*;
use hashbrown::HashMap;
use wasmtime::{ExternRef, FuncType, Instance, Linker, Store, Trap, Val, ValRaw, ValType};

use crate::wasm_externref_godot::{externref_to_variant, variant_to_externref};

macro_rules! unwrap_ext {
    {$v:expr; $e:expr} => {
        match $v {
            Ok(v) => v,
            Err(_) => $e,
        }
    };
    {$v:expr; $e:ident => $ee:expr} => {
        match $v {
            Ok(v) => v,
            Err($e) => $ee,
        }
    };
}

pub fn call_func<T, I>(store: &mut Store<T>, inst: &Instance, name: String, args: I) -> Variant
where
    I: Iterator<Item = Variant>,
{
    let func = match inst.get_func(&mut *store, &name) {
        Some(f) => f,
        None => {
            godot_error!("WASM Function {} does not exist!", name);
            return Variant::new();
        }
    };

    let mut params: Vec<Val>;
    let mut results: Vec<Val>;

    {
        let ty = func.ty(&mut *store);
        let pi = ty.params();
        params = Vec::with_capacity(pi.len());

        let mut args = args;
        for (i, t) in ty.params().enumerate() {
            let a = match args.next() {
                Some(v) => v,
                None => {
                    godot_error!(
                        "Too few arguments! (expected {}, got {})",
                        params.capacity(),
                        i - 1
                    );
                    return Variant::new();
                }
            };
            params.push(match t {
                ValType::I32 => Val::I32(unwrap_ext! {
                    i32::from_variant(&a);
                    {
                        godot_error!("Argument {} type mismatch (expected i32)!", i);
                        return Variant::new();
                    }
                }),
                ValType::I64 => Val::I64(unwrap_ext! {
                    i64::from_variant(&a);
                    {
                        godot_error!("Argument {} type mismatch (expected i64)!", i);
                        return Variant::new();
                    }
                }),
                ValType::F32 => Val::F32(unwrap_ext! {
                    f32::from_variant(&a).map(|v| v.to_bits());
                    {
                        godot_error!("Argument {} type mismatch (expected f32)!", i);
                        return Variant::new();
                    }
                }),
                ValType::F64 => Val::F64(unwrap_ext! {
                    f64::from_variant(&a).map(|v| v.to_bits());
                    {
                        godot_error!("Argument {} type mismatch (expected f64)!", i);
                        return Variant::new();
                    }
                }),
                ValType::ExternRef => Val::ExternRef(variant_to_externref(a)),
                _ => panic!("Unconvertible WASM argument type!"),
            });
        }

        results = ty
            .results()
            .map(|t| match t {
                ValType::I32 => Val::I32(0),
                ValType::I64 => Val::I64(0),
                ValType::F32 => Val::F32(0.0f32.to_bits()),
                ValType::F64 => Val::F64(0.0f64.to_bits()),
                ValType::ExternRef => Val::ExternRef(None),
                _ => panic!("Unconvertible WASM argument type!"),
            })
            .collect();
    }

    unwrap_ext! {
        func.call(&mut *store, &params, &mut results);
        e => {
            godot_error!("Function invocation error: {:?}", e);
            return Variant::new();
        }
    };

    VariantArray::from_iter(results.into_iter().map(|v| match v {
        Val::I32(v) => v.to_variant(),
        Val::I64(v) => v.to_variant(),
        Val::F32(v) => f32::from_bits(v).to_variant(),
        Val::F64(v) => f64::from_bits(v).to_variant(),
        Val::ExternRef(v) => externref_to_variant(v).unwrap_or_else(|e| {
            godot_error!("{}", e);
            Variant::new()
        }),
        _ => panic!("Unconvertible WASM argument type!"),
    }))
    .into_shared()
    .to_variant()
}

/// A Godot method
#[derive(Clone, Debug)]
pub struct GodotMethod {
    pub object: Variant,
    pub method: String,
}

impl Display for GodotMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { object, method } = self;
        write!(f, "{:?}.{}", object, method,)
    }
}

/// Host function map
pub type HostMap = HashMap<String, (GodotMethod, FuncType)>;

#[derive(Debug)]
struct GodotReturnError {
    source: Box<dyn Error + Send + Sync + 'static>,
    method: GodotMethod,
}

impl Display for GodotReturnError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error in return value of Godot method {}", self.method)
    }
}

impl Error for GodotReturnError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source)
    }
}

impl GodotReturnError {
    fn new(source: impl Error + Send + Sync + 'static, method: GodotMethod) -> Self {
        Self {
            source: Box::new(source) as Box<_>,
            method: method,
        }
    }
}

/// Host functionality module name
pub const HOST_MODULE: &str = "host";

/// Register hostmap to linker
pub fn register_hostmap<T>(linker: &mut Linker<T>, hostmap: HostMap) -> Result<()> {
    unsafe fn set_raw(
        ctx: impl wasmtime::AsContextMut,
        v: *mut ValRaw,
        t: ValType,
        var: Variant,
    ) -> Result<(), FromVariantError> {
        match t {
            ValType::I32 => (*v).i32 = i32::from_variant(&var)?,
            ValType::I64 => (*v).i64 = i64::from_variant(&var)?,
            ValType::F32 => (*v).f32 = f32::from_variant(&var)?.to_bits(),
            ValType::F64 => (*v).f64 = f64::from_variant(&var)?.to_bits(),
            ValType::ExternRef => {
                (*v).externref = match variant_to_externref(var) {
                    Some(v) => v.to_raw(ctx),
                    None => 0,
                }
            }
            _ => unreachable!("Unsupported type"),
        };
        Ok(())
    }

    for (name, (method, ty)) in hostmap {
        unsafe {
            linker.func_new_unchecked("host", &name, ty.clone(), move |mut ctx, raw| {
                let params = ty.params();
                let mut input = Vec::with_capacity(params.len());
                for (i, p) in params.enumerate() {
                    let v = raw.add(i);
                    input.push(match p {
                        ValType::I32 => (*v).i32.to_variant(),
                        ValType::I64 => (*v).i64.to_variant(),
                        ValType::F32 => f32::from_bits((*v).f32).to_variant(),
                        ValType::F64 => f64::from_bits((*v).f64).to_variant(),
                        ValType::ExternRef => {
                            externref_to_variant(ExternRef::from_raw((*v).externref))?
                        }
                        _ => unreachable!("Unsupported type"),
                    });
                }

                let output = method
                    .object
                    .clone()
                    .call(&method.method, &input)
                    .map_err(|e| Trap::from(anyhow::Error::new(e).context(method.clone())))?;
                drop(input);
                let ef =
                    |e| Trap::from(Box::new(GodotReturnError::new(e, method.clone())) as Box<_>);

                let mut results = ty.results();
                if results.len() == 0 {
                    return Ok(());
                } else if (results.len() == 1) && VariantArray::from_variant(&output).is_err() {
                    return set_raw(&mut ctx, raw, results.next().unwrap(), output).map_err(ef);
                }
                let output = VariantArray::from_variant(&output).map_err(ef)?;
                if (output.len() as usize) < results.len() {
                    return Err(Trap::new("Array too short"));
                }
                for (i, (t, v)) in results.zip(output.iter()).enumerate() {
                    set_raw(&mut ctx, raw.add(i), t, v).map_err(ef)?;
                }

                Ok(())
            })
        }?;
    }

    Ok(())
}
