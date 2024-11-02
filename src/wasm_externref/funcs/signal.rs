use anyhow::Result as AnyResult;
use godot::global::Error as GError;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut, TypedFunc};

use crate::godot_util::{from_var_any, ErrorWrapper};
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "signal.",
    from_object_signal => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        variant_to_externref(ctx, Signal::from_object_signal(&obj, name).to_variant())
    },
    object => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        v.object().map_or(Ok(None), |v| variant_to_externref(ctx, v.to_variant()))
    },
    name => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        variant_to_externref(ctx, v.name().to_variant())
    },
    connect => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>, flags: i64| -> AnyResult<()> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        site_context!(match v.connect(target, flags) {
            GError::OK => Ok(()),
            e => Err(ErrorWrapper::from(e)),
        })
    },
    disconnect => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        v.disconnect(target);
        Ok(())
    },
    is_connected => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        Ok(v.is_connected(target) as _)
    },
    emit => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<()> {
        let c = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;

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

        ctx.data_mut().as_mut().release_store(move || c.emit(&v));
        Ok(())
    },
    connections => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;

        variant_to_externref(ctx, v.connections().to_variant())
    },
}
