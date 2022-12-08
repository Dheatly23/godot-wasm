use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Linker};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::wasm_util::EXTERNREF_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.new",
            |_: Caller<_>| -> Result<Option<ExternRef>, Error> {
                Ok(variant_to_externref(VariantArray::new().owned_to_variant()))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.len",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<i32, Error> {
                Ok(VariantArray::from_variant(&externref_to_variant(v))?.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.get",
            |_: Caller<_>, v: Option<ExternRef>, i: i32| -> Result<Option<ExternRef>, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                Ok(variant_to_externref(v.get(i)))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.set",
            |_: Caller<StoreData>,
             v: Option<ExternRef>,
             i: i32,
             x: Option<ExternRef>|
             -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);
                v.set(i, x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.count",
            |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<i32, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);
                Ok(v.count(x))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.contains",
            |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<u32, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);
                Ok(v.contains(x) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.find",
            |_: Caller<_>,
             v: Option<ExternRef>,
             x: Option<ExternRef>,
             from: i32|
             -> Result<i32, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);
                Ok(v.find(x, from))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.rfind",
            |_: Caller<_>,
             v: Option<ExternRef>,
             x: Option<ExternRef>,
             from: i32|
             -> Result<i32, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);
                Ok(v.rfind(x, from))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.find_last",
            |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<i32, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);
                Ok(v.find_last(x))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.invert",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                v.invert();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.sort",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                v.sort();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.duplicate",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                Ok(variant_to_externref(v.duplicate().owned_to_variant()))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.clear",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.clear();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.remove",
            |_: Caller<_>, v: Option<ExternRef>, i: i32| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.remove(i);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.erase",
            |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.erase(x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.resize",
            |_: Caller<_>, v: Option<ExternRef>, i: i32| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.resize(i);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.push",
            |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.push(x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.pop",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                Ok(variant_to_externref(v.pop()))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.push_front",
            |_: Caller<_>, v: Option<ExternRef>, x: Option<ExternRef>| -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.push_front(x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.pop_front",
            |_: Caller<_>, v: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                Ok(variant_to_externref(v.pop_front()))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "array.insert",
            |_: Caller<_>,
             v: Option<ExternRef>,
             i: i32,
             x: Option<ExternRef>|
             -> Result<(), Error> {
                let v = VariantArray::from_variant(&externref_to_variant(v))?;
                let x = externref_to_variant(x);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.insert(i, x);
                Ok(())
            },
        )
        .unwrap();
}
