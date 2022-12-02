use anyhow::{bail, Error};
use gdnative::prelude::*;
use wasmtime::{Caller, Extern, Linker};

use crate::wasm_instance::StoreData;
use crate::wasm_util::OBJREGISTRY_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.new",
            |mut ctx: Caller<StoreData>| -> Result<u32, Error> {
                Ok(ctx
                    .data_mut()
                    .get_registry_mut()?
                    .register(Dictionary::new().owned_to_variant()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.len",
            |ctx: Caller<StoreData>, i: u32| -> Result<i32, Error> {
                let v = Dictionary::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;
                Ok(v.len())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.has",
            |ctx: Caller<StoreData>, i: u32, k: u32| -> Result<u32, Error> {
                let reg = ctx.data().get_registry()?;
                let v = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                let k = reg.get_or_nil(k as _);
                Ok(v.contains(k) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.has_all",
            |ctx: Caller<StoreData>, i: u32, ka: u32| -> Result<u32, Error> {
                let reg = ctx.data().get_registry()?;
                let v = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                let ka = VariantArray::from_variant(&reg.get_or_nil(ka as _))?;
                Ok(v.contains_all(&ka) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.get",
            |mut ctx: Caller<StoreData>, i: u32, k: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let v = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                let k = reg.get_or_nil(k as _);
                match v.get(k) {
                    Some(v) => Ok(reg.register(v.to_variant()) as _),
                    _ => Ok(0),
                }
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.set",
            |ctx: Caller<StoreData>, i: u32, k: u32, v: u32| -> Result<u32, Error> {
                let reg = ctx.data().get_registry()?;
                let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                let k = reg.get_or_nil(k as _);
                let v = reg.get_or_nil(v as _);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let d = unsafe { d.assume_unique() };
                let r = d.contains(k.clone());
                d.insert(k, v);
                Ok(r as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.delete",
            |ctx: Caller<StoreData>, i: u32, k: u32| -> Result<u32, Error> {
                let reg = ctx.data().get_registry()?;
                let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                let k = reg.get_or_nil(k as _);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let d = unsafe { d.assume_unique() };
                let r = d.contains(k.clone());
                d.erase(k);
                Ok(r as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.keys",
            |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                Ok(reg.register(d.keys().owned_to_variant()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.values",
            |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                Ok(reg.register(d.values().owned_to_variant()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.iter_slice",
            |mut ctx: Caller<StoreData>,
             i: u32,
             from: u32,
             to: u32,
             p: u32|
             -> Result<u32, Error> {
                if to > from {
                    bail!("Invalid range ({}-{})", from, to);
                }
                let mem = match ctx.get_export("memory") {
                    Some(Extern::Memory(v)) => v,
                    _ => return Ok(0),
                };
                let d = Dictionary::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?;

                if to == from {
                    return Ok(0);
                }

                let n = (to - from) as usize;
                let p = p as usize;

                let (ps, data) = mem.data_and_store_mut(&mut ctx);
                let reg = data.get_registry_mut()?;
                let ps = match ps.get_mut(p..p + n * 8) {
                    Some(v) => v,
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n * 8),
                };

                let mut ret = 0u32;
                let from = from as usize;
                for (i, (k, v)) in d.iter().skip(from).take(n).enumerate() {
                    let k = reg.register(k) as u32;
                    let v = reg.register(v) as u32;

                    ps[i * 8..i * 8 + 4].copy_from_slice(&k.to_le_bytes());
                    ps[i * 8 + 4..i * 8 + 8].copy_from_slice(&v.to_le_bytes());
                    ret += 1;
                }

                Ok(ret)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.clear",
            |ctx: Caller<StoreData>, i: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let d = unsafe { d.assume_unique() };
                d.clear();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "dictionary.duplicate",
            |mut ctx: Caller<StoreData>, i: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let d = Dictionary::from_variant(&reg.get_or_nil(i as _))?;
                Ok(reg.register(d.duplicate().owned_to_variant()) as _)
            },
        )
        .unwrap();
}
