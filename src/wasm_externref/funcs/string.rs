use std::io::Write;
use std::str::from_utf8;

use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, ExternRef, Linker};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::wasm_util::EXTERNREF_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "string.len",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<u32, Error> {
                let v = GodotString::from_variant(&externref_to_variant(v))?;

                // NOTE: Please fix this as soon as godot_rust opens up it's byte slice API.
                Ok(v.to_string().as_bytes().len() as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "string.read",
            |mut ctx: Caller<StoreData>, v: Option<ExternRef>, p: u32| -> Result<u32, Error> {
                let v = GodotString::from_variant(&externref_to_variant(v))?;
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
            EXTERNREF_MODULE,
            "string.write",
            |mut ctx: Caller<StoreData>, p: u32, n: u32| -> Result<Option<ExternRef>, Error> {
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(None),
                };

                let v = from_utf8(&mem.data(&mut ctx)[p as _..(p + n) as _])?;
                Ok(variant_to_externref(v.to_variant()))
            },
        )
        .unwrap();
}
