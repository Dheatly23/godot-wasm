use anyhow::Error;
use wasmtime::{Caller, Linker};

use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "delete",
            |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                match ctx.data_mut().get_registry_mut()?.unregister(i as _) {
                    Some(_) => Ok(1),
                    None => Ok(0),
                }
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "duplicate",
            |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let v = reg.get_or_nil(i as _);
                Ok(reg.register(v) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "copy",
            |mut ctx: Caller<StoreData>, s: u32, d: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let v = reg.get_or_nil(s as _);
                match reg.replace(d as _, v) {
                    Some(_) => Ok(1),
                    None => Ok(0),
                }
            },
        )
        .unwrap();
}
