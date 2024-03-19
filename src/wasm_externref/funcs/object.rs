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
        Ok(obj.has_method(name) as _)
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

        site_context!(obj.try_call(name, &v).map(variant_to_externref))
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

        site_context!(obj.try_call_deferred(name, &v).map(variant_to_externref))
    },
    callv => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, args: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;
        let args = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(args)))?;

        Ok(variant_to_externref(obj.callv(name, args)))
    },
    get => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;

        Ok(variant_to_externref(obj.get(name)))
    },
    set => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, value: Option<ExternRef>| -> Result<u32, Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let name = site_context!(StringName::try_from_variant(&externref_to_variant(name)))?;
        let value = externref_to_variant(value);

        obj.set(name, value);
        Ok(1)
    },
    connect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>, flags: u32| -> Result<(), Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let signal = site_context!(StringName::try_from_variant(&externref_to_variant(signal)))?;
        let target = site_context!(Callable::try_from_variant(&externref_to_variant(target)))?;

        match obj.connect_ex(signal, target).flags(flags).done() {
            GError::OK => Ok(()),
            e => bail_with_site!("Error: {e:?}"),
        }
    },
    disconnect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>| -> Result<(), Error> {
        let mut obj = site_context!(<Gd<Object>>::try_from_variant(&externref_to_variant(obj)))?;
        let signal = site_context!(StringName::try_from_variant(&externref_to_variant(signal)))?;
        let target = site_context!(Callable::try_from_variant(&externref_to_variant(target)))?;

        obj.disconnect(signal, target);
        Ok(())
    },
    emit_signal => |mut ctx: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, f: Option<Func>| -> Result<(), Error> {
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

        site_context!(obj.try_emit_signal(name, &v))?;
        Ok(())
    },
}
