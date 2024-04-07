use std::panic::{catch_unwind, AssertUnwindSafe};

use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut, TypedFunc};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "callable.",
    from_object_method => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;
        Ok(variant_to_externref(Callable::from_object_method(&obj, name).to_variant()))
    },
    invalid => |_: Caller<_>| -> Result<Option<ExternRef>, Error> {
        Ok(variant_to_externref(Callable::invalid().to_variant()))
    },
    is_custom => |_: Caller<_>, v: Option<ExternRef>| -> Result<u32, Error> {
        Ok(site_context!(from_var_any::<Callable>(&externref_to_variant(v)))?.is_custom() as _)
    },
    is_valid => |_: Caller<_>, v: Option<ExternRef>| -> Result<u32, Error> {
        Ok(site_context!(from_var_any::<Callable>(&externref_to_variant(v)))?.is_valid() as _)
    },
    object => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(v)))?;
        Ok(variant_to_externref(match v.object() {
            Some(v) => v.to_variant(),
            None => Variant::nil(),
        }))
    },
    method_name => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(v)))?;
        Ok(variant_to_externref(match v.method_name() {
            Some(v) => v.to_variant(),
            None => Variant::nil(),
        }))
    },
    call => |mut ctx: Caller<_>, v: Option<ExternRef>, f: Option<Func>| -> Result<Option<ExternRef>, Error> {
        let c = site_context!(from_var_any::<Callable>(&externref_to_variant(v)))?;

        let mut v = <Array<Variant>>::new();
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

        match catch_unwind(AssertUnwindSafe(|| c.callv(v))) {
            Ok(v) => Ok(variant_to_externref(v)),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    callv => |_: Caller<_>, v: Option<ExternRef>, args: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(v)))?;
        let a = site_context!(from_var_any::<VariantArray>(&externref_to_variant(args)))?;

        match catch_unwind(AssertUnwindSafe(|| v.callv(a))) {
            Ok(v) => Ok(variant_to_externref(v)),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    bindv => |_: Caller<_>, v: Option<ExternRef>, args: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(v)))?;
        let a = site_context!(from_var_any::<VariantArray>(&externref_to_variant(args)))?;

        Ok(variant_to_externref(v.bindv(a).to_variant()))
    },
}
