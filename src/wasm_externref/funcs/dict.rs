use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "dictionary.",
    new => |_: Caller<_>| -> Result<Option<ExternRef>, Error> {
        Ok(variant_to_externref(Dictionary::new().to_variant()))
    },
    len => |_: Caller<_>, d: Option<ExternRef>| -> Result<u32, Error> {
        let d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        Ok(d.len() as _)
    },
    has => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<u32, Error> {
        let d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);
        Ok(d.contains_key(k) as _)
    },
    has_all => |_: Caller<_>, d: Option<ExternRef>, ka: Option<ExternRef>| -> Result<u32, Error> {
        let d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        let ka = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(ka)))?;
        Ok(d.contains_all_keys(ka) as _)
    },
    get => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);
        Ok(d.get(k).and_then(variant_to_externref))
    },
    set => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>, v: Option<ExternRef>| -> Result<u32, Error> {
        let mut d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);
        let v = externref_to_variant(v);
        Ok(d.insert(k, v).is_some() as _)
    },
    delete => |_: Caller<_>, d: Option<ExternRef>, k: Option<ExternRef>| -> Result<u32, Error> {
        let mut d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        let k = externref_to_variant(k);
        Ok(d.remove(k).is_some() as _)
    },
    keys => |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        Ok(variant_to_externref(d.keys_array().to_variant()))
    },
    values => |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        Ok(variant_to_externref(d.values_array().to_variant()))
    },
    clear => |_: Caller<_>, d: Option<ExternRef>| -> Result<(), Error> {
        let mut d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        d.clear();
        Ok(())
    },
    duplicate => |_: Caller<_>, d: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let d = site_context!(Dictionary::try_from_variant(&externref_to_variant(d)))?;
        Ok(variant_to_externref(d.duplicate_shallow().to_variant()))
    },
}
