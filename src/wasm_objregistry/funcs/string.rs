use std::io::Write;
use std::str::from_utf8;

use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, Linker};

use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string.len",
            |ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                let v = GodotString::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;

                // NOTE: Please fix this as soon as godot_rust opens up it's byte slice API.
                Ok(v.to_string().as_bytes().len() as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string.read",
            |mut ctx: Caller<StoreData>, i: u32, p: u32| -> Result<u32, Error> {
                let v = GodotString::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                write!(&mut mem.data_mut(&mut ctx)[p as _..], "{}", v)?;
                Ok(1)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string.write",
            |mut ctx: Caller<StoreData>, i: u32, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let v = from_utf8(&mem.data(&mut ctx)[p as _..(p + n) as _])?.to_variant();
                ctx.data_mut().get_registry_mut()?.replace(i as _, v);
                Ok(1)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "string.write_new",
            |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<u32, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };

                let v = from_utf8(&mem.data(&mut ctx)[p as _..(p + n) as _])?.to_variant();
                Ok(ctx.data_mut().get_registry_mut()?.register(v) as _)
            },
        )
        .unwrap();
}
