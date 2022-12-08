mod funcs;

use gdnative::prelude::*;
use lazy_static::lazy_static;
use wasmtime::{ExternRef, Linker};

use crate::wasm_engine::ENGINE;
use crate::wasm_instance::StoreData;

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

lazy_static! {
    pub static ref EXTERNREF_LINKER: Linker<StoreData> = {
        let mut linker: Linker<StoreData> = Linker::new(&ENGINE);

        funcs::register_functions(&mut linker);

        linker
    };
}
