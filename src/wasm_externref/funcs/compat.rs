use anyhow::Error;
use wasmtime::{Caller, ExternRef, Linker};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::wasm_util::EXTERNREF_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "compat.register",
            |mut ctx: Caller<StoreData>, v: Option<ExternRef>| -> Result<u32, Error> {
                Ok(ctx
                    .data_mut()
                    .get_registry_mut()?
                    .register(externref_to_variant(v)) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "compat.get",
            |ctx: Caller<StoreData>, i: u32| -> Result<Option<ExternRef>, Error> {
                Ok(variant_to_externref(
                    ctx.data().get_registry()?.get_or_nil(i as _),
                ))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "compat.set",
            |mut ctx: Caller<StoreData>, i: u32, v: Option<ExternRef>| -> Result<(), Error> {
                ctx.data_mut()
                    .get_registry_mut()?
                    .replace(i as _, externref_to_variant(v));
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "compat.unregister",
            |mut ctx: Caller<StoreData>, i: u32| -> Result<(), Error> {
                ctx.data_mut().get_registry_mut()?.unregister(i as _);
                Ok(())
            },
        )
        .unwrap();
}
