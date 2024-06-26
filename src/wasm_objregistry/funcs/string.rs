use std::io::Write;
use std::str::from_utf8;

use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::godot_util::from_var_any;
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "string.",
    len => |ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
        let v = site_context!(from_var_any::<GString>(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;

        Ok(v.chars().iter().map(|c| c.len_utf8()).sum::<usize>() as _)
    },
    read => |mut ctx: Caller<'_, T>, i: u32, p: u32| -> Result<u32, Error> {
        let v = site_context!(from_var_any::<GString>(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        match mem.data_mut(&mut ctx).get_mut(p as _..) {
            Some(mut s) => site_context!(write!(&mut s, "{}", v))?,
            None => bail_with_site!("Invalid memory bounds ({}..)", p),
        };
        Ok(1)
    },
    write => |mut ctx: Caller<'_, T>, i: u32, p: u32, n: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let v = match mem.data(&mut ctx).get(p as _..(p + n) as _) {
            Some(s) => site_context!(from_utf8(s))?.to_variant(),
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n),
        };
        ctx.data_mut().as_mut().get_registry_mut()?.replace(i as _, v);
        Ok(1)
    },
    write_new => |mut ctx: Caller<'_, T>, p: u32, n: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let v = match mem.data(&mut ctx).get(p as _..(p + n) as _) {
            Some(s) => site_context!(from_utf8(s))?.to_variant(),
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n),
        };
        Ok(ctx.data_mut().as_mut().get_registry_mut()?.register(v) as _)
    },
    to_string_name => |mut ctx: Caller<'_, T>, i: u32| -> Result<(), Error> {
        let v = site_context!(from_var_any::<GString>(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;
        ctx.data_mut().as_mut().get_registry_mut()?.replace(i as _, StringName::from(v).to_variant());
        Ok(())
    },
    from_string_name => |mut ctx: Caller<'_, T>, i: u32| -> Result<(), Error> {
        let v = site_context!(from_var_any::<StringName>(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;
        ctx.data_mut().as_mut().get_registry_mut()?.replace(i as _, GString::from(v).to_variant());
        Ok(())
    },
}
