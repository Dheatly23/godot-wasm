use anyhow::Result as AnyResult;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "array.",
    new => |ctx: Caller<'_, _>| -> AnyResult<Option<Rooted<ExternRef>>> {
        variant_to_externref(ctx, VariantArray::new().owned_to_variant())
    },
    len => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<i32> {
        Ok(site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?.len())
    },
    get => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: i32| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        variant_to_externref(ctx, v.get(i))
    },
    set => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: i32, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        v.set(i, x);
        Ok(())
    },
    count => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<i32> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(v.count(x))
    },
    contains => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(v.contains(x) as _)
    },
    find => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>, from: i32| -> AnyResult<i32> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(v.find(x, from))
    },
    rfind => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>, from: i32| -> AnyResult<i32> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(v.rfind(x, from))
    },
    find_last => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<i32> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(v.find_last(x))
    },
    invert => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        v.invert();
        Ok(())
    },
    sort => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        v.sort();
        Ok(())
    },
    duplicate => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        variant_to_externref(ctx, v.duplicate().owned_to_variant())
    },
    clear => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.clear();
        Ok(())
    },
    remove => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: i32| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.remove(i);
        Ok(())
    },
    erase => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.erase(x);
        Ok(())
    },
    resize => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: i32| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.resize(i);
        Ok(())
    },
    push => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.push(x);
        Ok(())
    },
    pop => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        variant_to_externref(ctx, v.pop())
    },
    push_front => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.push_front(x);
        Ok(())
    },
    pop_front => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        variant_to_externref(ctx, v.pop_front())
    },
    insert => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: i32, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let v = site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.insert(i, x);
        Ok(())
    },
}
