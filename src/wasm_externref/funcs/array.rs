use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "array.",
    new => |_: Caller<_>| -> Result<Option<ExternRef>, Error> {
        Ok(variant_to_externref(<Array<Variant>>::new().to_variant()))
    },
    len => |_: Caller<_>, v: Option<ExternRef>| -> Result<u32, Error> {
        Ok(site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?.len() as _)
    },
    get => |_: Caller<_>, v: Option<ExternRef>, i: u32| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(v.get(i as _)))
    },
    set => |_: Caller<_>, v: Option<ExternRef>, i: u32, x: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        v.set(i as _, x);
        Ok(())
    },
    count => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<u32, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(v.count(&x) as _)
    },
    contains => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<u32, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(v.contains(&x) as _)
    },
    find => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<u32, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(match v.find(&x, None) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    find_from => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>, from: u32| -> Result<u32, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(match v.find(&x, Some(from as _)) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    rfind => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<u32, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(match v.rfind(&x, None) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    rfind_from => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>, from: u32| -> Result<u32, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(match v.rfind(&x, Some(from as _)) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    reverse => |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        v.reverse();
        Ok(())
    },
    sort => |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        v.sort_unstable();
        Ok(())
    },
    duplicate => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(v.duplicate_shallow().to_variant()))
    },
    clear => |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        v.clear();
        Ok(())
    },
    remove => |_: Caller<_>, v: Option<ExternRef>, i: u32| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        v.remove(i as _);
        Ok(())
    },
    erase => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        v.erase(&x);
        Ok(())
    },
    resize => |_: Caller<_>, v: Option<ExternRef>, i: u32| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        v.resize(i as _, &Variant::nil());
        Ok(())
    },
    push => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        v.push(x);
        Ok(())
    },
    pop => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(v.pop().unwrap_or_else(Variant::nil)))
    },
    push_front => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        v.push_front(x);
        Ok(())
    },
    pop_front => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(v.pop_front().unwrap_or_else(Variant::nil)))
    },
    insert => |_: Caller<_>, v: Option<ExternRef>, i: u32, x: Option<ExternRef>| -> Result<(), Error> {
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        v.insert(i as _, x);
        Ok(())
    },
}
