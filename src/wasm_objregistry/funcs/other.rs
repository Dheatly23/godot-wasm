use anyhow::Error;
use wasmtime::{Caller, Extern, Func, StoreContextMut};

use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry};

func_registry! {
    "",
    delete => |mut ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
        match ctx.data_mut().as_mut().get_registry_mut()?.unregister(i as _) {
            Some(_) => Ok(1),
            None => Ok(0),
        }
    },
    delete_many => |mut ctx: Caller<'_, T>, p: u32, n: u32| -> Result<u32, Error> {
        let mem = match ctx.get_export("memory") {
            Some(Extern::Memory(v)) => v,
            _ => return Ok(0),
        };

        let n = n as usize;
        let p = p as usize;

        let (ps, data) = mem.data_and_store_mut(&mut ctx);
        let reg = data.as_mut().get_registry_mut()?;
        let ps = match ps.get(p..p + n * 4) {
            Some(v) => v,
            None => bail_with_site!("Invalid memory bounds ({}-{})", p, p + n * 4),
        };

        let mut ret = 0u32;
        for i in ps.chunks(4) {
            reg.unregister(u32::from_le_bytes(i.try_into().unwrap()) as _);
            ret += 1;
        }

        Ok(ret)
    },
    duplicate => |mut ctx: Caller<'_, T>, i: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = reg.get_or_nil(i as _);
        Ok(reg.register(v) as _)
    },
    copy => |mut ctx: Caller<'_, T>, s: u32, d: u32| -> Result<u32, Error> {
        let reg = ctx.data_mut().as_mut().get_registry_mut()?;
        let v = reg.get_or_nil(s as _);
        match reg.replace(d as _, v) {
            Some(_) => Ok(1),
            None => Ok(0),
        }
    },
}
