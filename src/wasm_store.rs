use std::error::Error;
use std::fmt::{Display, Formatter};
use std::iter::FromIterator;
use std::mem::drop;

use anyhow::Result;
use gdnative::prelude::*;
use hashbrown::HashMap;
use wasmtime::{FuncType, Instance, Linker, Store, Trap, Val, ValRaw, ValType};

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
        _ => panic!("Unconvertible WASM argument type!"),
    }))
    .into_shared()
    .to_variant()
}

/// A Godot method
#[derive(Clone, Debug)]
pub struct GodotMethod {
    pub object: Variant,
    pub method: GodotString,
}

impl Display for GodotMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { object, method } = self;
        write!(f, "{:?}.{}", object, method,)
    }
}

/// Host function map
pub type HostMap = HashMap<GodotString, (GodotMethod, FuncType)>;

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
    fn new(source: impl Error + Send + Sync + 'static, method: &GodotMethod) -> Self {
        Self {
            source: Box::new(source) as Box<_>,
            method: method.clone(),
        }
    }
}

/// Host functionality module name
pub const HOST_MODULE: &str = "host";

/// Register hostmap to linker
pub fn register_hostmap<T, F>(
    store: &Store<T>,
    linker: &mut Linker<T>,
    get_hostmap: F,
) -> Result<()>
where
    F: Fn(&T) -> &HostMap + Send + Sync + Copy + 'static,
{
    unsafe fn set_raw(v: *mut ValRaw, t: ValType, var: Variant) -> Result<(), FromVariantError> {
        match t {
            ValType::I32 => (*v).i32 = i32::from_variant(&var)?,
            ValType::I64 => (*v).i64 = i64::from_variant(&var)?,
            ValType::F32 => (*v).f32 = f32::from_variant(&var)?.to_bits(),
            ValType::F64 => (*v).f64 = f64::from_variant(&var)?.to_bits(),
            _ => unreachable!("Unsupported type"),
        };
        Ok(())
    }

    for (name, (_, ty)) in get_hostmap(store.data()).iter() {
        let name = name.clone();
        unsafe {
            linker.func_new_unchecked("host", &name.to_string(), ty.clone(), move |caller, raw| {
                let data: &HostMap = get_hostmap(caller.data());
                let (method, ty) = data.get(&name).unwrap();

                let params = ty.params();
                let mut input = Vec::with_capacity(params.len());
                for (i, p) in params.enumerate() {
                    let v = raw.add(i);
                    input.push(match p {
                        ValType::I32 => (*v).i32.to_variant(),
                        ValType::I64 => (*v).i64.to_variant(),
                        ValType::F32 => f32::from_bits((*v).f32).to_variant(),
                        ValType::F64 => f64::from_bits((*v).f64).to_variant(),
                        _ => unreachable!("Unsupported type"),
                    });
                }

                let output = method.object.clone().call(method.method.clone(), &input);
                drop(input);

                let ef = |e| Trap::from(Box::new(GodotReturnError::new(e, method)) as Box<_>);
                let output = output
                    .map_err(|e| Trap::from(anyhow::Error::new(e).context(method.clone())))?;

                let mut results = ty.results();
                if results.len() == 0 {
                    return Ok(());
                } else if (results.len() == 1) && VariantArray::from_variant(&output).is_err() {
                    return set_raw(raw, results.next().unwrap(), output).map_err(ef);
                }
                let output = VariantArray::from_variant(&output).map_err(ef)?;
                if (output.len() as usize) < results.len() {
                    return Err(Trap::new("Array too short"));
                }
                for (i, (t, v)) in results.zip(output.iter()).enumerate() {
                    set_raw(raw.add(i), t, v).map_err(ef)?;
                }

                Ok(())
            })
        }?;
    }

    Ok(())
}
