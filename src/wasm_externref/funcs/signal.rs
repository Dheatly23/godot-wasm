use std::panic::{catch_unwind, AssertUnwindSafe};

use anyhow::Result as AnyResult;
use godot::engine::global::Error as GError;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut, TypedFunc};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "signal.",
    from_object_signal => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let obj = site_context!(from_var_any::<Gd<Object>>(&externref_to_variant(&ctx, obj)?))?;
        let name = site_context!(from_var_any::<StringName>(&externref_to_variant(&ctx, name)?))?;
        variant_to_externref(ctx, Signal::from_object_signal(&obj, name).to_variant())
    },
    object => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        match v.object() {
            Some(v) => variant_to_externref(ctx, v.to_variant()),
            None => Ok(None),
        }
    },
    name => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        variant_to_externref(ctx, v.name().to_variant())
    },
    connect => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>, flags: i64| -> AnyResult<()> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        match catch_unwind(AssertUnwindSafe(|| v.connect(target, flags))) {
            Ok(GError::OK) => Ok(()),
            Ok(e) => bail_with_site!("Error: {e:?}"),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    disconnect => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        match catch_unwind(AssertUnwindSafe(|| v.disconnect(target))) {
            Ok(_) => Ok(()),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    is_connected => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;
        let target = site_context!(from_var_any::<Callable>(&externref_to_variant(&ctx, target)?))?;

        match catch_unwind(AssertUnwindSafe(|| v.is_connected(target))) {
            Ok(v) => Ok(v as _),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    emit => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<()> {
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

        match catch_unwind(AssertUnwindSafe(|| c.emit(&v))) {
            Ok(_) => Ok(()),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
    connections => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<Signal>(&externref_to_variant(&ctx, v)?))?;

        match catch_unwind(AssertUnwindSafe(|| v.connections())) {
            Ok(v) => variant_to_externref(ctx, v.to_variant()),
            Err(_) => bail_with_site!("Error binding object"),
        }
    },
}
