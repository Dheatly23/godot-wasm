mod funcs;

use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{AsContext, AsContextMut, ExternRef, Rooted};

use crate::godot_util::SendSyncWrapper;
use crate::{bail_with_site, site_context};
pub use funcs::Funcs;

pub fn externref_to_variant(
    ctx: impl AsContext,
    v: Option<Rooted<ExternRef>>,
) -> AnyResult<Variant> {
    Ok(match v {
        None => None,
        Some(v) => match site_context!(v.data(ctx.as_context()))? {
            None => bail_with_site!("Externref is created by guest"),
            Some(v) => v
                .downcast_ref::<SendSyncWrapper<Variant>>()
                .map(|v| (**v).clone()),
        },
    }
    .unwrap_or_default())
}

pub fn variant_to_externref(
    ctx: impl AsContextMut,
    v: Variant,
) -> AnyResult<Option<Rooted<ExternRef>>> {
    if v.is_nil() {
        Ok(None)
    } else {
        site_context!(ExternRef::new(ctx, SendSyncWrapper::new(v)).map(Some))
    }
}
