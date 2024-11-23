use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut, TypedFunc};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "callable.",
    from_object_method => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        variant_to_externref(ctx, Callable::from_object_method(&obj, &name).to_variant())
    },
    invalid => |ctx: Caller<'_, _>| -> AnyResult<Option<Rooted<ExternRef>>> {
        variant_to_externref(ctx, Callable::invalid().to_variant())
    },
    is_custom => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        Ok(site_context!(from_var_any::<Callable>(&externref_to_variant(ctx, v)?))?.is_custom() as _)
    },
    is_valid => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        Ok(site_context!(from_var_any::<Callable>(&externref_to_variant(ctx, v)?))?.is_valid() as _)
    },
    object => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, v)?))?;
        v.object().map_or(Ok(None), |v| variant_to_externref(ctx, v.to_variant()))
    },
    method_name => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, v)?))?;
        v.method_name().map_or(Ok(None), |v| variant_to_externref(ctx, v.to_variant()))
    },
    call => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let c = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, v)?))?;

        let mut v = ctx.data_mut().as_mut().get_arg_arr().clone();
        v.clear();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(&externref_to_variant(&ctx, e)?);
                if n == 0 {
                    break;
                }
            }
        }

        let r = ctx.data_mut().as_mut().release_store(move || c.callv(&v));
        variant_to_externref(ctx, r)
    },
    callv => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>, args: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, v)?))?;
        let a = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, args)?))?;

        let r = ctx.data_mut().as_mut().release_store(move || v.callv(&a));
        variant_to_externref(ctx, r)
    },
    bindv => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, args: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, v)?))?;
        let a = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, args)?))?;

        variant_to_externref(ctx, v.bindv(&a).to_variant())
    },
}
