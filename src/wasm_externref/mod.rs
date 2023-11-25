mod funcs;

use gdnative::prelude::*;
use wasmtime::ExternRef;

pub use funcs::Funcs;

pub fn externref_to_variant(v: Option<ExternRef>) -> Variant {
    v.and_then(|v| v.data().downcast_ref::<Variant>().cloned())
        .unwrap_or_else(Variant::nil)
}

pub fn variant_to_externref(v: Variant) -> Option<ExternRef> {
    if v.is_nil() {
        None
    } else {
        Some(ExternRef::new(v))
    }
}
