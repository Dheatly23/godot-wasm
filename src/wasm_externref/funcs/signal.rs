use std::panic::{catch_unwind, AssertUnwindSafe};

use anyhow::Error;
use godot::engine::global::Error as GError;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut, TypedFunc};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "signal.",
    from_object_signal => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;
        Ok(variant_to_externref(Signal::from_object_signal(&obj, name).to_variant()))
    },
    object => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(Signal::try_from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(match v.object() {
            Some(v) => v.to_variant(),
            None => Variant::nil(),
        }))
    },
    name => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(Signal::try_from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(v.name().to_variant()))
    },
    connect => |_: Caller<_>, v: Option<ExternRef>, target: Option<ExternRef>, flags: i64| -> Result<(), Error> {
        let v = site_context!(Signal::try_from_variant(&externref_to_variant(v)))?;
        let target = site_context!(Callable::try_from_variant(&externref_to_variant(target)))?;

        match catch_unwind(AssertUnwindSafe(|| v.connect(target, flags))) {
            Ok(GError::OK) => Ok(()),
            Ok(e) => bail_with_site!("Error: {e:?}"),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    disconnect => |_: Caller<_>, v: Option<ExternRef>, target: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(Signal::try_from_variant(&externref_to_variant(v)))?;
        let target = site_context!(Callable::try_from_variant(&externref_to_variant(target)))?;

        match catch_unwind(AssertUnwindSafe(|| v.disconnect(target))) {
            Ok(_) => Ok(()),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    is_connected => |_: Caller<_>, v: Option<ExternRef>, target: Option<ExternRef>| -> Result<u32, Error> {
        let v = site_context!(Signal::try_from_variant(&externref_to_variant(v)))?;
        let target = site_context!(Callable::try_from_variant(&externref_to_variant(target)))?;

        match catch_unwind(AssertUnwindSafe(|| v.is_connected(target))) {
            Ok(v) => Ok(v as _),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    emit => |mut ctx: Caller<_>, v: Option<ExternRef>, f: Option<Func>| -> Result<(), Error> {
        let c = site_context!(Signal::try_from_variant(&externref_to_variant(v)))?;

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

        match catch_unwind(AssertUnwindSafe(|| c.emit(&v))) {
            Ok(_) => Ok(()),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    connections => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(Signal::try_from_variant(&externref_to_variant(v)))?;

        match catch_unwind(AssertUnwindSafe(|| v.connections())) {
            Ok(v) => Ok(variant_to_externref(v.to_variant())),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
}
