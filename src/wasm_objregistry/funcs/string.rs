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
                Ok(v.len() as _)
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

                mem.write(&mut ctx, p as _, v.to_string().as_bytes())?;
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

                let mut v = vec![0u8; n as usize];
                mem.read(&mut ctx, p as _, &mut v)?;
                let v = String::from_utf8(v)?;
                ctx.data_mut()
                    .get_registry_mut()?
                    .replace(i as _, v.to_variant());
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

                let mut v = vec![0u8; n as usize];
                mem.read(&mut ctx, p as _, &mut v)?;
                let v = String::from_utf8(v)?;
                Ok(ctx.data_mut().get_registry_mut()?.register(v.to_variant()) as _)
            },
        )
        .unwrap();
}
