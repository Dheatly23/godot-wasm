use std::mem;

use anyhow::{bail, Error};
use gdnative::prelude::*;
use lazy_static::lazy_static;
use slab::Slab;
use wasmtime::{Caller, Linker};

use crate::wasm_engine::ENGINE;
use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

pub struct ObjectRegistry {
    slab: Slab<Variant>,
}

impl Default for ObjectRegistry {
    #[inline]
    fn default() -> Self {
        Self { slab: Slab::new() }
    }
}

impl ObjectRegistry {
    #[inline]
    pub fn get(&self, ix: usize) -> Option<Variant> {
        self.slab.get(ix).cloned()
    }

    #[inline]
    pub fn register(&mut self, v: Variant) -> usize {
        if v.is_nil() {
            panic!("Variant cannot be nil!");
        }
        self.slab.insert(v)
    }

    #[inline]
    pub fn unregister(&mut self, ix: usize) -> Option<Variant> {
        self.slab.try_remove(ix)
    }

    #[inline]
    pub fn replace(&mut self, ix: usize, v: Variant) -> Option<Variant> {
        if v.is_nil() {
            panic!("Variant cannot be nil!");
        }
        match self.slab.get_mut(ix).as_mut() {
            Some(p) => Some(mem::replace(p, v)),
            None => None,
        }
    }

    fn get_with_err(&self, ix: usize) -> Result<Variant, Error> {
        match self.get(ix) {
            Some(v) => Ok(v),
            None => bail!("Index {} is null", ix),
        }
    }
}

macro_rules! is_typecheck {
    ($linker:ident, $(($name:literal, $var:ident)),* $(,)?) => {$(
        $linker.func_wrap(
            OBJREGISTRY_MODULE,
            concat!("is_", $name),
            |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                match ctx.data().get_registry()?.get(i as _) {
                    Some(v) if v.get_type() == VariantType::$var => Ok(1),
                    _ => Ok(0),
                }
            }
        ).unwrap();
    )*};
}

lazy_static! {
    pub static ref OBJREGISTRY_LINKER: Linker<StoreData> = {
        let mut linker: Linker<StoreData> = Linker::new(&ENGINE);

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "is_nonnull",
                |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    match ctx.data().get_registry()?.get(i as _) {
                        Some(_) => Ok(1),
                        None => Ok(0),
                    }
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "is_null",
                |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    match ctx.data().get_registry()?.get(i as _) {
                        Some(_) => Ok(0),
                        None => Ok(1),
                    }
                },
            )
            .unwrap();

        is_typecheck!(
            linker,
            ("bool", Bool),
            ("int", I64),
            ("float", F64),
            ("string", GodotString),
            ("vector2", Vector2),
            ("rect2", Rect2),
            ("vector3", Vector3),
            ("transform2d", Transform2D),
            ("plane", Plane),
            ("quat", Quat),
            ("aabb", Aabb),
            ("basis", Basis),
            ("transform", Transform),
            ("color", Color),
            ("nodepath", NodePath),
            ("rid", Rid),
            ("object", Object),
            ("dictionary", Dictionary),
            ("array", VariantArray),
            ("byte_array", ByteArray),
            ("int_array", Int32Array),
            ("float_array", Float32Array),
            ("string_array", StringArray),
            ("vector2_array", Vector2Array),
            ("vector3_array", Vector3Array),
            ("color_array", ColorArray),
        );

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "remove",
                |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    match ctx.data_mut().get_registry_mut()?.unregister(i as _) {
                        Some(_) => Ok(1),
                        None => Ok(0),
                    }
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "duplicate",
                |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = reg.get_with_err(i as _)?;
                    Ok(reg.register(v) as _)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "copy_to",
                |mut ctx: Caller<StoreData>, s: u32, d: u32| -> Result<u32, Error> {
                    let reg = ctx.data_mut().get_registry_mut()?;
                    let v = reg.get_with_err(s as _)?;
                    match reg.replace(d as _, v) {
                        Some(_) => Ok(1),
                        None => Ok(0),
                    }
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "get_int",
                |ctx: Caller<StoreData>, i: u32| -> Result<_, Error> {
                    let v = ctx.data().get_registry()?.get_with_err(i as _)?;
                    Ok(i64::from_variant(&v)?)
                },
            )
            .unwrap();

        linker
            .func_wrap(
                OBJREGISTRY_MODULE,
                "get_float",
                |ctx: Caller<StoreData>, i: u32| -> Result<_, Error> {
                    let v = ctx.data().get_registry()?.get_with_err(i as _)?;
                    Ok(f64::from_variant(&v)?)
                },
            )
            .unwrap();

        linker
    };
}
