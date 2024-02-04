use std::panic::{catch_unwind, AssertUnwindSafe};

use anyhow::Error;
use godot::engine::global::Error as GError;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut, TypedFunc};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "object.",
    has_method => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<u32, Error> {
        let obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;
        match catch_unwind(AssertUnwindSafe(|| obj.has_method(name))) {
            Ok(v) => Ok(v as _),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    call => |mut ctx: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, f: Option<Func>| -> Result<Option<ExternRef>, Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<ExternRef>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(e));
                if n == 0 {
                    break;
                }
            }
        }

        match catch_unwind(AssertUnwindSafe(|| obj.call(name, &v))) {
            Ok(v) => Ok(variant_to_externref(v)),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    call_deferred => |mut ctx: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, f: Option<Func>| -> Result<Option<ExternRef>, Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<ExternRef>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(e));
                if n == 0 {
                    break;
                }
            }
        }

        match catch_unwind(AssertUnwindSafe(|| obj.call_deferred(name, &v))) {
            Ok(v) => Ok(variant_to_externref(v)),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    get => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;

        match catch_unwind(AssertUnwindSafe(|| obj.get(name))) {
            Ok(v) => Ok(variant_to_externref(v)),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    set => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, value: Option<ExternRef>| -> Result<u32, Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;
        let value = externref_to_variant(value);

        match catch_unwind(AssertUnwindSafe(|| obj.set(name, value))) {
            Ok(_) => Ok(1),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    connect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>, flags: u32| -> Result<(), Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let signal = site_context!(StringName::try_from_variant(&externref_to_variant(signal)))?;
        let target = site_context!(Callable::try_from_variant(&externref_to_variant(target)))?;

        match catch_unwind(AssertUnwindSafe(|| obj.connect_ex(signal, target).flags(flags).done())) {
            Ok(GError::OK) => Ok(()),
            Ok(e) => bail_with_site!("Error: {e:?}"),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    disconnect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>| -> Result<(), Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let signal = site_context!(StringName::try_from_variant(&externref_to_variant(signal)))?;
        let target = site_context!(Callable::try_from_variant(&externref_to_variant(target)))?;

        match catch_unwind(AssertUnwindSafe(|| obj.disconnect(signal, target))) {
            Ok(_) => Ok(()),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
}
