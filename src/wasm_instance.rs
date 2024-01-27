use std::any::Any;
use std::collections::HashMap;
use std::iter::Peekable;
use std::ptr;
use std::sync::Arc;

use anyhow::{bail, Error};
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, RawMutex};
use rayon::prelude::*;
use scopeguard::guard;
#[cfg(feature = "wasi-preview2")]
use wasmtime::component::Instance as InstanceComp;
#[cfg(feature = "wasi")]
use wasmtime::Linker;
#[cfg(feature = "memory-limiter")]
use wasmtime::ResourceLimiter;
use wasmtime::{
    AsContextMut, Extern, Instance as InstanceWasm, Memory, Store, StoreContextMut, ValRaw,
};
#[cfg(feature = "wasi-preview2")]
use wasmtime_wasi::preview2::{Table as WasiTable, WasiCtx as WasiCtxPv2, WasiView};
#[cfg(feature = "wasi")]
use wasmtime_wasi::sync::{add_to_linker, WasiCtxBuilder};
#[cfg(feature = "wasi")]
use wasmtime_wasi::WasiCtx;

#[cfg(feature = "wasi")]
use crate::wasi_ctx::stdio::{
    BlockWritePipe, ByteBufferReadPipe, InnerStdin, LineWritePipe, OuterStdin, UnbufferedWritePipe,
};
#[cfg(feature = "wasi")]
use crate::wasi_ctx::WasiContext;
use crate::wasm_config::Config;
#[cfg(any(feature = "object-registry-compat", feature = "object-registry-extern"))]
use crate::wasm_config::ExternBindingType;
#[cfg(feature = "wasi")]
use crate::wasm_config::{PipeBindingType, PipeBufferType};
use crate::wasm_engine::{ModuleData, ModuleType, WasmModule, ENGINE};
#[cfg(feature = "object-registry-extern")]
use crate::wasm_externref::Funcs as ExternrefFuncs;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_objregistry::{Funcs as ObjregistryFuncs, ObjectRegistry};
#[cfg(feature = "object-registry-extern")]
use crate::wasm_util::EXTERNREF_MODULE;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_util::OBJREGISTRY_MODULE;
use crate::wasm_util::{
    config_store_common, from_raw, option_to_variant, to_raw, variant_to_option, HostModuleCache,
    VariantDispatch, MEMORY_EXPORT,
};
use crate::{bail_with_site, site_context};

#[derive(GodotClass)]
#[class(base=RefCounted, init, tool)]
pub struct WasmInstance {
    base: Base<RefCounted>,
    data: OnceCell<InstanceData<StoreData>>,

    #[var(get = get_module)]
    #[allow(dead_code)]
    module: Option<Gd<WasmModule>>,
}

pub struct InstanceData<T> {
    pub store: Mutex<Store<T>>,
    pub instance: InstanceType,
    pub module: Gd<WasmModule>,

    #[cfg(feature = "wasi")]
    pub wasi_stdin: Option<Arc<InnerStdin<dyn Any + Send + Sync>>>,
}

pub enum InstanceType {
    Core(InstanceWasm),
    #[cfg(feature = "wasi-preview2")]
    Component(InstanceComp),
}

impl InstanceType {
    pub fn get_core(&self) -> Result<&InstanceWasm, Error> {
        #[allow(irrefutable_let_patterns)]
        if let Self::Core(m) = self {
            Ok(m)
        } else {
            bail!("Instance is a component")
        }
    }

    #[cfg(feature = "wasi-preview2")]
    pub fn get_component(&self) -> Result<&InstanceComp, Error> {
        if let Self::Component(m) = self {
            Ok(m)
        } else {
            bail!("Instance is a component")
        }
    }
}

pub struct StoreData {
    mutex_raw: *const RawMutex,
    pub config: Config,
    pub error_signal: Option<String>,

    #[cfg(feature = "memory-limiter")]
    pub memory_limits: MemoryLimit,

    #[cfg(feature = "object-registry-compat")]
    pub object_registry: Option<ObjectRegistry>,

    #[cfg(feature = "wasi")]
    pub wasi_ctx: MaybeWasi,
}

// SAFETY: Store data is safely contained within instance data?
unsafe impl Send for StoreData {}
unsafe impl Sync for StoreData {}

impl AsRef<Self> for StoreData {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsMut<Self> for StoreData {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl Default for StoreData {
    fn default() -> Self {
        Self {
            mutex_raw: ptr::null(),
            config: Config::default(),
            error_signal: None,

            #[cfg(feature = "memory-limiter")]
            memory_limits: MemoryLimit {
                max_memory: u64::MAX,
                max_table_entries: u64::MAX,
            },

            #[cfg(feature = "object-registry-compat")]
            object_registry: None,

            #[cfg(feature = "wasi")]
            wasi_ctx: MaybeWasi::NoCtx,
        }
    }
}

impl StoreData {
    #[allow(dead_code)]
    pub fn new(config: Config) -> Self {
        Self {
            mutex_raw: ptr::null(),
            config,
            error_signal: None,

            #[cfg(feature = "memory-limiter")]
            memory_limits: MemoryLimit {
                max_memory: u64::MAX,
                max_table_entries: u64::MAX,
            },

            #[cfg(feature = "object-registry-compat")]
            object_registry: None,

            #[cfg(feature = "wasi")]
            wasi_ctx: MaybeWasi::NoCtx,
        }
    }
}

pub enum MaybeWasi {
    NoCtx,
    Preview1(WasiCtx),
    #[cfg(feature = "wasi-preview2")]
    Preview2(WasiCtxPv2, WasiTable),
}

#[cfg(feature = "wasi-preview2")]
impl WasiView for StoreData {
    fn table(&self) -> &WasiTable {
        match &self.wasi_ctx {
            MaybeWasi::Preview2(_, tbl) => tbl,
            _ => panic!("Requested WASI Preview 2 interface while none set, this is a bug"),
        }
    }

    fn table_mut(&mut self) -> &mut WasiTable {
        match &mut self.wasi_ctx {
            MaybeWasi::Preview2(_, tbl) => tbl,
            _ => panic!("Requested WASI Preview 2 interface while none set, this is a bug"),
        }
    }

    fn ctx(&self) -> &WasiCtxPv2 {
        match &self.wasi_ctx {
            MaybeWasi::Preview2(ctx, _) => ctx,
            _ => panic!("Requested WASI Preview 2 interface while none set, this is a bug"),
        }
    }

    fn ctx_mut(&mut self) -> &mut WasiCtxPv2 {
        match &mut self.wasi_ctx {
            MaybeWasi::Preview2(ctx, _) => ctx,
            _ => panic!("Requested WASI Preview 2 interface while none set, this is a bug"),
        }
    }
}

#[cfg(feature = "memory-limiter")]
pub struct MemoryLimit {
    pub max_memory: u64,
    pub max_table_entries: u64,
}

#[cfg(feature = "memory-limiter")]
impl ResourceLimiter for MemoryLimit {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _: Option<usize>,
    ) -> Result<bool, Error> {
        if self.max_memory == u64::MAX {
            return Ok(true);
        }

        let delta = (desired - current) as u64;
        if let Some(v) = self.max_memory.checked_sub(delta) {
            self.max_memory = v;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn table_growing(&mut self, current: u32, desired: u32, _: Option<u32>) -> Result<bool, Error> {
        if self.max_table_entries == u64::MAX {
            return Ok(true);
        }

        let delta = (desired - current) as u64;
        if let Some(v) = self.max_table_entries.checked_sub(delta) {
            self.max_table_entries = v;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl<T> InstanceData<T>
where
    T: AsRef<StoreData> + AsMut<StoreData>,
{
    pub fn instantiate(
        mut store: Store<T>,
        module: Gd<WasmModule>,
        host: Option<Dictionary>,
    ) -> Result<Self, Error> {
        config_store_common(&mut store)?;

        #[cfg(feature = "wasi")]
        let mut wasi_stdin = None;

        #[cfg(feature = "wasi")]
        let wasi_linker = if store.data().as_ref().config.with_wasi {
            let mut builder = WasiCtxBuilder::new();

            let StoreData {
                wasi_ctx, config, ..
            } = store.data_mut().as_mut();

            if config.wasi_stdin == PipeBindingType::Instance {
                if let Some(data) = config.wasi_stdin_data.clone() {
                    builder.stdin(Box::new(ByteBufferReadPipe::new(data)));
                } else {
                    // TODO: Emit signal
                    let (outer, inner) = OuterStdin::new(move || {});
                    builder.stdin(Box::new(outer));
                    wasi_stdin = Some(inner as _);
                }
            }
            if config.wasi_stdout == PipeBindingType::Instance {
                builder.stdout(match config.wasi_stdout_buffer {
                    PipeBufferType::Unbuffered => {
                        Box::new(UnbufferedWritePipe::new(move |_buf| {})) as _
                    }
                    PipeBufferType::LineBuffer => Box::new(LineWritePipe::new(move |_buf| {})) as _,
                    PipeBufferType::BlockBuffer => {
                        Box::new(BlockWritePipe::new(move |_buf| {})) as _
                    }
                });
            }
            if config.wasi_stderr == PipeBindingType::Instance {
                builder.stderr(match config.wasi_stderr_buffer {
                    PipeBufferType::Unbuffered => {
                        Box::new(UnbufferedWritePipe::new(move |_buf| {})) as _
                    }
                    PipeBufferType::LineBuffer => Box::new(LineWritePipe::new(move |_buf| {})) as _,
                    PipeBufferType::BlockBuffer => {
                        Box::new(BlockWritePipe::new(move |_buf| {})) as _
                    }
                });
            }

            *wasi_ctx = match &config.wasi_context {
                Some(ctx) => {
                    MaybeWasi::Preview1(WasiContext::build_ctx(ctx.clone(), builder, &*config)?)
                }
                None => MaybeWasi::Preview1(WasiContext::init_ctx_no_context(
                    builder.inherit_stdout().inherit_stderr().build(),
                    &*config,
                )?),
            };
            let mut r = <Linker<T>>::new(&ENGINE);
            add_to_linker(&mut r, |data| match &mut data.as_mut().wasi_ctx {
                MaybeWasi::Preview1(ctx) => ctx,
                _ => panic!("Requested WASI Preview 1 interface while none set, this is a bug"),
            })?;
            Some(r)
        } else {
            None
        };

        #[cfg(feature = "object-registry-compat")]
        if store.data().as_ref().config.extern_bind == ExternBindingType::Registry {
            store.data_mut().as_mut().object_registry = Some(ObjectRegistry::default());
        }

        let sp = &mut store;
        let instance = Self::instantiate_wasm(
            sp,
            module.bind().get_data()?,
            &mut HashMap::new(),
            &mut host.map(HostModuleCache::new),
            #[cfg(feature = "object-registry-compat")]
            &mut ObjregistryFuncs::default(),
            #[cfg(feature = "object-registry-extern")]
            &mut ExternrefFuncs::default(),
            #[cfg(feature = "wasi")]
            wasi_linker.as_ref(),
        )?;

        Ok(Self {
            instance: InstanceType::Core(instance),
            module,
            store: Mutex::new(store),
            #[cfg(feature = "wasi")]
            wasi_stdin,
        })
    }

    fn instantiate_wasm(
        store: &mut Store<T>,
        module: &ModuleData,
        insts: &mut HashMap<InstanceId, InstanceWasm>,
        host: &mut Option<HostModuleCache<T>>,
        #[cfg(feature = "object-registry-compat")] objregistry_funcs: &mut ObjregistryFuncs,
        #[cfg(feature = "object-registry-extern")] externref_funcs: &mut ExternrefFuncs,
        #[cfg(feature = "wasi")] wasi_linker: Option<&Linker<T>>,
    ) -> Result<InstanceWasm, Error> {
        #[allow(irrefutable_let_patterns)]
        let ModuleType::Core(module_) = &module.module
        else {
            bail_with_site!("Cannot instantiate component")
        };
        let it = module_.imports();
        let mut imports = Vec::with_capacity(it.len());

        for i in it {
            if let Some(v) = host
                .as_mut()
                .and_then(|v| v.get_extern(&mut *store, i.module(), i.name()).transpose())
                .transpose()?
            {
                imports.push(v);
                continue;
            }

            match (i.module(), &store.data().as_ref().config) {
                #[cfg(feature = "object-registry-compat")]
                (
                    OBJREGISTRY_MODULE,
                    Config {
                        extern_bind: ExternBindingType::Registry,
                        ..
                    },
                ) => {
                    if let Some(v) =
                        objregistry_funcs.get_func(&mut store.as_context_mut(), i.name())
                    {
                        imports.push(v.into());
                        continue;
                    }
                }
                #[cfg(feature = "object-registry-extern")]
                (
                    EXTERNREF_MODULE,
                    Config {
                        extern_bind: ExternBindingType::Native,
                        ..
                    },
                ) => {
                    if let Some(v) = externref_funcs.get_func(&mut store.as_context_mut(), i.name())
                    {
                        imports.push(v.into());
                        continue;
                    }
                }
                _ => (),
            }

            #[cfg(feature = "wasi")]
            if let Some(l) = wasi_linker.as_ref() {
                if let Some(v) = l.get_by_import(&mut *store, &i) {
                    imports.push(v);
                    continue;
                }
            }

            if let Some(v) = module.imports.get(i.module()) {
                let v = loop {
                    match insts.get(&v.instance_id()) {
                        Some(v) => break v,
                        None => {
                            let t = Self::instantiate_wasm(
                                &mut *store,
                                v.bind().get_data()?,
                                &mut *insts,
                                &mut *host,
                                #[cfg(feature = "object-registry-compat")]
                                &mut *objregistry_funcs,
                                #[cfg(feature = "object-registry-extern")]
                                &mut *externref_funcs,
                                #[cfg(feature = "wasi")]
                                wasi_linker,
                            )?;
                            insts.insert(v.instance_id(), t);
                        }
                    }
                };

                if let Some(v) = v.get_export(&mut *store, i.name()) {
                    imports.push(v.clone());
                    continue;
                }
            }

            bail_with_site!("Unknown import {:?}.{:?}", i.module(), i.name());
        }

        #[cfg(feature = "epoch-timeout")]
        store.set_epoch_deadline(store.data().as_ref().config.epoch_timeout);
        InstanceWasm::new(store, module_, &imports)
    }

    pub fn acquire_store<F, R>(&self, f: F) -> R
    where
        for<'a> F: FnOnce(&Self, StoreContextMut<'a, T>) -> R,
    {
        let mut guard_ = self.store.lock();

        let _scope;
        // SAFETY: Context should be destroyed after function call
        unsafe {
            let p = &mut guard_.data_mut().as_mut().mutex_raw as *mut _;
            let mut v = self.store.raw() as *const _;
            ptr::swap(p, &mut v);
            _scope = guard(p, move |p| {
                *p = v;
            });
        }

        f(self, guard_.as_context_mut())
    }
}

impl StoreData {
    pub(crate) fn release_store<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard;
        if !self.mutex_raw.is_null() {
            // SAFETY: Pointer is valid and locked mutex
            unsafe {
                _guard = guard(&*self.mutex_raw, |v| v.lock());
                _guard.unlock();
            }
        }

        f()
    }

    #[cfg(feature = "object-registry-compat")]
    pub fn get_registry(&self) -> Result<&ObjectRegistry, Error> {
        site_context!(self
            .object_registry
            .as_ref()
            .ok_or_else(|| Error::msg("Object registry not enabled!")))
    }

    #[cfg(feature = "object-registry-compat")]
    pub fn get_registry_mut(&mut self) -> Result<&mut ObjectRegistry, Error> {
        site_context!(self
            .object_registry
            .as_mut()
            .ok_or_else(|| Error::msg("Object registry not enabled!")))
    }
}

impl WasmInstance {
    pub fn get_data(&self) -> Result<&InstanceData<StoreData>, Error> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized instance")
        }
    }

    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&InstanceData<StoreData>) -> Result<R, Error>,
    {
        match self.get_data().and_then(f) {
            Ok(v) => Some(v),
            Err(e) => {
                /*
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    &s,
                );
                */
                godot_error!("{:?}", e);
                /*
                self.base.emit_signal(
                    StringName::from("error_happened"),
                    &[format!("{}", e).to_variant()],
                );
                */
                None
            }
        }
    }

    pub fn initialize_(
        &self,
        module: Gd<WasmModule>,
        host: Option<Dictionary>,
        config: Option<Variant>,
    ) -> bool {
        match self.data.get_or_try_init(move || {
            InstanceData::instantiate(
                Store::new(
                    &ENGINE,
                    StoreData {
                        mutex_raw: ptr::null(),
                        config: match config {
                            Some(v) => match Config::try_from_variant(&v) {
                                Ok(v) => v,
                                Err(e) => {
                                    godot_error!("{:?}", e);
                                    Config::default()
                                }
                            },
                            None => Config::default(),
                        },
                        error_signal: None,

                        #[cfg(feature = "memory-limiter")]
                        memory_limits: MemoryLimit {
                            max_memory: u64::MAX,
                            max_table_entries: u64::MAX,
                        },

                        #[cfg(feature = "object-registry-compat")]
                        object_registry: None,

                        #[cfg(feature = "wasi")]
                        wasi_ctx: MaybeWasi::NoCtx,
                    },
                ),
                module,
                host,
            )
        }) {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    fn get_memory<F, R>(&self, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(StoreContextMut<'a, StoreData>, Memory) -> Result<R, Error>,
    {
        self.unwrap_data(|m| {
            m.acquire_store(|m, mut store| {
                match site_context!(m.instance.get_core())?.get_memory(&mut store, MEMORY_EXPORT) {
                    Some(mem) => f(store, mem),
                    None => bail_with_site!("No memory exported"),
                }
            })
        })
    }

    fn read_memory<F, R>(&self, i: usize, n: usize, f: F) -> Option<R>
    where
        F: FnOnce(&[u8]) -> Result<R, Error>,
    {
        self.get_memory(|store, mem| {
            let data = mem.data(&store);
            match data.get(i..i + n) {
                Some(s) => f(s),
                None => bail_with_site!("Index out of bound {}-{}", i, i + n),
            }
        })
    }

    fn write_memory<F, R>(&self, i: usize, n: usize, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(&'a mut [u8]) -> Result<R, Error>,
    {
        self.get_memory(|mut store, mem| {
            let data = mem.data_mut(&mut store);
            match data.get_mut(i..i + n) {
                Some(s) => f(s),
                None => bail_with_site!("Index out of bound {}-{}", i, i + n),
            }
        })
    }
}

#[godot_api]
impl WasmInstance {
    #[signal]
    fn error_happened();

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[func]
    fn initialize(
        &self,
        module: Gd<WasmModule>,
        host: Variant,
        config: Variant,
    ) -> Option<Gd<WasmInstance>> {
        let Ok(host) = variant_to_option::<Dictionary>(host) else {
            godot_error!("Host is not a dictionary!");
            return None;
        };
        let config = if config.is_nil() { None } else { Some(config) };

        if self.initialize_(module, host, config) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    #[func]
    fn get_module(&self) -> Option<Gd<WasmModule>> {
        self.unwrap_data(|m| Ok(m.module.clone()))
    }

    #[func]
    fn call_wasm(&self, name: StringName, args: Array<Variant>) -> Array<Variant> {
        self.unwrap_data(move |m| {
            m.acquire_store(move |m, mut store| {
                let name = name.to_string();
                let f = match site_context!(m.instance.get_core())?.get_export(&mut store, &name) {
                    Some(f) => match f {
                        Extern::Func(f) => f,
                        _ => bail_with_site!("Export {} is not a function", &name),
                    },
                    None => bail_with_site!("Export {} does not exists", &name),
                };

                store.gc();

                let ty = f.ty(&store);
                let pi = ty.params();
                let ri = ty.results();
                let mut arr = Vec::with_capacity(pi.len().max(ri.len()));

                let pl = pi.len();
                for (t, v) in pi.zip(args.iter_shared()) {
                    arr.push(unsafe { to_raw(&mut store, t, v)? });
                }
                if arr.len() != pl {
                    bail_with_site!("Too few parameter (expected {}, got {})", pl, arr.len());
                }
                while arr.len() < ri.len() {
                    arr.push(ValRaw::i32(0));
                }

                #[cfg(feature = "epoch-timeout")]
                store.set_epoch_deadline(store.data().config.epoch_timeout);

                // SAFETY: Array length is maximum of params and returns and initialized
                unsafe {
                    site_context!(f.call_unchecked(&mut store, arr.as_mut_ptr(), arr.len()))?;
                }

                let mut ret = Array::new();
                for (t, v) in ri.zip(arr) {
                    ret.push(unsafe { from_raw(&mut store, t, v)? });
                }

                Ok(ret)
            })
        })
        .unwrap_or_default()
    }

    /// Emit trap when returning from host. Only used for host binding.
    /// Returns previous error message, if any.
    #[func]
    fn signal_error(&self, msg: GString) -> Variant {
        option_to_variant(
            self.unwrap_data(|m| {
                m.acquire_store(|_, mut store| {
                    Ok(store.data_mut().error_signal.replace(msg.to_string()))
                })
            })
            .flatten(),
        )
    }

    /// Cancel effect of signal_error.
    /// Returns previous error message, if any.
    #[func]
    fn signal_error_cancel(&self) -> Variant {
        option_to_variant(
            self.unwrap_data(|m| {
                m.acquire_store(|_, mut store| Ok(store.data_mut().error_signal.take()))
            })
            .flatten(),
        )
    }

    #[func]
    fn reset_epoch(&self) {
        #[cfg(feature = "epoch-timeout")]
        self.unwrap_data(|m| {
            m.acquire_store(|_, mut store| {
                store.set_epoch_deadline(store.data().config.epoch_timeout);
                Ok(())
            })
        });

        #[cfg(not(feature = "epoch-timeout"))]
        godot_error!("Feature epoch-timeout not enabled!");
    }

    #[func]
    fn register_object(&self, _obj: Variant) -> Variant {
        #[cfg(feature = "object-registry-compat")]
        return option_to_variant(self.unwrap_data(|m| {
            if _obj.is_nil() {
                bail_with_site!("Value is null!");
            }
            m.acquire_store(|_, mut store| Ok(store.data_mut().get_registry_mut()?.register(_obj)))
        }));

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            Variant::nil()
        }
    }

    #[func]
    fn registry_get(&self, _ix: i64) -> Variant {
        #[cfg(feature = "object-registry-compat")]
        return option_to_variant(
            self.unwrap_data(|m| {
                m.acquire_store(|_, store| {
                    Ok(store.data().get_registry()?.get(usize::try_from(_ix)?))
                })
            })
            .flatten(),
        );

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            Variant::nil()
        }
    }

    #[func]
    fn registry_set(&self, _ix: i64, _obj: Variant) -> Variant {
        #[cfg(feature = "object-registry-compat")]
        return option_to_variant(
            self.unwrap_data(|m| {
                m.acquire_store(|_, mut store| {
                    let _ix = usize::try_from(_ix)?;
                    let reg = store.data_mut().get_registry_mut()?;
                    if _obj.is_nil() {
                        Ok(reg.unregister(_ix))
                    } else {
                        Ok(reg.replace(_ix, _obj))
                    }
                })
            })
            .flatten(),
        );

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            Variant::nil()
        }
    }

    #[func]
    fn unregister_object(&self, _ix: i64) -> Variant {
        #[cfg(feature = "object-registry-compat")]
        return option_to_variant(
            self.unwrap_data(|m| {
                m.acquire_store(|_, mut store| {
                    Ok(store
                        .data_mut()
                        .get_registry_mut()?
                        .unregister(usize::try_from(_ix)?))
                })
            })
            .flatten(),
        );

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            Variant::nil()
        }
    }

    #[func]
    fn has_memory(&self) -> bool {
        self.unwrap_data(|m| {
            m.acquire_store(|m, mut store| {
                Ok(matches!(
                    site_context!(m.instance.get_core())?.get_export(&mut store, MEMORY_EXPORT),
                    Some(Extern::Memory(_))
                ))
            })
        })
        .unwrap_or_default()
    }

    #[func]
    fn stdin_add_line(&self, line: GString) {
        #[cfg(feature = "wasi")]
        self.unwrap_data(|m| {
            if let Some(stdin) = &m.wasi_stdin {
                stdin.add_line(line)?;
            }
            Ok(())
        });

        #[cfg(not(feature = "wasi"))]
        godot_error!("Feature wasi not enabled!");
    }

    #[func]
    fn stdin_close(&self) {
        #[cfg(feature = "wasi")]
        self.unwrap_data(|m| {
            if let Some(stdin) = &m.wasi_stdin {
                stdin.close_pipe();
            }
            Ok(())
        });

        #[cfg(not(feature = "wasi"))]
        godot_error!("Feature wasi not enabled!");
    }

    #[func]
    fn memory_size(&self) -> i64 {
        self.get_memory(|store, mem| Ok(mem.data_size(store) as i64))
            .unwrap_or_default()
    }

    #[func]
    fn memory_read(&self, i: i64, n: i64) -> PackedByteArray {
        self.read_memory(i as _, n as _, |s| Ok(PackedByteArray::from(s)))
            .unwrap_or_default()
    }

    #[func]
    fn memory_write(&self, i: i64, a: PackedByteArray) -> bool {
        let a = a.to_vec();
        self.write_memory(i as _, a.len(), |s| {
            s.copy_from_slice(&a);
            Ok(())
        })
        .is_some()
    }

    #[func]
    fn get_8(&self, i: i64) -> i64 {
        self.read_memory(i as _, 1, |s| Ok(s[0]))
            .unwrap_or_default()
            .into()
    }

    #[func]
    fn put_8(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 1, |s| {
            s[0] = (v & 255) as _;
            Ok(())
        })
        .is_some()
    }

    #[func]
    fn get_16(&self, i: i64) -> i64 {
        self.read_memory(i as _, 2, |s| Ok(u16::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    #[func]
    fn put_16(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 2, |s| {
            s.copy_from_slice(&((v & 0xffff) as u16).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[func]
    fn get_32(&self, i: i64) -> i64 {
        self.read_memory(i as _, 4, |s| Ok(u32::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    #[func]
    fn put_32(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 4, |s| {
            s.copy_from_slice(&((v & 0xffffffff) as u32).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[func]
    fn get_64(&self, i: i64) -> i64 {
        self.read_memory(i as _, 8, |s| Ok(i64::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
    }

    #[func]
    fn put_64(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[func]
    fn get_float(&self, i: i64) -> f64 {
        self.read_memory(i as _, 4, |s| Ok(f32::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    #[func]
    fn put_float(&self, i: i64, v: f64) -> bool {
        self.write_memory(i as _, 4, |s| {
            s.copy_from_slice(&(v as f32).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[func]
    fn get_double(&self, i: i64) -> f64 {
        self.read_memory(i as _, 8, |s| Ok(f64::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
    }

    #[func]
    fn put_double(&self, i: i64, v: f64) -> bool {
        self.write_memory(i as _, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[func]
    fn put_array(&self, i: i64, v: Variant) -> bool {
        fn f<const N: usize, T: Sync>(
            d: &mut [u8],
            i: usize,
            s: &[T],
            f: impl Fn(&T, &mut [u8; N]) + Send + Sync,
        ) -> Result<(), Error> {
            let e = i + s.len() * N;
            let Some(d) = d.get_mut(i..e) else {
                bail_with_site!("Index out of range ({}..{})", i, e);
            };

            s.par_iter()
                .zip(d.par_chunks_exact_mut(N))
                .for_each(|(s, d)| f(s, d.try_into().unwrap()));

            Ok(())
        }

        self.get_memory(|mut store, mem| {
            let i = i as usize;
            let data = mem.data_mut(&mut store);
            match VariantDispatch::from(&v) {
                VariantDispatch::PackedByteArray(v) => {
                    let s = v.as_slice();
                    let e = i + s.len();
                    let Some(d) = data.get_mut(i..e) else {
                        bail_with_site!("Index out of range ({}..{})", i, e);
                    };

                    d.copy_from_slice(s);
                    Ok(())
                }
                VariantDispatch::PackedInt32Array(v) => {
                    f::<4, _>(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes())
                }
                VariantDispatch::PackedInt64Array(v) => {
                    f::<8, _>(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes())
                }
                VariantDispatch::PackedFloat32Array(v) => {
                    f::<4, _>(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes())
                }
                VariantDispatch::PackedFloat64Array(v) => {
                    f::<8, _>(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes())
                }
                VariantDispatch::PackedVector2Array(v) => {
                    f::<8, _>(data, i, v.as_slice(), |s, d| {
                        *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                        *<&mut [u8; 4]>::try_from(&mut d[4..]).unwrap() = s.y.to_le_bytes();
                    })
                }
                VariantDispatch::PackedVector3Array(v) => {
                    f::<12, _>(data, i, v.as_slice(), |s, d| {
                        *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                        *<&mut [u8; 4]>::try_from(&mut d[4..8]).unwrap() = s.y.to_le_bytes();
                        *<&mut [u8; 4]>::try_from(&mut d[8..]).unwrap() = s.z.to_le_bytes();
                    })
                }
                VariantDispatch::PackedColorArray(v) => {
                    f::<16, _>(data, i, v.as_slice(), |s, d| {
                        *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.r.to_le_bytes();
                        *<&mut [u8; 4]>::try_from(&mut d[4..8]).unwrap() = s.g.to_le_bytes();
                        *<&mut [u8; 4]>::try_from(&mut d[8..12]).unwrap() = s.b.to_le_bytes();
                        *<&mut [u8; 4]>::try_from(&mut d[12..]).unwrap() = s.a.to_le_bytes();
                    })
                }
                _ => bail_with_site!("Unknown value type {:?}", v.get_type()),
            }
        })
        .is_some()
    }

    #[func]
    fn get_array(&self, i: i64, n: i64, t: i64) -> Variant {
        fn f<const N: usize, T: Send, R: for<'a> From<&'a [T]>>(
            s: &[u8],
            i: usize,
            n: usize,
            f: impl Fn(&[u8; N]) -> T + Send + Sync,
        ) -> Result<R, Error> {
            let e = i + n * N;
            let Some(s) = s.get(i..e) else {
                bail_with_site!("Index out of range ({}..{})", i, e);
            };

            Ok(R::from(
                &s.par_chunks_exact(N)
                    .map(|s| f(s.try_into().unwrap()))
                    .collect::<Vec<_>>(),
            ))
        }

        option_to_variant(self.get_memory(|store, mem| {
            let (i, n) = (i as usize, n as usize);
            let data = mem.data(&store);
            match t {
                29 => {
                    let e = i + n;
                    let Some(s) = data.get(i..e) else {
                        bail_with_site!("Index out of range ({}..{})", i, e);
                    };

                    Ok(Variant::from(PackedByteArray::from(s)))
                }
                30 => Ok(Variant::from(f::<4, _, PackedInt32Array>(
                    data,
                    i,
                    n,
                    |s| i32::from_le_bytes(*s),
                )?)),
                31 => Ok(Variant::from(f::<8, _, PackedInt64Array>(
                    data,
                    i,
                    n,
                    |s| i64::from_le_bytes(*s),
                )?)),
                32 => Ok(Variant::from(f::<4, _, PackedFloat32Array>(
                    data,
                    i,
                    n,
                    |s| f32::from_le_bytes(*s),
                )?)),
                33 => Ok(Variant::from(f::<8, _, PackedFloat64Array>(
                    data,
                    i,
                    n,
                    |s| f64::from_le_bytes(*s),
                )?)),
                35 => Ok(Variant::from(f::<8, _, PackedVector2Array>(
                    data,
                    i,
                    n,
                    |s| Vector2 {
                        x: f32::from_le_bytes(s[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(s[4..].try_into().unwrap()),
                    },
                )?)),
                36 => Ok(Variant::from(f::<12, _, PackedVector3Array>(
                    data,
                    i,
                    n,
                    |s| Vector3 {
                        x: f32::from_le_bytes(s[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(s[4..8].try_into().unwrap()),
                        z: f32::from_le_bytes(s[8..].try_into().unwrap()),
                    },
                )?)),
                37 => Ok(Variant::from(f::<16, _, PackedColorArray>(
                    data,
                    i,
                    n,
                    |s| Color {
                        r: f32::from_le_bytes(s[..4].try_into().unwrap()),
                        g: f32::from_le_bytes(s[4..8].try_into().unwrap()),
                        b: f32::from_le_bytes(s[8..12].try_into().unwrap()),
                        a: f32::from_le_bytes(s[12..].try_into().unwrap()),
                    },
                )?)),
                ..=37 => bail_with_site!("Unsupported type ID {}", t),
                _ => bail_with_site!("Unknown type {}", t),
            }
        }))
    }

    #[func]
    fn read_struct(&self, format: GString, p: i64) -> Variant {
        option_to_variant(self.get_memory(|store, mem| {
            read_struct_(mem.data(store), p as _, unsafe { format.chars_unchecked() })
        }))
    }

    #[func]
    fn write_struct(&self, format: GString, p: i64, arr: Array<Variant>) -> i64 {
        self.get_memory(|store, mem| {
            write_struct_(
                mem.data_mut(store),
                p as _,
                unsafe { format.chars_unchecked() },
                arr,
            )
        })
        .unwrap_or_default() as _
    }
}

fn process_number<I: Iterator<Item = (usize, char)>>(c: char, it: &mut Peekable<I>) -> u64 {
    let mut n: u64 = match c {
        '1' => 1,
        '2' => 2,
        '3' => 3,
        '4' => 4,
        '5' => 5,
        '6' => 6,
        '7' => 7,
        '8' => 8,
        '9' => 9,
        _ => unreachable!(),
    };
    while let Some((_, c @ '0'..='9')) = it.peek() {
        let i = match c {
            '0' => 0,
            '1' => 1,
            '2' => 2,
            '3' => 3,
            '4' => 4,
            '5' => 5,
            '6' => 6,
            '7' => 7,
            '8' => 8,
            '9' => 9,
            _ => unreachable!(),
        };
        n = n.saturating_mul(10).saturating_add(i);
        it.next();
    }

    n
}

fn read_struct_(data: &[u8], mut p: usize, format: &[char]) -> Result<Array<Variant>, Error> {
    fn f<const N: usize, T: ToGodot>(
        data: &[u8],
        p: &mut usize,
        a: &mut Array<Variant>,
        r: &mut Option<u64>,
        f: impl Fn(&[u8; N]) -> T,
    ) -> Result<(), Error> {
        for _ in 0..r.take().unwrap_or(1) {
            let s = *p;
            let e = s + N;
            let Some(data) = data.get(s..e) else {
                bail_with_site!("Index out of range ({s}..{e})")
            };
            a.push(f(data.try_into().unwrap()).to_variant());
            *p += N;
        }

        Ok(())
    }

    let mut ret = <Array<Variant>>::new();
    let mut it = format.iter().copied().enumerate().peekable();
    let mut n: Option<u64> = None;
    while let Some((i, c)) = it.next() {
        match c {
            // Parse initial number
            '1'..='9' if n.is_none() => {
                n = Some(process_number(c, &mut it));
                if it.peek().is_some() {
                    continue;
                }
                bail_with_site!(
                    "Quantity without type (at index {} {:?})",
                    i,
                    format[i..].iter().collect::<String>()
                );
            }
            'x' => {
                p += n.take().unwrap_or(1) as usize;
                continue;
            }
            'b' => f::<1, _>(data, &mut p, &mut ret, &mut n, |v| v[0] as i8 as i64),
            'B' => f::<1, _>(data, &mut p, &mut ret, &mut n, |v| v[0] as i64),
            'h' => f::<2, _>(data, &mut p, &mut ret, &mut n, |v| {
                i16::from_le_bytes(*v) as i64
            }),
            'H' => f::<2, _>(data, &mut p, &mut ret, &mut n, |v| {
                u16::from_le_bytes(*v) as i64
            }),
            'i' => f::<4, _>(data, &mut p, &mut ret, &mut n, |v| {
                i32::from_le_bytes(*v) as i64
            }),
            'I' => f::<4, _>(data, &mut p, &mut ret, &mut n, |v| {
                u32::from_le_bytes(*v) as i64
            }),
            'l' => f::<8, _>(data, &mut p, &mut ret, &mut n, |v| i64::from_le_bytes(*v)),
            'L' => f::<8, _>(data, &mut p, &mut ret, &mut n, |v| u64::from_le_bytes(*v)),
            'f' => f::<4, _>(data, &mut p, &mut ret, &mut n, |v| f32::from_le_bytes(*v)),
            'd' => f::<8, _>(data, &mut p, &mut ret, &mut n, |v| f64::from_le_bytes(*v)),
            'v' => {
                match it.next() {
                    None => bail_with_site!("Vector without size (at index {i})"),
                    Some((_, '2')) => match it.next() {
                        Some((_, 'f')) => {
                            f::<8, _>(data, &mut p, &mut ret, &mut n, |v| Vector2 {
                                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                                y: f32::from_le_bytes(v[4..].try_into().unwrap()),
                            })?;
                            continue;
                        }
                        Some((_, 'd')) => {
                            f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Vector2 {
                                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                                y: f64::from_le_bytes(v[8..].try_into().unwrap()) as _,
                            })?;
                            continue;
                        }
                        Some((_, 'i')) => {
                            f::<8, _>(data, &mut p, &mut ret, &mut n, |v| Vector2i {
                                x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                                y: i32::from_le_bytes(v[4..].try_into().unwrap()),
                            })?;
                            continue;
                        }
                        Some((_, 'l')) => {
                            f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Vector2i {
                                x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                                y: i64::from_le_bytes(v[8..].try_into().unwrap()) as _,
                            })?;
                            continue;
                        }
                        _ => (),
                    },
                    Some((_, '3')) => match it.next() {
                        Some((_, 'f')) => {
                            f::<12, _>(data, &mut p, &mut ret, &mut n, |v| Vector3 {
                                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                                y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                                z: f32::from_le_bytes(v[8..].try_into().unwrap()),
                            })?;
                            continue;
                        }
                        Some((_, 'd')) => {
                            f::<24, _>(data, &mut p, &mut ret, &mut n, |v| Vector3 {
                                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                                y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                                z: f64::from_le_bytes(v[16..].try_into().unwrap()) as _,
                            })?;
                            continue;
                        }
                        Some((_, 'i')) => {
                            f::<12, _>(data, &mut p, &mut ret, &mut n, |v| Vector3i {
                                x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                                y: i32::from_le_bytes(v[4..8].try_into().unwrap()),
                                z: i32::from_le_bytes(v[8..].try_into().unwrap()),
                            })?;
                            continue;
                        }
                        Some((_, 'l')) => {
                            f::<24, _>(data, &mut p, &mut ret, &mut n, |v| Vector3i {
                                x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                                y: i64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                                z: i64::from_le_bytes(v[16..].try_into().unwrap()) as _,
                            })?;
                            continue;
                        }
                        _ => (),
                    },
                    Some((_, '4')) => match it.next() {
                        Some((_, 'f')) => {
                            f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Vector4 {
                                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                                y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                                z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                                w: f32::from_le_bytes(v[12..].try_into().unwrap()),
                            })?;
                            continue;
                        }
                        Some((_, 'd')) => {
                            f::<32, _>(data, &mut p, &mut ret, &mut n, |v| Vector4 {
                                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                                y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                                z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                                w: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                            })?;
                            continue;
                        }
                        Some((_, 'i')) => {
                            f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Vector4i {
                                x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                                y: i32::from_le_bytes(v[4..8].try_into().unwrap()),
                                z: i32::from_le_bytes(v[8..12].try_into().unwrap()),
                                w: i32::from_le_bytes(v[12..].try_into().unwrap()),
                            })?;
                            continue;
                        }
                        Some((_, 'l')) => {
                            f::<32, _>(data, &mut p, &mut ret, &mut n, |v| Vector4i {
                                x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                                y: i64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                                z: i64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                                w: i64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                            })?;
                            continue;
                        }
                        _ => (),
                    },
                    _ => (),
                }

                let s = match it.peek() {
                    Some(&(j, _)) => &format[i..j],
                    None => &format[i..],
                };
                bail_with_site!(
                    "Unknown type {:?} (at index {i})",
                    s.iter().collect::<String>()
                );
            }
            'p' => match it.next() {
                None => bail_with_site!("Plane without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Plane {
                    normal: Vector3 {
                        x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                        z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    },
                    d: f32::from_le_bytes(v[12..].try_into().unwrap()),
                }),
                Some((_, 'd')) => f::<32, _>(data, &mut p, &mut ret, &mut n, |v| Plane {
                    normal: Vector3 {
                        x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    },
                    d: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Plane subtype {c:?} (at index {i})")
                }
            },
            'q' => match it.next() {
                None => bail_with_site!("Quat without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Quaternion {
                    x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    w: f32::from_le_bytes(v[12..].try_into().unwrap()),
                }),
                Some((_, 'd')) => f::<32, _>(data, &mut p, &mut ret, &mut n, |v| Quaternion {
                    x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                    y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                    z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    w: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Quat subtype {c:?} (at index {i})")
                }
            },
            'C' => match it.next() {
                None => bail_with_site!("Color without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Color {
                    r: f32::from_le_bytes(v[..4].try_into().unwrap()),
                    g: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    b: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    a: f32::from_le_bytes(v[12..].try_into().unwrap()),
                }),
                Some((_, 'd')) => f::<32, _>(data, &mut p, &mut ret, &mut n, |v| Color {
                    r: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                    g: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                    b: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    a: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                }),
                Some((_, 'b')) => f::<4, _>(data, &mut p, &mut ret, &mut n, |&[r, g, b, a]| {
                    Color::from_rgba8(r, g, b, a)
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Color subtype {c:?} (at index {i})")
                }
            },
            'r' => match it.next() {
                None => bail_with_site!("Rect2 without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Rect2 {
                    position: Vector2 {
                        x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    },
                    size: Vector2 {
                        x: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                        y: f32::from_le_bytes(v[12..].try_into().unwrap()),
                    },
                }),
                Some((_, 'd')) => f::<32, _>(data, &mut p, &mut ret, &mut n, |v| Rect2 {
                    position: Vector2 {
                        x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                    },
                    size: Vector2 {
                        x: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                    },
                }),
                Some((_, 'i')) => f::<16, _>(data, &mut p, &mut ret, &mut n, |v| Rect2i {
                    position: Vector2i {
                        x: i32::from_le_bytes(v[..4].try_into().unwrap()),
                        y: i32::from_le_bytes(v[4..8].try_into().unwrap()),
                    },
                    size: Vector2i {
                        x: i32::from_le_bytes(v[8..12].try_into().unwrap()) as _,
                        y: i32::from_le_bytes(v[12..].try_into().unwrap()) as _,
                    },
                }),
                Some((_, 'l')) => f::<32, _>(data, &mut p, &mut ret, &mut n, |v| Rect2i {
                    position: Vector2i {
                        x: i64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                        y: i64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                    },
                    size: Vector2i {
                        x: i64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                        y: i64::from_le_bytes(v[24..].try_into().unwrap()) as _,
                    },
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Rect2 subtype {c:?} (at index {i})")
                }
            },
            'a' => match it.next() {
                None => bail_with_site!("Aabb without subtype (at index {i})"),
                Some((_, 'f')) => f::<24, _>(data, &mut p, &mut ret, &mut n, |v| Aabb {
                    position: Vector3 {
                        x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                        z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                    },
                    size: Vector3 {
                        x: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                        y: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                        z: f32::from_le_bytes(v[20..].try_into().unwrap()),
                    },
                }),
                Some((_, 'd')) => f::<48, _>(data, &mut p, &mut ret, &mut n, |v| Aabb {
                    position: Vector3 {
                        x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                    },
                    size: Vector3 {
                        x: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[40..].try_into().unwrap()) as _,
                    },
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Aabb subtype {c:?} (at index {i})")
                }
            },
            'm' => match it.next() {
                None => bail_with_site!("Basis without subtype (at index {i})"),
                Some((_, 'f')) => f::<36, _>(data, &mut p, &mut ret, &mut n, |v| Basis {
                    rows: [
                        Vector3 {
                            x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                            y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                            z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                        },
                        Vector3 {
                            x: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                            y: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                            z: f32::from_le_bytes(v[20..24].try_into().unwrap()),
                        },
                        Vector3 {
                            x: f32::from_le_bytes(v[24..28].try_into().unwrap()),
                            y: f32::from_le_bytes(v[28..32].try_into().unwrap()),
                            z: f32::from_le_bytes(v[32..].try_into().unwrap()),
                        },
                    ],
                }),
                Some((_, 'd')) => f::<72, _>(data, &mut p, &mut ret, &mut n, |v| Basis {
                    rows: [
                        Vector3 {
                            x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                        },
                        Vector3 {
                            x: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[40..48].try_into().unwrap()) as _,
                        },
                        Vector3 {
                            x: f64::from_le_bytes(v[48..56].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[56..64].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[64..].try_into().unwrap()) as _,
                        },
                    ],
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Basis subtype {c:?} (at index {i})")
                }
            },
            'M' => match it.next() {
                None => bail_with_site!("Projection without subtype (at index {i})"),
                Some((_, 'f')) => f::<64, _>(data, &mut p, &mut ret, &mut n, |v| Projection {
                    cols: [
                        Vector4 {
                            x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                            y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                            z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                            w: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                        },
                        Vector4 {
                            x: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                            y: f32::from_le_bytes(v[20..24].try_into().unwrap()),
                            z: f32::from_le_bytes(v[24..28].try_into().unwrap()),
                            w: f32::from_le_bytes(v[28..32].try_into().unwrap()),
                        },
                        Vector4 {
                            x: f32::from_le_bytes(v[32..36].try_into().unwrap()),
                            y: f32::from_le_bytes(v[36..40].try_into().unwrap()),
                            z: f32::from_le_bytes(v[40..44].try_into().unwrap()),
                            w: f32::from_le_bytes(v[44..48].try_into().unwrap()),
                        },
                        Vector4 {
                            x: f32::from_le_bytes(v[48..52].try_into().unwrap()),
                            y: f32::from_le_bytes(v[52..56].try_into().unwrap()),
                            z: f32::from_le_bytes(v[56..60].try_into().unwrap()),
                            w: f32::from_le_bytes(v[60..].try_into().unwrap()),
                        },
                    ],
                }),
                Some((_, 'd')) => f::<128, _>(data, &mut p, &mut ret, &mut n, |v| Projection {
                    cols: [
                        Vector4 {
                            x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                            w: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                        },
                        Vector4 {
                            x: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[40..48].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[48..56].try_into().unwrap()) as _,
                            w: f64::from_le_bytes(v[56..64].try_into().unwrap()) as _,
                        },
                        Vector4 {
                            x: f64::from_le_bytes(v[64..72].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[72..80].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[80..88].try_into().unwrap()) as _,
                            w: f64::from_le_bytes(v[88..96].try_into().unwrap()) as _,
                        },
                        Vector4 {
                            x: f64::from_le_bytes(v[96..104].try_into().unwrap()) as _,
                            y: f64::from_le_bytes(v[104..112].try_into().unwrap()) as _,
                            z: f64::from_le_bytes(v[112..120].try_into().unwrap()) as _,
                            w: f64::from_le_bytes(v[120..].try_into().unwrap()) as _,
                        },
                    ],
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Projection subtype {c:?} (at index {i})")
                }
            },
            't' => match it.next() {
                None => bail_with_site!("Transform2D without subtype (at index {i})"),
                Some((_, 'f')) => f::<24, _>(data, &mut p, &mut ret, &mut n, |v| Transform2D {
                    a: Vector2 {
                        x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    },
                    b: Vector2 {
                        x: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                        y: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                    },
                    origin: Vector2 {
                        x: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                        y: f32::from_le_bytes(v[20..].try_into().unwrap()),
                    },
                }),
                Some((_, 'd')) => f::<48, _>(data, &mut p, &mut ret, &mut n, |v| Transform2D {
                    a: Vector2 {
                        x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                    },
                    b: Vector2 {
                        x: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                    },
                    origin: Vector2 {
                        x: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[40..].try_into().unwrap()) as _,
                    },
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Transform2D subtype {c:?} (at index {i})")
                }
            },
            'T' => match it.next() {
                None => bail_with_site!("Transform3D without subtype (at index {i})"),
                Some((_, 'f')) => f::<48, _>(data, &mut p, &mut ret, &mut n, |v| Transform3D {
                    basis: Basis {
                        rows: [
                            Vector3 {
                                x: f32::from_le_bytes(v[..4].try_into().unwrap()),
                                y: f32::from_le_bytes(v[4..8].try_into().unwrap()),
                                z: f32::from_le_bytes(v[8..12].try_into().unwrap()),
                            },
                            Vector3 {
                                x: f32::from_le_bytes(v[12..16].try_into().unwrap()),
                                y: f32::from_le_bytes(v[16..20].try_into().unwrap()),
                                z: f32::from_le_bytes(v[20..24].try_into().unwrap()),
                            },
                            Vector3 {
                                x: f32::from_le_bytes(v[24..28].try_into().unwrap()),
                                y: f32::from_le_bytes(v[28..32].try_into().unwrap()),
                                z: f32::from_le_bytes(v[32..36].try_into().unwrap()),
                            },
                        ],
                    },
                    origin: Vector3 {
                        x: f32::from_le_bytes(v[36..40].try_into().unwrap()),
                        y: f32::from_le_bytes(v[40..44].try_into().unwrap()),
                        z: f32::from_le_bytes(v[44..].try_into().unwrap()),
                    },
                }),
                Some((_, 'd')) => f::<96, _>(data, &mut p, &mut ret, &mut n, |v| Transform3D {
                    basis: Basis {
                        rows: [
                            Vector3 {
                                x: f64::from_le_bytes(v[..8].try_into().unwrap()) as _,
                                y: f64::from_le_bytes(v[8..16].try_into().unwrap()) as _,
                                z: f64::from_le_bytes(v[16..24].try_into().unwrap()) as _,
                            },
                            Vector3 {
                                x: f64::from_le_bytes(v[24..32].try_into().unwrap()) as _,
                                y: f64::from_le_bytes(v[32..40].try_into().unwrap()) as _,
                                z: f64::from_le_bytes(v[40..48].try_into().unwrap()) as _,
                            },
                            Vector3 {
                                x: f64::from_le_bytes(v[48..56].try_into().unwrap()) as _,
                                y: f64::from_le_bytes(v[56..64].try_into().unwrap()) as _,
                                z: f64::from_le_bytes(v[64..72].try_into().unwrap()) as _,
                            },
                        ],
                    },
                    origin: Vector3 {
                        x: f64::from_le_bytes(v[72..80].try_into().unwrap()) as _,
                        y: f64::from_le_bytes(v[80..88].try_into().unwrap()) as _,
                        z: f64::from_le_bytes(v[88..].try_into().unwrap()) as _,
                    },
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Transform3D subtype {c:?} (at index {i})")
                }
            },
            _ => bail_with_site!("Unknown type {c:?} (at index {i})"),
        }?;
    }

    Ok(ret)
}

fn write_struct_(
    data: &mut [u8],
    mut p: usize,
    format: &[char],
    arr: Array<Variant>,
) -> Result<usize, Error> {
    fn f<const N: usize, T: FromGodot>(
        data: &mut [u8],
        p: &mut usize,
        a: &mut impl Iterator<Item = Variant>,
        r: &mut Option<u64>,
        f: impl Fn(&mut [u8; N], T),
    ) -> Result<(), Error> {
        for _ in 0..r.take().unwrap_or(1) {
            let Some(v) = a
                .next()
                .map(|v| site_context!(T::try_from_variant(&v)))
                .transpose()?
            else {
                bail_with_site!("Input array too small")
            };
            let s = *p;
            let e = s + N;
            let Some(data) = data.get_mut(s..e) else {
                bail_with_site!("Index out of range ({s}..{e})")
            };
            f(data.try_into().unwrap(), v);
            *p += N;
        }

        Ok(())
    }

    let s = p;
    let mut arr = arr.iter_shared();
    let mut it = format.iter().copied().enumerate().peekable();
    let mut n: Option<u64> = None;
    while let Some((i, c)) = it.next() {
        match c {
            // Parse initial number
            '1'..='9' if n.is_none() => {
                n = Some(process_number(c, &mut it));
                if it.peek().is_some() {
                    continue;
                }
                bail_with_site!(
                    "Quantity without type (at index {} {:?})",
                    i,
                    format[i..].iter().collect::<String>()
                );
            }
            'x' => {
                p += n.take().unwrap_or(1) as usize;
                continue;
            }
            'b' => f::<1, i64>(data, &mut p, &mut arr, &mut n, |s, d| s[0] = d as i8 as u8),
            'B' => f::<1, i64>(data, &mut p, &mut arr, &mut n, |s, d| s[0] = d as u8),
            'h' => f::<2, i64>(data, &mut p, &mut arr, &mut n, |s, d| {
                *s = (d as i16).to_le_bytes()
            }),
            'H' => f::<2, i64>(data, &mut p, &mut arr, &mut n, |s, d| {
                *s = (d as u16).to_le_bytes()
            }),
            'i' => f::<4, i64>(data, &mut p, &mut arr, &mut n, |s, d| {
                *s = (d as i32).to_le_bytes()
            }),
            'I' => f::<4, i64>(data, &mut p, &mut arr, &mut n, |s, d| {
                *s = (d as u32).to_le_bytes()
            }),
            'l' | 'L' => f::<8, i64>(data, &mut p, &mut arr, &mut n, |s, d| *s = d.to_le_bytes()),
            'f' => f::<4, f32>(data, &mut p, &mut arr, &mut n, |s, d| *s = d.to_le_bytes()),
            'd' => f::<8, f64>(data, &mut p, &mut arr, &mut n, |s, d| *s = d.to_le_bytes()),
            'v' => {
                match it.next() {
                    None => bail_with_site!("Vector without size (at index {i})"),
                    Some((_, '2')) => match it.next() {
                        Some((_, 'f')) => {
                            f::<8, Vector2>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[4..]).unwrap() = d.y.to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'd')) => {
                            f::<16, Vector2>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                                    (d.x as f64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[8..]).unwrap() =
                                    (d.y as f64).to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'i')) => {
                            f::<8, Vector2i>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[4..]).unwrap() = d.y.to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'l')) => {
                            f::<16, Vector2i>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                                    (d.x as i64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[8..]).unwrap() =
                                    (d.y as i64).to_le_bytes();
                            })?;
                            continue;
                        }
                        _ => (),
                    },
                    Some((_, '3')) => match it.next() {
                        Some((_, 'f')) => {
                            f::<12, Vector3>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() =
                                    d.y.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[8..]).unwrap() = d.z.to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'd')) => {
                            f::<24, Vector3>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                                    (d.x as f64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                                    (d.y as f64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[16..]).unwrap() =
                                    (d.z as f64).to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'i')) => {
                            f::<12, Vector3i>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() =
                                    d.y.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[8..]).unwrap() = d.z.to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'l')) => {
                            f::<24, Vector3i>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                                    (d.x as i64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                                    (d.y as i64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[16..]).unwrap() =
                                    (d.z as i64).to_le_bytes();
                            })?;
                            continue;
                        }
                        _ => (),
                    },
                    Some((_, '4')) => match it.next() {
                        Some((_, 'f')) => {
                            f::<16, Vector4>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() =
                                    d.y.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() =
                                    d.z.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() =
                                    d.w.to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'd')) => {
                            f::<32, Vector4>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                                    (d.x as f64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                                    (d.y as f64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                                    (d.z as f64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() =
                                    (d.w as f64).to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'i')) => {
                            f::<16, Vector4i>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() =
                                    d.y.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() =
                                    d.z.to_le_bytes();
                                *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() =
                                    d.w.to_le_bytes();
                            })?;
                            continue;
                        }
                        Some((_, 'l')) => {
                            f::<32, Vector4i>(data, &mut p, &mut arr, &mut n, |s, d| {
                                *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                                    (d.x as i64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                                    (d.y as i64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                                    (d.z as i64).to_le_bytes();
                                *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() =
                                    (d.w as i64).to_le_bytes();
                            })?;
                            continue;
                        }
                        _ => (),
                    },
                    _ => (),
                }

                let s = match it.peek() {
                    Some(&(j, _)) => &format[i..j],
                    None => &format[i..],
                };
                bail_with_site!(
                    "Unknown type {:?} (at index {i})",
                    s.iter().collect::<String>()
                );
            }
            'p' => match it.next() {
                None => bail_with_site!("Plane without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, Plane>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.normal.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.normal.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.normal.z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.d.to_le_bytes();
                }),
                Some((_, 'd')) => f::<32, Plane>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.normal.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.normal.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.normal.z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.d as f64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Plane subtype {c:?} (at index {i})")
                }
            },
            'q' => match it.next() {
                None => bail_with_site!("Quat without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, Quaternion>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.w.to_le_bytes();
                }),
                Some((_, 'd')) => f::<32, Quaternion>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() = (d.z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.w as f64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Quat subtype {c:?} (at index {i})")
                }
            },
            'C' => match it.next() {
                None => bail_with_site!("Color without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, Color>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.r.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.g.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.b.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.a.to_le_bytes();
                }),
                Some((_, 'd')) => f::<32, Color>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.r as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() = (d.g as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() = (d.b as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() = (d.a as f64).to_le_bytes();
                }),
                Some((_, 'b')) => f::<4, Color>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *s = [
                        (d.r * 255.) as u8,
                        (d.g * 255.) as u8,
                        (d.b * 255.) as u8,
                        (d.a * 255.) as u8,
                    ];
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Color subtype {c:?} (at index {i})")
                }
            },
            'r' => match it.next() {
                None => bail_with_site!("Rect2 without subtype (at index {i})"),
                Some((_, 'f')) => f::<16, Rect2>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.position.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.position.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.size.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.size.y.to_le_bytes();
                }),
                Some((_, 'd')) => f::<32, Rect2>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.position.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.position.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.size.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() =
                        (d.size.y as f64).to_le_bytes();
                }),
                Some((_, 'i')) => f::<16, Rect2i>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.position.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.position.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.size.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..]).unwrap() = d.size.y.to_le_bytes();
                }),
                Some((_, 'l')) => f::<32, Rect2i>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.position.x as i64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.position.y as i64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.size.x as i64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..]).unwrap() =
                        (d.size.y as i64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Rect2 subtype {c:?} (at index {i})")
                }
            },
            'a' => match it.next() {
                None => bail_with_site!("Aabb without subtype (at index {i})"),
                Some((_, 'f')) => f::<24, Aabb>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.position.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.position.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.position.z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.size.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.size.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[20..]).unwrap() = d.size.z.to_le_bytes();
                }),
                Some((_, 'd')) => f::<48, Aabb>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.position.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.position.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.position.z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.size.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.size.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..]).unwrap() =
                        (d.size.z as f64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Aabb subtype {c:?} (at index {i})")
                }
            },
            'm' => match it.next() {
                None => bail_with_site!("Basis without subtype (at index {i})"),
                Some((_, 'f')) => f::<36, Basis>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.rows[0].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.rows[0].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.rows[0].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.rows[1].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.rows[1].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[20..24]).unwrap() = d.rows[1].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[24..28]).unwrap() = d.rows[2].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[28..32]).unwrap() = d.rows[2].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[32..]).unwrap() = d.rows[2].z.to_le_bytes();
                }),
                Some((_, 'd')) => f::<72, Basis>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.rows[0].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.rows[0].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.rows[0].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.rows[1].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.rows[1].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..48]).unwrap() =
                        (d.rows[1].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[48..56]).unwrap() =
                        (d.rows[2].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[56..64]).unwrap() =
                        (d.rows[2].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[64..]).unwrap() =
                        (d.rows[2].z as f64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Basis subtype {c:?} (at index {i})")
                }
            },
            'M' => match it.next() {
                None => bail_with_site!("Projection without subtype (at index {i})"),
                Some((_, 'f')) => f::<64, Projection>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.cols[0].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.cols[0].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.cols[0].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.cols[0].w.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.cols[1].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[20..24]).unwrap() = d.cols[1].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[24..28]).unwrap() = d.cols[1].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[28..32]).unwrap() = d.cols[1].w.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[32..36]).unwrap() = d.cols[2].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[36..40]).unwrap() = d.cols[2].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[40..44]).unwrap() = d.cols[2].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[44..48]).unwrap() = d.cols[2].w.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[48..52]).unwrap() = d.cols[3].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[52..56]).unwrap() = d.cols[3].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[56..60]).unwrap() = d.cols[3].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[60..]).unwrap() = d.cols[3].w.to_le_bytes();
                }),
                Some((_, 'd')) => f::<128, Projection>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.cols[0].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.cols[0].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.cols[0].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.cols[0].w as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.cols[1].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..48]).unwrap() =
                        (d.cols[1].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[48..56]).unwrap() =
                        (d.cols[1].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[56..64]).unwrap() =
                        (d.cols[1].w as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[64..72]).unwrap() =
                        (d.cols[2].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[72..80]).unwrap() =
                        (d.cols[2].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[80..88]).unwrap() =
                        (d.cols[2].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[88..96]).unwrap() =
                        (d.cols[2].w as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[96..104]).unwrap() =
                        (d.cols[3].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[104..112]).unwrap() =
                        (d.cols[3].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[112..120]).unwrap() =
                        (d.cols[3].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[120..]).unwrap() =
                        (d.cols[3].w as f64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Projection subtype {c:?} (at index {i})")
                }
            },
            't' => match it.next() {
                None => bail_with_site!("Transform2D without subtype (at index {i})"),
                Some((_, 'f')) => f::<24, Transform2D>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() = d.a.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() = d.a.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() = d.b.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() = d.b.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() = d.origin.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[20..]).unwrap() = d.origin.y.to_le_bytes();
                }),
                Some((_, 'd')) => f::<48, Transform2D>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() = (d.a.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.a.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.b.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.b.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.origin.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..]).unwrap() =
                        (d.origin.y as f64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Transform2D subtype {c:?} (at index {i})")
                }
            },
            'T' => match it.next() {
                None => bail_with_site!("Transform3D without subtype (at index {i})"),
                Some((_, 'f')) => f::<48, Transform3D>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut s[..4]).unwrap() =
                        d.basis.rows[0].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[4..8]).unwrap() =
                        d.basis.rows[0].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[8..12]).unwrap() =
                        d.basis.rows[0].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[12..16]).unwrap() =
                        d.basis.rows[1].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[16..20]).unwrap() =
                        d.basis.rows[1].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[20..24]).unwrap() =
                        d.basis.rows[1].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[24..28]).unwrap() =
                        d.basis.rows[2].x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[28..32]).unwrap() =
                        d.basis.rows[2].y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[32..36]).unwrap() =
                        d.basis.rows[2].z.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[36..40]).unwrap() = d.origin.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[40..44]).unwrap() = d.origin.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut s[44..]).unwrap() = d.origin.z.to_le_bytes();
                }),
                Some((_, 'd')) => f::<96, Transform3D>(data, &mut p, &mut arr, &mut n, |s, d| {
                    *<&mut [u8; 8]>::try_from(&mut s[..8]).unwrap() =
                        (d.basis.rows[0].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[8..16]).unwrap() =
                        (d.basis.rows[0].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[16..24]).unwrap() =
                        (d.basis.rows[0].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[24..32]).unwrap() =
                        (d.basis.rows[1].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[32..40]).unwrap() =
                        (d.basis.rows[1].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[40..48]).unwrap() =
                        (d.basis.rows[1].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[48..56]).unwrap() =
                        (d.basis.rows[2].x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[56..64]).unwrap() =
                        (d.basis.rows[2].y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[64..72]).unwrap() =
                        (d.basis.rows[2].z as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[72..80]).unwrap() =
                        (d.origin.x as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[80..88]).unwrap() =
                        (d.origin.y as f64).to_le_bytes();
                    *<&mut [u8; 8]>::try_from(&mut s[88..]).unwrap() =
                        (d.origin.z as f64).to_le_bytes();
                }),
                Some((_, c)) => {
                    bail_with_site!("Unknown Transform3D subtype {c:?} (at index {i})")
                }
            },
            _ => bail_with_site!("Unknown type {c:?} (at index {i})"),
        }?;
    }

    Ok(p - s)
}
