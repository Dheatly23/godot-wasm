use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "array.",
    new => |ctx: Caller<'_, _>| -> AnyResult<Option<Rooted<ExternRef>>> {
        variant_to_externref(ctx, VariantArray::new().to_variant())
    },
    len => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        Ok(site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx, v)?))?.len() as _)
    },
    get => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: u32| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        variant_to_externref(ctx, v.get(i as _))
    },
    set => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: u32, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        v.set(i as _, x);
        Ok(())
    },
    count => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(v.count(&x) as _)
    },
    contains => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(v.contains(&x) as _)
    },
    find => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(match v.find(&x, None) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    find_from => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>, from: u32| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(match v.find(&x, Some(from as _)) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    rfind => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(match v.rfind(&x, None) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    rfind_from => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>, from: u32| -> AnyResult<u32> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        Ok(match v.rfind(&x, Some(from as _)) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    reverse => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx, v)?))?.reverse();
        Ok(())
    },
    sort => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx, v)?))?.sort_unstable();
        Ok(())
    },
    duplicate => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        variant_to_externref(ctx, v.duplicate_shallow().to_variant())
    },
    clear => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx, v)?))?.clear();
        Ok(())
    },
    remove => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: u32| -> AnyResult<()> {
        site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx, v)?))?.remove(i as _);
        Ok(())
    },
    erase => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        v.erase(&x);
        Ok(())
    },
    resize => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: u32| -> AnyResult<()> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(ctx, v)?))?;
        v.resize(i as _, &Variant::nil());
        Ok(())
    },
    push => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        v.push(x);
        Ok(())
    },
    pop => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        match v.pop() {
            Some(v) => variant_to_externref(ctx, v),
            None => Ok(None),
        }
    },
    push_front => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        v.push_front(x);
        Ok(())
    },
    pop_front => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        match v.pop_front() {
            Some(v) => variant_to_externref(ctx, v),
            None => Ok(None),
        }
    },
    insert => |ctx: Caller<'_, _>, v: Option<Rooted<ExternRef>>, i: u32, x: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut v = site_context!(from_var_any::<VariantArray>(&externref_to_variant(&ctx, v)?))?;
        let x = externref_to_variant(&ctx, x)?;
        v.insert(i as _, x);
        Ok(())
    },
}
