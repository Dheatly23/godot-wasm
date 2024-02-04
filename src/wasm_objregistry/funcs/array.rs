use anyhow::Error;
use godot::prelude::*;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "array.",
    new => |mut ctx: Caller<T>| -> Result<u32, Error> {
        Ok(ctx
            .data_mut().as_mut()
            .get_registry_mut()?
            .register(<Array<Variant>>::new().to_variant()) as _)
    },
    len => |ctx: Caller<T>, i: u32| -> Result<u32, Error> {
        Ok(site_context!(<Array<Variant>>::try_from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?
        .len() as _)
    },
    get => |mut ctx: Caller<T>, v: u32, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        Ok(reg.register(v.get(i as _)) as _)
    },
    set => |ctx: Caller<T>, v: u32, i: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        v.set(i as _, x);
        Ok(())
    },
    slice => |mut ctx: Caller<T>, v: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
        if to > from {
            bail_with_site!("Invalid range ({}..{})", from, to);
        }
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };
        let v = site_context!(<Array<Variant>>::try_from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(v as _)
        ))?;

        if to == from {
            return Ok(0);
        }

        let n = (to - from) as usize;
        let p = p as usize;

        let (ps, data) = mem.data_and_store_mut(&mut ctx);
        let reg = data.as_mut().get_registry_mut()?;
        let ps = match ps.get_mut(p..p + n * 4) {
            Some(v) => v,
            None => bail_with_site!("Invalid memory bounds ({}..{})", p, p + n * 4),
        };

        let mut ret = 0u32;
        for (i, p) in (from as usize..to as usize).zip(ps.chunks_mut(4)) {
            let v = reg.register(v.get(i as _)) as u32;

            p.copy_from_slice(&v.to_le_bytes());
            ret += 1;
        }

        Ok(ret)
    },
    count => |ctx: Caller<T>, v: u32, x: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(v.count(&x) as _)
    },
    contains => |ctx: Caller<T>, v: u32, x: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(v.contains(&x) as _)
    },
    find => |ctx: Caller<T>, v: u32, x: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(match v.find(&x, None) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    find_from => |ctx: Caller<T>, v: u32, x: u32, from: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(match v.find(&x, Some(from as _)) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    rfind => |ctx: Caller<T>, v: u32, x: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(match v.rfind(&x, None) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    rfind_from => |ctx: Caller<T>, v: u32, x: u32, from: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(match v.rfind(&x, Some(from as _)) {
            Some(v) => v as _,
            None => u32::MAX,
        })
    },
    reverse => |ctx: Caller<T>, v: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        v.reverse();
        Ok(())
    },
    sort => |ctx: Caller<T>, v: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        v.sort_unstable();
        Ok(())
    },
    duplicate => |mut ctx: Caller<T>, v: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        Ok(reg.register(v.duplicate_shallow().to_variant()) as _)
    },
    clear => |ctx: Caller<T>, v: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        v.clear();
        Ok(())
    },
    remove => |ctx: Caller<T>, v: u32, i: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        v.remove(i as _);
        Ok(())
    },
    erase => |ctx: Caller<T>, v: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        v.erase(&x);
        Ok(())
    },
    resize => |ctx: Caller<T>, v: u32, i: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        v.resize(i as _);
        Ok(())
    },
    push => |ctx: Caller<T>, v: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        v.push(x);
        Ok(())
    },
    pop => |mut ctx: Caller<T>, v: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        Ok(reg.register(v.pop().unwrap_or_else(Variant::nil)) as _)
    },
    push_front => |ctx: Caller<T>, v: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        v.push_front(x);
        Ok(())
    },
    pop_front => |mut ctx: Caller<T>, v: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        Ok(reg.register(v.pop_front().unwrap_or_else(Variant::nil)) as _)
    },
    insert => |ctx: Caller<T>, v: u32, i: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let mut v = site_context!(<Array<Variant>>::try_from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        v.insert(i as _, x);
        Ok(())
    },
}
