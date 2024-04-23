mod funcs;

use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{AsContext, AsContextMut, ExternRef, Rooted};

use crate::godot_util::SendSyncWrapper;
use crate::site_context;
pub use funcs::Funcs;

pub fn externref_to_variant(
    ctx: impl AsContext,
    v: Option<Rooted<ExternRef>>,
) -> AnyResult<Variant> {
    v.and_then(|v| {
        site_context!(v.data(ctx.as_context()))
            .map(|v| {
                v.downcast_ref::<SendSyncWrapper<Variant>>()
                    .map(|v| (**v).clone())
            })
            .transpose()
    })
    .transpose()
    .map(|v| v.unwrap_or_default())
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
