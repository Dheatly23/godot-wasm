use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::{AsContext, AsContextMut, Caller, ExternRef, Func, Rooted, StoreContextMut};

use crate::godot_util::from_var_any;
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{func_registry, site_context};

func_registry! {
    "dictionary.",
    new => |mut ctx: Caller<'_, _>| -> AnyResult<Option<Rooted<ExternRef>>> {
        variant_to_externref(ctx.as_context_mut(), VarDictionary::new().to_variant())
    },
    len => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        Ok(d.len() as _)
    },
    has => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        let k = externref_to_variant(ctx.as_context(), k)?;
        Ok(d.contains_key(k) as _)
    },
    has_all => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, ka: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        let ka = site_context!(from_var_any::<VarArray>(&externref_to_variant(ctx.as_context(), ka)?))?;
        Ok(d.contains_all_keys(&ka) as _)
    },
    get => |mut ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        let k = externref_to_variant(ctx.as_context(), k)?;
        match d.get(k) {
            Some(v) => variant_to_externref(ctx.as_context_mut(), v),
            None => Ok(None),
        }
    },
    set => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>, v: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        let k = externref_to_variant(ctx.as_context(), k)?;
        let v = externref_to_variant(ctx.as_context(), v)?;
        Ok(d.insert(k, v).is_some() as _)
    },
    delete => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>, k: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let mut d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        let k = externref_to_variant(ctx.as_context(), k)?;
        Ok(d.remove(k).is_some() as _)
    },
    keys => |mut ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        variant_to_externref(ctx.as_context_mut(), d.keys_array().to_variant())
    },
    values => |mut ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        variant_to_externref(ctx.as_context_mut(), d.values_array().to_variant())
    },
    clear => |ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let mut d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        d.clear();
        Ok(())
    },
    duplicate => |mut ctx: Caller<'_, _>, d: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let d = site_context!(from_var_any::<VarDictionary>(&externref_to_variant(ctx.as_context(), d)?))?;
        variant_to_externref(ctx.as_context_mut(), d.duplicate_shallow().to_variant())
    },
}
