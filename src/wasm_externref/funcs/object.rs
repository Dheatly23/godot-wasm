use anyhow::Result as AnyResult;
use godot::engine::global::Error as GError;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut, TypedFunc};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "object.",
    from_instance_id => |ctx: Caller<'_, _>, id: i64| -> AnyResult<Option<Rooted<ExternRef>>> {
        let Some(id) = InstanceId::try_from_i64(id) else {
            bail_with_site!("Instance ID is 0")
        };

        variant_to_externref(ctx, site_context!(<Gd<Object>>::try_from_instance_id(id).map_err(|e| e.into_erased()))?.to_variant())
    },
    instance_id => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>| -> AnyResult<i64> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?).map(|o| o.instance_id().to_i64()))
    },
    get_property_list => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?)).and_then(|o| variant_to_externref(ctx, o.get_property_list().to_variant()))
    },
    get_method_list => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?)).and_then(|o| variant_to_externref(ctx, o.get_method_list().to_variant()))
    },
    get_signal_list => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?)).and_then(|o| variant_to_externref(ctx, o.get_signal_list().to_variant()))
    },
    has_method => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        Ok(obj.has_method(name) as _)
    },
    has_signal => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        Ok(obj.has_signal(name) as _)
    },
    call => |mut ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(&ctx, e)?);
                if n == 0 {
                    break;
                }
            }
        }

        variant_to_externref(ctx, site_context!(obj.try_call(name, &v))?)
    },
    call_deferred => |mut ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(&ctx, e)?);
                if n == 0 {
                    break;
                }
            }
        }

        variant_to_externref(ctx, site_context!(obj.try_call_deferred(name, &v))?)
    },
    callv => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, args: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        let args = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, args)?))?;

        variant_to_externref(ctx, obj.callv(name, args))
    },
    get => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;

        variant_to_externref(ctx, obj.get(name))
    },
    set => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        let value = externref_to_variant(&ctx, value)?;

        obj.set(name, value);
        Ok(1)
    },
    set_deferred => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        let value = externref_to_variant(&ctx, value)?;

        obj.set_deferred(name, value);
        Ok(1)
    },
    get_indexed => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, path: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let path = site_context!(from_var_any::<NodePath>(&externref_to_variant(&ctx, path)?))?;

        variant_to_externref(ctx, obj.get_indexed(path))
    },
    set_indexed => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, path: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let path = site_context!(from_var_any::<NodePath>(&externref_to_variant(&ctx, path)?))?;
        let value = externref_to_variant(&ctx, value)?;

        obj.set_indexed(path, value);
        Ok(1)
    },
    connect => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>, flags: u32| -> AnyResult<()> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, signal)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        match obj.connect_ex(signal, target).flags(flags).done() {
            GError::OK => Ok(()),
            e => bail_with_site!("Error: {e:?}"),
        }
    },
    disconnect => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, signal)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        obj.disconnect(signal, target);
        Ok(())
    },
    is_connected => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, signal)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        Ok(obj.is_connected(signal, target) as _)
    },
    emit_signal => |mut ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<()> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(&ctx, e)?);
                if n == 0 {
                    break;
                }
            }
        }

        site_context!(obj.try_emit_signal(name, &v))?;
        Ok(())
    },
}
