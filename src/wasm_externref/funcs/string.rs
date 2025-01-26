use std::io::Write;
use std::str::from_utf8;

use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{AsContext, AsContextMut, Caller, Extern, ExternRef, Func, Rooted, StoreContextMut};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "string.",
    len => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<GString>(&externref_to_variant(ctx.as_context(), v)?))?;

        Ok(v.chars().iter().map(|c| c.len_utf8()).sum::<usize>() as _)
    },
    read => |mut ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, p: u32| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<GString>(&externref_to_variant(ctx.as_context(), v)?))?;
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        match mem.data_mut(&mut ctx).get_mut(p as _..) {
            Some(mut s) => site_context!(write!(&mut s, "{}", v))?,
            None => bail_with_site!("Invalid memory range ({}..)", p),
        };
        Ok(1)
    },
    write => |mut ctx: Caller<'_, _>, p: u32, n: u32| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(None),
        };

        let v = match mem.data(&mut ctx).get(p as _..(p + n) as _) {
            Some(s) => site_context!(from_utf8(s))?.to_variant(),
            None => bail_with_site!("Invalid memory range ({}..{})", p, p + n),
        };
        variant_to_externref(ctx.as_context_mut(), v)
    },
    to_string_name => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<GString>(&externref_to_variant(ctx.as_context(), v)?))?;
        variant_to_externref(ctx.as_context_mut(), StringName::from(v).to_variant())
    },
    from_string_name => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<StringName>(&externref_to_variant(ctx.as_context(), v)?))?;
        variant_to_externref(ctx.as_context_mut(), GString::from(v).to_variant())
    },
}
