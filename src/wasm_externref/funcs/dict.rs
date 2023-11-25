use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "dictionary.",
    new => |_: Caller<_>| -> Result<Option<ExternRef>, Error> {
        Ok(variant_to_externref(Dictionary::new().owned_to_variant()))
    },
    len => |_: Caller<_>, d: Option<ExternRef>| -> Result<i32, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        Ok(d.len())
    },
    has => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<u32, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);
        Ok(d.contains(k) as _)
    },
    has_all => |_: Caller<_>, d: Option<ExternRef>, ka: Option<ExternRef>| -> Result<u32, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        let ka = site_context!(VariantArray::from_variant(&externref_to_variant(ka)))?;
        Ok(d.contains_all(&ka) as _)
    },
    get => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);
        match d.get(k) {
            Some(v) => Ok(variant_to_externref(v)),
            _ => Ok(None),
        }
    },
    set => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>, v: Option<ExternRef>| -> Result<u32, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);
        let v = externref_to_variant(v);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        let r = d.contains(k.clone());
        d.insert(k, v);
        Ok(r as _)
    },
    delete => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<u32, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        let r = d.contains(k.clone());
        d.erase(k);
        Ok(r as _)
    },
    keys => |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        Ok(variant_to_externref(d.keys().owned_to_variant()))
    },
    values => |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        Ok(variant_to_externref(d.values().owned_to_variant()))
    },
    clear => |_: Caller<_>, d: Option<ExternRef>| -> Result<(), Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        d.clear();
        Ok(())
    },
    duplicate => |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(d)))?;
        Ok(variant_to_externref(d.duplicate().owned_to_variant()))
    },
}
