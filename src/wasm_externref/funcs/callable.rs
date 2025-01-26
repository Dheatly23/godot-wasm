use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{
    AsContext, AsContextMut, Caller, ExternRef, Func, Rooted, StoreContextMut, TypedFunc,
};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "callable.",
    from_object_method => |mut ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(ctx.as_context(), obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(ctx.as_context(), name)?))?;
        variant_to_externref(ctx.as_context_mut(), Callable::from_object_method(&obj, &name).to_variant())
    },
    invalid => |mut ctx: Caller<'_, _>| -> AnyResult<Option<Rooted<ExternRef>>> {
        variant_to_externref(ctx.as_context_mut(), Callable::invalid().to_variant())
    },
    is_custom => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        Ok(site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?.is_custom() as _)
    },
    is_valid => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        Ok(site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?.is_valid() as _)
    },
    object => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        v.object().map_or(Ok(None), |v| variant_to_externref(ctx.as_context_mut(), v.to_variant()))
    },
    method_name => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        v.method_name().map_or(Ok(None), |v| variant_to_externref(ctx.as_context_mut(), v.to_variant()))
    },
    call => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let c = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(ctx.as_context(), e)?);
                if n == 0 {
                    break;
                }
            }
        }

        let r = ctx.data_mut().as_mut().release_store(move || c.call(&v));
        variant_to_externref(ctx.as_context_mut(), r)
    },
    call_deferred => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<()> {
        let c = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(ctx.as_context(), e)?);
                if n == 0 {
                    break;
                }
            }
        }

        ctx.data_mut().as_mut().release_store(move || c.call_deferred(&v));
        Ok(())
    },
    callv => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>, args: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        let a = site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx.as_context(), args)?))?;

        let r = ctx.data_mut().as_mut().release_store(move || v.callv(&a));
        variant_to_externref(ctx.as_context_mut(), r)
    },
    bindv => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, args: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        let a = site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx.as_context(), args)?))?;

        variant_to_externref(ctx.as_context_mut(), v.bindv(&a).to_variant())
    },
    bind => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let c = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(ctx.as_context(), e)?);
                if n == 0 {
                    break;
                }
            }
        }

        variant_to_externref(ctx.as_context_mut(), c.bind(&v).to_variant())
    },
    unbind => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, n: u64| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        variant_to_externref(ctx.as_context_mut(), v.unbind(n as _).to_variant())
    },
    get_argument_count => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u64> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        Ok(v.get_argument_count() as _)
    },
    get_bound_arguments => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        variant_to_externref(ctx.as_context_mut(), v.get_bound_arguments().to_variant())
    },
    get_bound_arguments_count => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u64> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(ctx.as_context(), v)?))?;
        Ok(v.get_bound_arguments_count() as _)
    },
}
