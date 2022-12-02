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
            "array.new",
            |mut ctx: Caller<StoreData>| -> Result<u32, Error> {
                Ok(ctx
                    .data_mut()
                    .get_registry_mut()?
                    .register(VariantArray::new().owned_to_variant()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.len",
            |ctx: Caller<StoreData>, i: u32| -> Result<i32, Error> {
                Ok(
                    VariantArray::from_variant(&ctx.data().get_registry()?.get_or_nil(i as _))?
                        .len(),
                )
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.get",
            |mut ctx: Caller<StoreData>, v: u32, i: i32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                Ok(reg.register(v.get(i)) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.set",
            |ctx: Caller<StoreData>, v: u32, i: i32, x: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);
                v.set(i, x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.slice",
            |mut ctx: Caller<StoreData>,
             v: u32,
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
                let v = VariantArray::from_variant(&ctx.data().get_registry()?.get_or_nil(v as _))?;

                if to == from {
                    return Ok(0);
                }

                let n = (to - from) as usize;
                let p = p as usize;

                let (ps, data) = mem.data_and_store_mut(&mut ctx);
                let reg = data.get_registry_mut()?;
                let ps = match ps.get_mut(p..p + n * 4) {
                    Some(v) => v,
                    None => bail!("Invalid memory bounds ({}-{})", p, p + n * 4),
                };

                let mut ret = 0u32;
                for i in from as usize..to as usize {
                    let v = reg.register(v.get(i as _)) as u32;

                    ps[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                    ret += 1;
                }

                Ok(ret)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.count",
            |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<i32, Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);
                Ok(v.count(x))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.contains",
            |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<u32, Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);
                Ok(v.contains(x) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.find",
            |ctx: Caller<StoreData>, v: u32, x: u32, from: i32| -> Result<i32, Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);
                Ok(v.find(x, from))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.rfind",
            |ctx: Caller<StoreData>, v: u32, x: u32, from: i32| -> Result<i32, Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);
                Ok(v.rfind(x, from))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.find_last",
            |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<i32, Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);
                Ok(v.find_last(x))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.invert",
            |ctx: Caller<StoreData>, v: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                v.invert();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.sort",
            |ctx: Caller<StoreData>, v: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                v.sort();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.duplicate",
            |mut ctx: Caller<StoreData>, v: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                Ok(reg.register(v.duplicate().owned_to_variant()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.clear",
            |ctx: Caller<StoreData>, v: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.clear();
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.remove",
            |ctx: Caller<StoreData>, v: u32, i: i32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.remove(i);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.erase",
            |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.erase(x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.resize",
            |ctx: Caller<StoreData>, v: u32, i: i32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.resize(i);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.push",
            |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.push(x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.pop",
            |mut ctx: Caller<StoreData>, v: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                Ok(reg.register(v.pop()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.push_front",
            |ctx: Caller<StoreData>, v: u32, x: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.push_front(x);
                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.pop_front",
            |mut ctx: Caller<StoreData>, v: u32| -> Result<u32, Error> {
                let reg = ctx.data_mut().get_registry_mut()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                Ok(reg.register(v.pop_front()) as _)
            },
        )
        .unwrap();

    linker
        .func_wrap(
            OBJREGISTRY_MODULE,
            "array.insert",
            |ctx: Caller<StoreData>, v: u32, i: i32, x: u32| -> Result<(), Error> {
                let reg = ctx.data().get_registry()?;
                let v = VariantArray::from_variant(&reg.get_or_nil(v as _))?;
                let x = reg.get_or_nil(x as _);

                // SAFETY: It's up to wasm/godot if dictionary is uniquely held.
                let v = unsafe { v.assume_unique() };
                v.insert(i, x);
                Ok(())
            },
        )
        .unwrap();
}
