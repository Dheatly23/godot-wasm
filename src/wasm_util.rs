#[cfg(not(feature = "new-host-import"))]
use std::collections::HashMap;
#[cfg(not(feature = "new-host-import"))]
use std::marker::PhantomData;
#[cfg(feature = "object-registry-extern")]
use std::ptr;
#[cfg(feature = "epoch-timeout")]
use std::time;

use anyhow::{bail, Error};
use gdnative::api::WeakRef;
use gdnative::log::Site;
use gdnative::prelude::*;
#[cfg(feature = "new-host-import")]
use wasmtime::Linker;
#[cfg(feature = "epoch-timeout")]
use wasmtime::UpdateDeadline;
use wasmtime::{AsContextMut, Caller, Extern, Func, FuncType, Store, ValRaw, ValType};
#[cfg(feature = "object-registry-extern")]
use wasmtime::{ExternRef, RefType};

#[cfg(feature = "epoch-timeout")]
use crate::wasm_config::Config;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_engine::{ENGINE, EPOCH};
#[cfg(feature = "object-registry-extern")]
use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;

#[cfg(all(feature = "epoch-timeout", not(feature = "more-precise-timer")))]
pub const EPOCH_MULTIPLIER: u64 = 1000;
#[cfg(all(feature = "epoch-timeout", feature = "more-precise-timer"))]
pub const EPOCH_MULTIPLIER: u64 = 50;
#[cfg(feature = "epoch-timeout")]
pub const EPOCH_DEADLINE: u64 = 5u64.saturating_mul(EPOCH_MULTIPLIER);
#[cfg(feature = "epoch-timeout")]
pub const EPOCH_INTERVAL: time::Duration = time::Duration::from_millis(1000 / EPOCH_MULTIPLIER);

/*
#[cfg(feature = "wasi")]
pub const FILE_NOTEXIST: u32 = 0;
#[cfg(feature = "wasi")]
pub const FILE_FILE: u32 = 1;
#[cfg(feature = "wasi")]
pub const FILE_DIR: u32 = 2;
#[cfg(feature = "wasi")]
pub const FILE_LINK: u32 = 3;
*/

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;
#[cfg(feature = "object-registry-extern")]
pub const TYPE_VARIANT: u32 = 6;

#[cfg(not(feature = "new-host-import"))]
pub const HOST_MODULE: &str = "host";
#[cfg(feature = "object-registry-compat")]
pub const OBJREGISTRY_MODULE: &str = "godot_object_v1";
#[cfg(feature = "object-registry-extern")]
pub const EXTERNREF_MODULE: &str = "godot_object_v2";

pub const MODULE_INCLUDES: &[&str] = &[
    #[cfg(not(feature = "new-host-import"))]
    HOST_MODULE,
    #[cfg(feature = "object-registry-compat")]
    OBJREGISTRY_MODULE,
    #[cfg(feature = "object-registry-extern")]
    EXTERNREF_MODULE,
    #[cfg(feature = "wasi")]
    "wasi_unstable",
    #[cfg(feature = "wasi")]
    "wasi_snapshot_preview0",
    #[cfg(feature = "wasi")]
    "wasi_snapshot_preview1",
];

pub const MEMORY_EXPORT: &str = "memory";

#[macro_export]
macro_rules! bail_with_site {
    ($($t:tt)*) => {
        return Err(anyhow::anyhow!($($t)*).context(gdnative::log::godot_site!()))
    };
}

#[macro_export]
macro_rules! site_context {
    ($e:expr) => {
        $e.map_err(|e| {
            $crate::wasm_util::add_site(anyhow::Error::from(e), gdnative::log::godot_site!())
        })
    };
}

pub fn add_site(e: Error, site: Site<'static>) -> Error {
    if e.is::<Site>() {
        e
    } else {
        e.context(site)
    }
}

#[macro_export]
macro_rules! func_registry{
    ($head:literal, $($t:tt)*) => {
        $crate::func_registry!{(Funcs, $head), $($t)*}
    };
    (($fi:ident, $head:literal) $(, $i:ident => $e:expr)* $(,)?) => {
        #[derive(Default)]
        pub struct $fi {
            $($i: Option<Func>),*
        }

        impl $fi {
            pub fn get_func<T>(&mut self, store: &mut StoreContextMut<'_, T>, name: &str) -> Option<Func>
            where
                T: AsRef<StoreData> + AsMut<StoreData>,
            {
                match name {
                    $(concat!($head, stringify!($i)) => Some(self.$i.get_or_insert_with(move || Func::wrap(store, $e)).clone()),)*
                    _ => None,
                }
            }
        }
    };
}

pub fn from_signature(sig: &FuncType) -> Result<(PoolArray<u8>, PoolArray<u8>), Error> {
    let p = sig.params();
    let r = sig.results();

    let mut pr = <PoolArray<u8>>::new();
    let mut rr = <PoolArray<u8>>::new();

    pr.resize(p.len() as _);
    rr.resize(r.len() as _);

    for (s, d) in p
        .zip(pr.write().iter_mut())
        .chain(r.zip(rr.write().iter_mut()))
    {
        *d = match s {
            ValType::I32 => TYPE_I32,
            ValType::I64 => TYPE_I64,
            ValType::F32 => TYPE_F32,
            ValType::F64 => TYPE_F64,
            #[cfg(feature = "object-registry-extern")]
            ValType::Ref(r) if RefType::eq(&r, &RefType::EXTERNREF) => TYPE_VARIANT,
            _ => bail_with_site!("Unconvertible signture"),
        } as _;
    }

    Ok((pr, rr))
}

pub fn to_signature(params: Variant, results: Variant) -> Result<FuncType, Error> {
    fn f(it: impl Iterator<Item = Result<u32, Error>>) -> Result<Vec<ValType>, Error> {
        let mut ret = match it.size_hint() {
            (_, Some(n)) => Vec::with_capacity(n),
            (n, None) => Vec::with_capacity(n),
        };

        for i in it {
            ret.push(match i? {
                TYPE_I32 => ValType::I32,
                TYPE_I64 => ValType::I64,
                TYPE_F32 => ValType::F32,
                TYPE_F64 => ValType::F64,
                #[cfg(feature = "object-registry-extern")]
                TYPE_VARIANT => ValType::Ref(RefType::EXTERNREF),
                v => bail_with_site!("Unknown enumeration value {}", v),
            });
        }

        Ok(ret)
    }

    let p = match VariantDispatch::from(&params) {
        VariantDispatch::VariantArray(v) => f(v
            .into_iter()
            .map(|v| Ok(site_context!(u32::from_variant(&v))?))),
        VariantDispatch::ByteArray(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        VariantDispatch::Int32Array(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        _ => bail!("Unconvertible value {}", params),
    }?;

    let r = match VariantDispatch::from(&results) {
        VariantDispatch::VariantArray(v) => f(v
            .into_iter()
            .map(|v| Ok(site_context!(u32::from_variant(&v))?))),
        VariantDispatch::ByteArray(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        VariantDispatch::Int32Array(v) => f(v.read().as_slice().iter().map(|v| Ok(*v as u32))),
        _ => bail!("Unconvertible value {}", results),
    }?;

    Ok(FuncType::new(&ENGINE, p, r))
}

// Mark this unsafe for future proofing
pub unsafe fn to_raw(_store: impl AsContextMut, t: ValType, v: Variant) -> Result<ValRaw, Error> {
    Ok(match t {
        ValType::I32 => ValRaw::i32(site_context!(i32::from_variant(&v))?),
        ValType::I64 => ValRaw::i64(site_context!(i64::from_variant(&v))?),
        ValType::F32 => ValRaw::f32(site_context!(f32::from_variant(&v))?.to_bits()),
        ValType::F64 => ValRaw::f64(site_context!(f64::from_variant(&v))?.to_bits()),
        #[cfg(feature = "object-registry-extern")]
        ValType::Ref(r) if RefType::eq(&r, &RefType::EXTERNREF) => {
            ValRaw::externref(match variant_to_externref(v) {
                Some(v) => v.to_raw(_store),
                None => ptr::null_mut(),
            })
        }
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

// Mark this unsafe for future proofing
pub unsafe fn from_raw(_store: impl AsContextMut, t: ValType, v: ValRaw) -> Result<Variant, Error> {
    Ok(match t {
        ValType::I32 => v.get_i32().to_variant(),
        ValType::I64 => v.get_i64().to_variant(),
        ValType::F32 => f32::from_bits(v.get_f32()).to_variant(),
        ValType::F64 => f64::from_bits(v.get_f64()).to_variant(),
        #[cfg(feature = "object-registry-extern")]
        ValType::Ref(r) if RefType::eq(&r, &RefType::EXTERNREF) => {
            externref_to_variant(ExternRef::from_raw(v.get_externref()))
        }
        _ => bail_with_site!("Unsupported WASM type conversion {}", t),
    })
}

fn wrap_godot_method<T>(
    store: impl AsContextMut<Data = T>,
    ty: FuncType,
    obj: Variant,
    method: GodotString,
) -> Func
where
    T: AsRef<StoreData> + AsMut<StoreData>,
{
    let ty_cloned = ty.clone();
    let f = move |mut ctx: Caller<T>, args: &mut [ValRaw]| -> Result<(), Error> {
        let pi = ty.params();
        let mut p = Vec::with_capacity(pi.len());
        for (ix, t) in pi.enumerate() {
            p.push(unsafe { from_raw(&mut ctx, t, args[ix])? });
        }

        let mut obj = match <Ref<WeakRef, Shared>>::from_variant(&obj) {
            Ok(obj) => unsafe { obj.assume_safe().get_ref() },
            Err(_) => obj.clone(),
        };
        let r = ctx
            .data_mut()
            .as_mut()
            .release_store(|| unsafe { site_context!(obj.call(method.clone(), &p)) })?;

        if let Some(msg) = ctx.data_mut().as_mut().error_signal.take() {
            return Err(Error::msg(msg));
        }

        let mut ri = ty.results();
        if ri.len() == 0 {
        } else if let Ok(r) = VariantArray::from_variant(&r) {
            for (ix, t) in ri.enumerate() {
                let v = r.get(ix as _);
                args[ix] = unsafe { to_raw(&mut ctx, t, v)? };
            }
        } else if ri.len() == 1 {
            args[0] = unsafe { to_raw(&mut ctx, ri.next().unwrap(), r)? };
        } else {
            bail_with_site!("Unconvertible return value {}", r);
        }

        #[cfg(feature = "epoch-timeout")]
        if let Config {
            with_epoch: true,
            epoch_autoreset: true,
            epoch_timeout,
            ..
        } = ctx.data().as_ref().config
        {
            ctx.as_context_mut().set_epoch_deadline(epoch_timeout);
        }

        Ok(())
    };

    unsafe { Func::new_unchecked(store, ty_cloned, f) }
}

#[cfg(feature = "new-host-import")]
pub struct HostModuleCache<T> {
    cache: Linker<T>,
    host: Dictionary,
}

#[cfg(not(feature = "new-host-import"))]
pub struct HostModuleCache<T> {
    cache: HashMap<String, Extern>,
    host: Dictionary,
    phantom: PhantomData<T>,
}

impl<T: AsRef<StoreData> + AsMut<StoreData>> HostModuleCache<T> {
    pub fn new(host: Dictionary) -> Self {
        #[cfg(feature = "new-host-import")]
        {
            Self {
                cache: Linker::new(&ENGINE),
                host,
            }
        }

        #[cfg(not(feature = "new-host-import"))]
        {
            Self {
                cache: HashMap::new(),
                host,
                phantom: PhantomData,
            }
        }
    }

    pub fn get_extern<S: AsContextMut<Data = T>>(
        &mut self,
        store: &mut S,
        module: &str,
        name: &str,
    ) -> Result<Option<Extern>, Error> {
        #[derive(FromVariant)]
        struct Data {
            params: Variant,
            results: Variant,
            object: Variant,
            method: GodotString,
        }

        #[cfg(feature = "new-host-import")]
        if let r @ Some(_) = self.cache.get(&mut *store, module, name) {
            Ok(r)
        } else if let Some(data) = self
            .host
            .get(module)
            .map(|d| site_context!(Dictionary::from_variant(&d)))
            .transpose()?
            .and_then(|d| d.get(name))
        {
            let data = site_context!(Data::from_variant(&data))?;

            let obj = match <Ref<WeakRef, Shared>>::from_variant(&data.object) {
                Ok(obj) => unsafe { obj.assume_safe().get_ref() },
                Err(_) => data.object.clone(),
            };
            if !obj.has_method(data.method.clone()) {
                bail!("Object {} has no method {}", obj, data.method);
            }

            let v = Extern::from(wrap_godot_method(
                &mut *store,
                to_signature(data.params, data.results)?,
                data.object,
                data.method,
            ));
            self.cache.define(store, module, name, v.clone())?;
            Ok(Some(v))
        } else {
            Ok(None)
        }

        #[cfg(not(feature = "new-host-import"))]
        if module != HOST_MODULE {
            Ok(None)
        } else if let r @ Some(_) = self.cache.get(name).cloned() {
            Ok(r)
        } else if let Some(data) = self.host.get(name) {
            let data = site_context!(Data::from_variant(&data))?;

            let obj = match <Ref<WeakRef, Shared>>::from_variant(&data.object) {
                Ok(obj) => unsafe { obj.assume_safe().get_ref() },
                Err(_) => data.object.clone(),
            };
            if !obj.has_method(data.method.clone()) {
                bail!("Object {} has no method {}", obj, data.method);
            }

            let v = Extern::from(wrap_godot_method(
                store,
                to_signature(data.params, data.results)?,
                data.object,
                data.method,
            ));
            self.cache.insert(name.to_string(), v.clone());
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }
}

pub fn config_store_common<T>(store: &mut Store<T>) -> Result<(), Error>
where
    T: AsRef<StoreData> + AsMut<StoreData>,
{
    #[cfg(feature = "epoch-timeout")]
    if store.data().as_ref().config.with_epoch {
        store.epoch_deadline_trap();
        EPOCH.spawn_thread(|| ENGINE.increment_epoch());
    } else {
        store.epoch_deadline_callback(|_| Ok(UpdateDeadline::Continue(EPOCH_DEADLINE)));
    }

    #[cfg(feature = "memory-limiter")]
    {
        let data = store.data_mut().as_mut();
        if let Some(v) = data.config.max_memory {
            data.memory_limits.max_memory = v;
        }
        if let Some(v) = data.config.max_entries {
            data.memory_limits.max_table_entries = v;
        }
        store.limiter(|data| &mut data.as_mut().memory_limits);
    }

    Ok(())
}
