mod funcs;

use anyhow::Result as AnyResult;
use gdnative::prelude::*;
use wasmtime::{AsContext, AsContextMut, ExternRef, Rooted};

use crate::site_context;
pub use funcs::Funcs;

pub fn externref_to_variant(
    ctx: impl AsContext,
    v: Option<Rooted<ExternRef>>,
) -> AnyResult<Variant> {
    v.and_then(|v| {
        site_context!(v.data(ctx.as_context()))
            .map(|v| v.downcast_ref::<Variant>().cloned())
            .transpose()
    })
    .transpose()
    .map(|v| v.unwrap_or_else(Variant::nil))
}

pub fn variant_to_externref(
    ctx: impl AsContextMut,
    v: Variant,
) -> AnyResult<Option<Rooted<ExternRef>>> {
    if v.is_nil() {
        Ok(None)
    } else {
        site_context!(ExternRef::new(ctx, v).map(Some))
    }
}
