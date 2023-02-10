use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Linker};

use crate::site_context;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::wasm_util::EXTERNREF_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.new",
            |_: Caller<_>| -> Result<Option<ExternRef>, Error> {
                Ok(variant_to_externref(Dictionary::new().owned_to_variant()))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.len",
            |_: Caller<_>, d: Option<ExternRef>| -> Result<i32, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                Ok(d.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.has",
            |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<u32, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                let k = externref_to_variant(k);
                Ok(d.contains(k) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.has_all",
            |_: Caller<_>, d: Option<ExternRef>, ka: Option<ExternRef>| -> Result<u32, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                let ka = site_context!(VariantArray::from_variant(&externref_to_variant(ka)))?;
                Ok(d.contains_all(&ka) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.get",
            |_: Caller<_>,
             d: Option<ExternRef>,
             k: Option<ExternRef>|
             -> Result<Option<ExternRef>, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                let k = externref_to_variant(k);
                match d.get(k) {
                    Some(v) => Ok(variant_to_externref(v)),
                    _ => Ok(None),
                }
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.set",
            |_: Caller<_>,
             d: Option<ExternRef>,
             k: Option<ExternRef>,
             v: Option<ExternRef>|
             -> Result<u32, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                let k = externref_to_variant(k);
                let v = externref_to_variant(v);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let d = unsafe { d.assume_unique() };
                let r = d.contains(k.clone());
                d.insert(k, v);
                Ok(r as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.delete",
            |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<u32, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                let k = externref_to_variant(k);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let d = unsafe { d.assume_unique() };
                let r = d.contains(k.clone());
                d.erase(k);
                Ok(r as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.keys",
            |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                Ok(variant_to_externref(d.keys().owned_to_variant()))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.values",
            |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                Ok(variant_to_externref(d.values().owned_to_variant()))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.clear",
            |_: Caller<_>, d: Option<ExternRef>| -> Result<(), Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let d = unsafe { d.assume_unique() };
                d.clear();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "dictionary.duplicate",
            |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
                let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
                Ok(variant_to_externref(d.duplicate().owned_to_variant()))
            },
        )
        .unwrap();
}
