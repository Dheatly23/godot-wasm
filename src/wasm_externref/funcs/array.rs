use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "array.",
    new => |_: Caller<_>| -> Result<Option<ExternRef>, Error> {
        Ok(variant_to_externref(VariantArray::new().owned_to_variant()))
    },
    len => |_: Caller<_>, v: Option<ExternRef>| -> Result<i32, Error> {
        Ok(site_context!(VariantArray::from_variant(&externref_to_variant(v)))?.len())
    },
    get => |_: Caller<_>, v: Option<ExternRef>, i: i32| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(v.get(i)))
    },
    set => |_: Caller<_>, v: Option<ExternRef>, i: i32, x: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        v.set(i, x);
        Ok(())
    },
    count => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<i32, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(v.count(x))
    },
    contains => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<u32, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(v.contains(x) as _)
    },
    find => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>, from: i32| -> Result<i32, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(v.find(x, from))
    },
    rfind => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>, from: i32| -> Result<i32, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(v.rfind(x, from))
    },
    find_last => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<i32, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);
        Ok(v.find_last(x))
    },
    invert => |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        v.invert();
        Ok(())
    },
    sort => |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        v.sort();
        Ok(())
    },
    duplicate => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        Ok(variant_to_externref(v.duplicate().owned_to_variant()))
    },
    clear => |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.clear();
        Ok(())
    },
    remove => |_: Caller<_>, v: Option<ExternRef>, i: i32| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.remove(i);
        Ok(())
    },
    erase => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.erase(x);
        Ok(())
    },
    resize => |_: Caller<_>, v: Option<ExternRef>, i: i32| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.resize(i);
        Ok(())
    },
    push => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.push(x);
        Ok(())
    },
    pop => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        Ok(variant_to_externref(v.pop()))
    },
    push_front => |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.push_front(x);
        Ok(())
    },
    pop_front => |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        Ok(variant_to_externref(v.pop_front()))
    },
    insert => |_: Caller<_>, v: Option<ExternRef>, i: i32, x: Option<ExternRef>| -> Result<(), Error> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(v)))?;
        let x = externref_to_variant(x);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.insert(i, x);
        Ok(())
    },
}
