use anyhow::Result as AnyResult;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "dictionary.",
    new => |ctx: Caller<'_, _>| -> AnyResult<Option<Rooted<ExternRef>>> {
        variant_to_externref(ctx, Dictionary::new().owned_to_variant())
    },
    len => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<i32> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        Ok(d.len())
    },
    has => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        let k = externref_to_variant(&ctx, k)?;
        Ok(d.contains(k) as _)
    },
    has_all => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, ka: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        let ka = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, ka)?))?;
        Ok(d.contains_all(&ka) as _)
    },
    get => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        let k = externref_to_variant(&ctx, k)?;
        match d.get(k) {
            Some(v) => variant_to_externref(ctx, v),
            _ => Ok(None),
        }
    },
    set => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        let k = externref_to_variant(&ctx, k)?;
        let v = externref_to_variant(&ctx, v)?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        let r = d.contains(k.clone());
        d.insert(k, v);
        Ok(r as _)
    },
    delete => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        let k = externref_to_variant(&ctx, k)?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        let r = d.contains(k.clone());
        d.erase(k);
        Ok(r as _)
    },
    keys => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        variant_to_externref(ctx, d.keys().owned_to_variant())
    },
    values => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        variant_to_externref(ctx, d.values().owned_to_variant())
    },
    clear => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        d.clear();
        Ok(())
    },
    duplicate => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(Dictionary::from_variant(&externref_to_variant(&ctx, d)?))?;
        variant_to_externref(ctx, d.duplicate().owned_to_variant())
    },
}
