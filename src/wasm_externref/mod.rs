mod funcs;

use godot::prelude::*;
use wasmtime::ExternRef;

use crate::godot_util::SendSyncWrapper;
pub use funcs::Funcs;

pub fn externref_to_variant(v: Option<ExternRef>) -> Variant {
    v.and_then(|v| {
        v.data()
            .downcast_ref::<SendSyncWrapper<Variant>>()
            .map(|v| (**v).clone())
    })
    .unwrap_or_default()
}

pub fn variant_to_externref(v: Variant) -> Option<ExternRef> {
    if v.is_nil() {
        None
    } else {
        Some(ExternRef::new(SendSyncWrapper::new(v)))
    }
}
