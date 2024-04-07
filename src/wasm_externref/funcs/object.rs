use anyhow::Error;
use godot::engine::global::Error as GError;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut, TypedFunc};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "object.",
    from_instance_id => |_: Caller<_>, id: i64| -> Result<Option<ExternRef>, Error> {
        let Some(id) = InstanceId::try_from_i64(id) else {
            bail_with_site!("Instance ID is 0")
        };

        site_context!(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased())).map(|o| variant_to_externref(o.to_variant()))
    },
    instance_id => |_: Caller<_>, obj: Option<ExternRef>| -> Result<i64, Error> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)).map(|o| o.instance_id().to_i64()))
    },
    get_property_list => |_: Caller<_>, obj: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)).map(|o| variant_to_externref(o.get_property_list().to_variant())))
    },
    get_method_list => |_: Caller<_>, obj: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)).map(|o| variant_to_externref(o.get_method_list().to_variant())))
    },
    get_signal_list => |_: Caller<_>, obj: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)).map(|o| variant_to_externref(o.get_signal_list().to_variant())))
    },
    has_method => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<u32, Error> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;
        Ok(obj.has_method(name) as _)
    },
    has_signal => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<u32, Error> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;
        Ok(obj.has_signal(name) as _)
    },
    call => |mut ctx: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, f: Option<Func>| -> Result<Option<ExternRef>, Error> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;

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
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;

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
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;
        let args = site_context!(from_var_any::<VariantArray>(&externref_to_variant(args)))?;

        Ok(variant_to_externref(obj.callv(name, args)))
    },
    get => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;

        Ok(variant_to_externref(obj.get(name)))
    },
    set => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, value: Option<ExternRef>| -> Result<u32, Error> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;
        let value = externref_to_variant(value);

        obj.set(name, value);
        Ok(1)
    },
    set_deferred => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, value: Option<ExternRef>| -> Result<u32, Error> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;
        let value = externref_to_variant(value);

        obj.set_deferred(name, value);
        Ok(1)
    },
    get_indexed => |_: Caller<_>, obj: Option<ExternRef>, path: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let path = site_context!(from_var_any::<NodePath>(&externref_to_variant(path)))?;

        Ok(variant_to_externref(obj.get_indexed(path)))
    },
    set_indexed => |_: Caller<_>, obj: Option<ExternRef>, path: Option<ExternRef>, value: Option<ExternRef>| -> Result<u32, Error> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let path = site_context!(from_var_any::<NodePath>(&externref_to_variant(path)))?;
        let value = externref_to_variant(value);

        obj.set_indexed(path, value);
        Ok(1)
    },
    connect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>, flags: u32| -> Result<(), Error> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(signal)))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(target)))?;

        match obj.connect_ex(signal, target).flags(flags).done() {
            GError::OK => Ok(()),
            e => bail_with_site!("Error: {e:?}"),
        }
    },
    disconnect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>| -> Result<(), Error> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(signal)))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(target)))?;

        obj.disconnect(signal, target);
        Ok(())
    },
    is_connected => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>| -> Result<u32, Error> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(signal)))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(target)))?;

        Ok(obj.is_connected(signal, target) as _)
    },
    emit_signal => |mut ctx: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, f: Option<Func>| -> Result<(), Error> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(obj)))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(name)))?;

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
