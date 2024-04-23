use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "dictionary.",
    new => |mut ctx: Caller<'_, T>| -> Result<u32, Error> {
        Ok(ctx
            .data_mut().as_mut()
            .get_registry_mut()?
            .register(Dictionary::new().owned_to_variant()) as _)
    },
    len => |ctx: Caller<'_, T>, i: u32| -> Result<i32, Error> {
        let v = site_context!(Dictionary::from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;
        Ok(v.len())
    },
    has => |ctx: Caller<'_, T>, i: u32, k: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);
        Ok(v.contains(k) as _)
    },
    has_all => |ctx: Caller<'_, T>, i: u32, ka: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        let ka = site_context!(VariantArray::from_variant(&reg.get_or_nil(ka as _)))?;
        Ok(v.contains_all(&ka) as _)
    },
    get => |mut ctx: Caller<'_, T>, i: u32, k: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);
        match v.get(k) {
            Some(v) => Ok(reg.register(v.to_variant()) as _),
            _ => Ok(0),
        }
    },
    set => |ctx: Caller<'_, T>, i: u32, k: u32, v: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let d = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);
        let v = reg.get_or_nil(v as _);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        let r = d.contains(k.clone());
        d.insert(k, v);
        Ok(r as _)
    },
    delete => |ctx: Caller<'_, T>, i: u32, k: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let d = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        let r = d.contains(k.clone());
        d.erase(k);
        Ok(r as _)
    },
    keys => |mut ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let d = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        Ok(reg.register(d.keys().owned_to_variant()) as _)
    },
    values => |mut ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let d = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        Ok(reg.register(d.values().owned_to_variant()) as _)
    },
    iter_slice => |mut ctx: Caller<'_, T>, i: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
        if to > from {
            bail_with_site!("Invalid range ({}..{})", from, to);
        }
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };
        let d = site_context!(Dictionary::from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?;

        if to == from {
            return Ok(0);
        }

        let n = (to - from) as usize;
        let p = p as usize;

        let (ps, data) = mem.data_and_store_mut(&mut ctx);
        let reg = data.as_mut().get_registry_mut()?;
        let ps = match ps.get_mut(p..p + n * 8) {
            Some(v) => v,
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n * 8),
        };

        let mut ret = 0u32;
        for ((k, v), p) in d.iter().skip(from as usize).take(n).zip(ps.chunks_mut(8)) {
            let k = reg.register(k) as u32;
            let v = reg.register(v) as u32;

            p[..4].copy_from_slice(&k.to_le_bytes());
            p[4..].copy_from_slice(&v.to_le_bytes());
            ret += 1;
        }

        Ok(ret)
    },
    clear => |ctx: Caller<'_, T>, i: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let d = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let d = unsafe { d.assume_unique() };
        d.clear();
        Ok(())
    },
    duplicate => |mut ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let d = site_context!(Dictionary::from_variant(&reg.get_or_nil(i as _)))?;
        Ok(reg.register(d.duplicate().owned_to_variant()) as _)
    },
}
