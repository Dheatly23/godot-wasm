use anyhow::Error;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut};

use crate::func_registry;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;

func_registry! {
    "compat.",
    register => |mut ctx: Caller<T>, v: Option<ExternRef>| -> Result<u32, Error> {
        Ok(ctx.data_mut()
            .as_mut()
            .get_registry_mut()?
            .register(externref_to_variant(v)) as _)
    },
    get => |ctx: Caller<T>, i: u32| -> Result<Option<ExternRef>, Error> {
        Ok(variant_to_externref(
            ctx.data().as_ref().get_registry()?.get_or_nil(i as _),
        ))
    },
    set => |mut ctx: Caller<T>, i: u32, v: Option<ExternRef>| -> Result<(), Error> {
        ctx.data_mut()
            .as_mut()
            .get_registry_mut()?
            .replace(i as _, externref_to_variant(v));
        Ok(())
    },
    unregister => |mut ctx: Caller<T>, i: u32| -> Result<(), Error> {
        ctx.data_mut().as_mut().get_registry_mut()?.unregister(i as _);
        Ok(())
    },
}
