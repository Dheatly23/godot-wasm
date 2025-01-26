mod funcs;

use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{ExternRef, Rooted, StoreContext, StoreContextMut};

use crate::godot_util::SendSyncWrapper;
use crate::{bail_with_site, site_context};
pub use funcs::Funcs;

pub fn externref_to_variant<T>(
    ctx: StoreContext<'_, T>,
    v: Option<Rooted<ExternRef>>,
) -> AnyResult<Variant> {
    Ok(match v {
        None => None,
        Some(v) => match site_context!(v.data(ctx))? {
            None => bail_with_site!("Externref is created by guest"),
            Some(v) => v
                .downcast_ref::<SendSyncWrapper<Variant>>()
                .map(|v| (**v).clone()),
        },
    }
    .unwrap_or_default())
}

pub fn variant_to_externref<T>(
    ctx: StoreContextMut<'_, T>,
    v: Variant,
) -> AnyResult<Option<Rooted<ExternRef>>> {
    if v.is_nil() {
        Ok(None)
    } else {
        site_context!(ExternRef::new(ctx, SendSyncWrapper::new(v)).map(Some))
    }
}
