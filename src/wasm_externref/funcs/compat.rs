use anyhow::Result as AnyResult;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut};

use crate::func_registry;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;

func_registry! {
    "compat.",
    register => |mut ctx: Caller<'_, T>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = externref_to_variant(&ctx, v)?;
        Ok(ctx.data_mut()
            .as_mut()
            .get_registry_mut()?
            .register(v) as _)
    },
    get => |ctx: Caller<'_, T>, i: u32| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = ctx.data().as_ref().get_registry()?.get_or_nil(i as _);
        variant_to_externref(ctx, v)
    },
    set => |mut ctx: Caller<'_, T>, i: u32, v: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = externref_to_variant(&ctx, v)?;
        ctx.data_mut()
            .as_mut()
            .get_registry_mut()?
            .replace(i as _, v);
        Ok(())
    },
    unregister => |mut ctx: Caller<'_, T>, i: u32| -> AnyResult<()> {
        ctx.data_mut().as_mut().get_registry_mut()?.unregister(i as _);
        Ok(())
    },
}
