use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "array.",
    new => |mut ctx: Caller<'_, T>| -> Result<u32, Error> {
        Ok(ctx
            .data_mut().as_mut()
            .get_registry_mut()?
            .register(VariantArray::new().owned_to_variant()) as _)
    },
    len => |ctx: Caller<'_, T>, i: u32| -> Result<i32, Error> {
        Ok(site_context!(VariantArray::from_variant(
            &ctx.data().as_ref().get_registry()?.get_or_nil(i as _)
        ))?
        .len())
    },
    get => |mut ctx: Caller<'_, T>, v: u32, i: i32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        Ok(reg.register(v.get(i)) as _)
    },
    set => |ctx: Caller<'_, T>, v: u32, i: i32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        v.set(i, x);
        Ok(())
    },
    slice => |mut ctx: Caller<'_, T>, v: u32, from: u32, to: u32, p: u32| -> Result<u32, Error> {
        if to > from {
            bail_with_site!("Invalid range ({}..{})", from, to);
        }
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };
        let v = site_context!(VariantArray::from_variant(
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
    count => |ctx: Caller<'_, T>, v: u32, x: u32| -> Result<i32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(v.count(x))
    },
    contains => |ctx: Caller<'_, T>, v: u32, x: u32| -> Result<u32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(v.contains(x) as _)
    },
    find => |ctx: Caller<'_, T>, v: u32, x: u32, from: i32| -> Result<i32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(v.find(x, from))
    },
    rfind => |ctx: Caller<'_, T>, v: u32, x: u32, from: i32| -> Result<i32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(v.rfind(x, from))
    },
    find_last => |ctx: Caller<'_, T>, v: u32, x: u32| -> Result<i32, Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);
        Ok(v.find_last(x))
    },
    invert => |ctx: Caller<'_, T>, v: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        v.invert();
        Ok(())
    },
    sort => |ctx: Caller<'_, T>, v: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        v.sort();
        Ok(())
    },
    duplicate => |mut ctx: Caller<'_, T>, v: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        Ok(reg.register(v.duplicate().owned_to_variant()) as _)
    },
    clear => |ctx: Caller<'_, T>, v: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.clear();
        Ok(())
    },
    remove => |ctx: Caller<'_, T>, v: u32, i: i32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.remove(i);
        Ok(())
    },
    erase => |ctx: Caller<'_, T>, v: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.erase(x);
        Ok(())
    },
    resize => |ctx: Caller<'_, T>, v: u32, i: i32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.resize(i);
        Ok(())
    },
    push => |ctx: Caller<'_, T>, v: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.push(x);
        Ok(())
    },
    pop => |mut ctx: Caller<'_, T>, v: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        Ok(reg.register(v.pop()) as _)
    },
    push_front => |ctx: Caller<'_, T>, v: u32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.push_front(x);
        Ok(())
    },
    pop_front => |mut ctx: Caller<'_, T>, v: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        Ok(reg.register(v.pop_front()) as _)
    },
    insert => |ctx: Caller<'_, T>, v: u32, i: i32, x: u32| -> Result<(), Error> {
        let reg = ctx.data().as_ref().get_registry()?;
        let v = site_context!(VariantArray::from_variant(&reg.get_or_nil(v as _)))?;
        let x = reg.get_or_nil(x as _);

        // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
        let v = unsafe { v.assume_unique() };
        v.insert(i, x);
        Ok(())
    },
}
