use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::godot_util::from_var_any;
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "dictionary.",
    new => |mut ctx: Caller<T>| -> Result<u32, Error> {
        Ok(ctx
            .data_mut().as_mut()
            .get_registry_mut()?
            .register(Dictionary::new().to_variant()) as _)
    },
    len => |ctx: Caller<T>, i: u32| -> Result<u32, Error> {
        Ok(site_context!(from_var_any::<Dictionary>(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?.len() as _)
    },
    has => |ctx: Caller<T>, i: u32, k: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);
        Ok(v.contains_key(k) as _)
    },
    has_all => |ctx: Caller<T>, i: u32, ka: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        let ka = site_context!(from_var_any::<VariantArray>(&reg.get_or_nil(ka as _)))?;
        Ok(v.contains_all_keys(ka) as _)
    },
    get => |mut ctx: Caller<T>, i: u32, k: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);
        match v.get(k) {
            Some(v) => Ok(reg.register(v.to_variant()) as _),
            _ => Ok(0),
        }
    },
    set => |ctx: Caller<T>, i: u32, k: u32, v: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut d = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);
        let v = reg.get_or_nil(v as _);
        Ok(d.insert(k, v).is_some() as _)
    },
    delete => |ctx: Caller<T>, i: u32, k: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut d = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        let k = reg.get_or_nil(k as _);
        Ok(d.remove(k).is_some() as _)
    },
    keys => |mut ctx: Caller<T>, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let d = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        Ok(reg.register(d.keys_array().to_variant()) as _)
    },
    values => |mut ctx: Caller<T>, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let d = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        Ok(reg.register(d.values_array().to_variant()) as _)
    },
    iter_slice => |mut ctx: Caller<T>, i: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
        if to > from {
            bail_with_site!("Invalid range ({}..{})", from, to);
        }
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };
        let d = site_context!(from_var_any::<Dictionary>(
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
        for ((k, v), p) in d.iter_shared().skip(from as usize).take(n).zip(ps.chunks_mut(8)) {
            let k = reg.register(k) as u32;
            let v = reg.register(v) as u32;

            p[..4].copy_from_slice(&k.to_le_bytes());
            p[4..].copy_from_slice(&v.to_le_bytes());
            ret += 1;
        }

        Ok(ret)
    },
    clear => |ctx: Caller<T>, i: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut d = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        d.clear();
        Ok(())
    },
    duplicate => |mut ctx: Caller<T>, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let d = site_context!(from_var_any::<Dictionary>(&reg.get_or_nil(i as _)))?;
        Ok(reg.register(d.duplicate_shallow().to_variant()) as _)
    },
}
