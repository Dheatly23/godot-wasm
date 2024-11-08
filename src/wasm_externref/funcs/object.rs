use anyhow::Result as AnyResult;
use godot::global::Error as GError;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut, TypedFunc};

use crate::godot_util::{from_var_any, ErrorWrapper};
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
        site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(ctx, obj)?).map(|o| o.instance_id().to_i64()))
    },
    get_property_list => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let r = ctx.data_mut().as_mut().release_store(move || obj.get_property_list());
        variant_to_externref(ctx, r.to_variant())
    },
    get_method_list => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let r = ctx.data_mut().as_mut().release_store(move || obj.get_method_list());
        variant_to_externref(ctx, r.to_variant())
    },
    get_signal_list => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let r = ctx.data_mut().as_mut().release_store(move || obj.get_signal_list());
        variant_to_externref(ctx, r.to_variant())
    },
    has_method => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        ctx.data_mut().as_mut().release_store(move || Ok(obj.has_method(&name) as _))
    },
    has_signal => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        ctx.data_mut().as_mut().release_store(move || Ok(obj.has_signal(&name) as _))
    },
    call => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
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

        let r = ctx.data_mut().as_mut().release_store(move || site_context!(obj.try_call(&name, &v)))?;
        variant_to_externref(ctx, r)
    },
    call_deferred => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
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

        let r = ctx.data_mut().as_mut().release_store(move || site_context!(obj.try_call_deferred(&name, &v)))?;
        variant_to_externref(ctx, r)
    },
    callv => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, args: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        let args = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, args)?))?;

        let r = ctx.data_mut().as_mut().release_store(move || obj.callv(&name, &args));
        variant_to_externref(ctx, r)
    },
    get => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;

        let r = ctx.data_mut().as_mut().release_store(move || obj.get(&name));
        variant_to_externref(ctx, r)
    },
    set => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        let value = externref_to_variant(&ctx, value)?;

        ctx.data_mut().as_mut().release_store(move || obj.set(&name, &value));
        Ok(1)
    },
    set_deferred => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        let value = externref_to_variant(&ctx, value)?;

        ctx.data_mut().as_mut().release_store(move || obj.set_deferred(&name, &value));
        Ok(1)
    },
    get_indexed => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, path: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let path = site_context!(from_var_any::<NodePath>(&externref_to_variant(&ctx, path)?))?;

        let r = ctx.data_mut().as_mut().release_store(move || obj.get_indexed(&path));
        variant_to_externref(ctx, r)
    },
    set_indexed => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, path: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let path = site_context!(from_var_any::<NodePath>(&externref_to_variant(&ctx, path)?))?;
        let value = externref_to_variant(&ctx, value)?;

        ctx.data_mut().as_mut().release_store(move || obj.set_indexed(&path, &value));
        Ok(1)
    },
    connect => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>, flags: u32| -> AnyResult<()> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, signal)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        match obj.connect_ex(&signal, &target).flags(flags).done() {
            GError::OK => Ok(()),
            e => Err(ErrorWrapper::from(e).into()),
        }
    },
    disconnect => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, signal)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        obj.disconnect(&signal, &target);
        Ok(())
    },
    is_connected => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let signal = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, signal)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        Ok(obj.is_connected(&signal, &target) as _)
    },
    emit_signal => |mut ctx: Caller<'_, T>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<()> {
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

        site_context!(match ctx.data_mut().as_mut().release_store(move || site_context!(obj.try_emit_signal(&name, &v)))? {
            GError::OK => Ok(()),
            e => Err(ErrorWrapper::from(e)),
        })
    },
}
