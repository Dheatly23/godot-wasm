use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::{mem, ptr};

use anyhow::{bail, Error};
use gdnative::core_types::PoolElement;
use gdnative::export::user_data::Map;
use gdnative::log::{error, godot_site, Site};
use gdnative::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, RawMutex};
use rayon::prelude::*;
use scopeguard::guard;
#[cfg(feature = "wasi-preview2")]
use wasmtime::component::Instance as InstanceComp;
#[cfg(feature = "wasi")]
use wasmtime::component::ResourceTable;
#[cfg(feature = "wasi")]
use wasmtime::Linker;
use wasmtime::{
    AsContextMut, Extern, Instance as InstanceWasm, Memory, ResourceLimiter, RootScope, Store,
    StoreContextMut, ValRaw,
};
#[cfg(feature = "wasi")]
use wasmtime_wasi::preview1::{add_to_linker_sync, WasiP1Ctx};
#[cfg(feature = "wasi")]
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

use crate::rw_struct::{read_struct, write_struct};
#[cfg(feature = "wasi")]
use crate::wasi_ctx::stdio::{
    BlockWritePipe, ByteBufferReadPipe, InnerStdin, LineWritePipe, OuterStdin, StreamWrapper,
    UnbufferedWritePipe,
};
#[cfg(feature = "wasi")]
use crate::wasi_ctx::WasiContext;
use crate::wasm_config::{Config, ExternBindingType};
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
use crate::wasm_util::{config_store_common, from_raw, to_raw, HostModuleCache, MEMORY_EXPORT};
use crate::{bail_with_site, site_context};

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::ArcData<WasmInstance>)]
pub struct WasmInstance {
    data: OnceCell<InstanceData<StoreData>>,
    memory: Option<Memory>,
}

pub struct InstanceData<T> {
    pub store: Mutex<Store<T>>,
    pub instance: InstanceType,
    pub module: Instance<WasmModule, Shared>,

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
    #[cfg(feature = "wasi")]
    Preview1(WasiP1Ctx),
    #[cfg(feature = "wasi-preview2")]
    Preview2(WasiCtx, ResourceTable),
}

#[cfg(feature = "wasi")]
impl WasiView for StoreData {
    fn table(&mut self) -> &mut ResourceTable {
        match &mut self.wasi_ctx {
            MaybeWasi::Preview1(v) => v.table(),
            #[cfg(feature = "wasi-preview2")]
            MaybeWasi::Preview2(_, tbl) => tbl,
            _ => panic!("Requested WASI Preview 2 interface while none set, this is a bug"),
        }
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        match &mut self.wasi_ctx {
            MaybeWasi::Preview1(v) => v.ctx(),
            #[cfg(feature = "wasi-preview2")]
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
        max: Option<usize>,
    ) -> Result<bool, Error> {
        if max.map_or(false, |max| desired > max) {
            return Ok(false);
        } else if self.max_memory == u64::MAX {
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

    fn table_growing(
        &mut self,
        current: u32,
        desired: u32,
        max: Option<u32>,
    ) -> Result<bool, Error> {
        if max.map_or(false, |max| desired > max) {
            return Ok(false);
        } else if self.max_table_entries == u64::MAX {
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
    T: Send + AsRef<StoreData> + AsMut<StoreData>,
{
    pub fn instantiate(
        owner: &Reference,
        mut store: Store<T>,
        module: Instance<WasmModule, Shared>,
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

            let inst_id = owner.get_instance_id();
            if config.wasi_stdin == PipeBindingType::Instance {
                if let Some(data) = config.wasi_stdin_data.clone() {
                    builder.stdin(StreamWrapper::from(ByteBufferReadPipe::new(data)));
                } else {
                    let (outer, inner) = OuterStdin::new(move || unsafe {
                        let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                            return;
                        };
                        owner.emit_signal("stdin_request", &[]);
                    });
                    builder.stdin(outer);
                    wasi_stdin = Some(inner as _);
                }
            }
            if config.wasi_stdout == PipeBindingType::Instance {
                match config.wasi_stdout_buffer {
                    PipeBufferType::Unbuffered => {
                        builder.stdout(UnbufferedWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stdout_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        }))
                    }
                    PipeBufferType::LineBuffer => {
                        builder.stdout(StreamWrapper::from(LineWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stdout_emit",
                                &[String::from_utf8_lossy(buf).to_variant()],
                            );
                        })))
                    }
                    PipeBufferType::BlockBuffer => builder.stdout(StreamWrapper::from(
                        BlockWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stdout_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        }),
                    )),
                };
            }
            if config.wasi_stderr == PipeBindingType::Instance {
                match config.wasi_stderr_buffer {
                    PipeBufferType::Unbuffered => {
                        builder.stderr(UnbufferedWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stderr_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        }))
                    }
                    PipeBufferType::LineBuffer => {
                        builder.stderr(StreamWrapper::from(LineWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stderr_emit",
                                &[String::from_utf8_lossy(buf).to_variant()],
                            );
                        })))
                    }
                    PipeBufferType::BlockBuffer => builder.stderr(StreamWrapper::from(
                        BlockWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stderr_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        }),
                    )),
                };
            }

            let _ctx = match &config.wasi_context {
                Some(ctx) => WasiContext::build_ctx(ctx.clone(), &mut builder, &*config),
                None => WasiContext::init_ctx_no_context(&mut builder, &*config),
            }?;
            *wasi_ctx = MaybeWasi::Preview1(builder.build_p1());
            let mut r = <Linker<T>>::new(&ENGINE);
            add_to_linker_sync(&mut r, |data| match &mut data.as_mut().wasi_ctx {
                MaybeWasi::Preview1(v) => v,
                _ => panic!("WASI Preview 1 context required, but none supplied"),
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
        let instance = module
            .script()
            .map(move |m| {
                let mut insts = HashMap::new();
                Self::instantiate_wasm(
                    sp,
                    m.get_data()?,
                    &mut insts,
                    &mut host.map(HostModuleCache::new),
                    #[cfg(feature = "object-registry-compat")]
                    &mut ObjregistryFuncs::default(),
                    #[cfg(feature = "object-registry-extern")]
                    &mut ExternrefFuncs::default(),
                    #[cfg(feature = "wasi")]
                    wasi_linker.as_ref(),
                )
            })
            .unwrap()?;

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
        insts: &mut HashMap<Ref<Reference, Shared>, InstanceWasm>,
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
                    match insts.get(v.base()) {
                        Some(v) => break v,
                        None => {
                            let t = v
                                .script()
                                .map(|m| {
                                    Self::instantiate_wasm(
                                        &mut *store,
                                        m.get_data()?,
                                        &mut *insts,
                                        &mut *host,
                                        #[cfg(feature = "object-registry-compat")]
                                        &mut *objregistry_funcs,
                                        #[cfg(feature = "object-registry-extern")]
                                        &mut *externref_funcs,
                                        #[cfg(feature = "wasi")]
                                        wasi_linker,
                                    )
                                })
                                .unwrap()?;
                            insts.insert(v.base().clone(), t);
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
    fn new(_owner: &Reference) -> Self {
        Self {
            data: OnceCell::new(),
            memory: None,
        }
    }

    pub fn get_data(&self) -> Result<&InstanceData<StoreData>, Error> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized instance")
        }
    }

    pub fn unwrap_data<F, R>(&self, base: TRef<Reference>, f: F) -> Option<R>
    where
        F: FnOnce(&InstanceData<StoreData>) -> Result<R, Error>,
    {
        match self.get_data().and_then(f) {
            Ok(v) => Some(v),
            Err(e) => {
                let s = format!("{:?}", e);
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    &s,
                );
                base.emit_signal("error_happened", &[s.owned_to_variant()]);
                None
            }
        }
    }

    pub fn initialize_(
        &self,
        owner: &Reference,
        module: Instance<WasmModule, Shared>,
        host: Option<Dictionary>,
        config: Option<Variant>,
    ) -> bool {
        match self.data.get_or_try_init(move || -> Result<_, Error> {
            let mut ret = InstanceData::instantiate(
                owner,
                Store::new(
                    &ENGINE,
                    StoreData {
                        mutex_raw: ptr::null(),
                        config: match config {
                            Some(v) => match Config::from_variant(&v) {
                                Ok(v) => v,
                                Err(e) => {
                                    godot_error!("{}", e);
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
            )?;

            // SAFETY: Nobody else can access memory
            #[allow(mutable_transmutes)]
            unsafe {
                *mem::transmute::<_, &mut Option<Memory>>(&self.memory) = match &ret.instance {
                    InstanceType::Core(inst) => inst.get_memory(ret.store.get_mut(), MEMORY_EXPORT),
                    #[allow(unreachable_patterns)]
                    _ => None,
                };
            }
            Ok(ret)
        }) {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    fn get_memory<F, R>(&self, base: TRef<Reference>, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(StoreContextMut<'a, StoreData>, Memory) -> Result<R, Error>,
    {
        self.unwrap_data(base, |m| {
            m.acquire_store(|_, store| match self.memory {
                Some(mem) => f(store, mem),
                None => bail_with_site!("No memory exported"),
            })
        })
    }

    fn read_memory<F, R>(&self, base: TRef<Reference>, i: usize, n: usize, f: F) -> Option<R>
    where
        F: FnOnce(&[u8]) -> Result<R, Error>,
    {
        self.get_memory(base, |store, mem| {
            let data = mem.data(&store);
            match data.get(i..i + n) {
                Some(s) => f(s),
                None => bail_with_site!("Index out of bound {}-{}", i, i + n),
            }
        })
    }

    fn write_memory<F, R>(&self, base: TRef<Reference>, i: usize, n: usize, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(&'a mut [u8]) -> Result<R, Error>,
    {
        self.get_memory(base, |mut store, mem| {
            let data = mem.data_mut(&mut store);
            match data.get_mut(i..i + n) {
                Some(s) => f(s),
                None => bail_with_site!("Index out of bound {}-{}", i, i + n),
            }
        })
    }
}

#[methods]
impl WasmInstance {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .property::<Option<Instance<WasmModule, Shared>>>("module")
            .with_getter(|v, b| v.unwrap_data(b, |m| Ok(m.module.clone())))
            .done();

        builder
            .signal("error_happened")
            .with_param("message", VariantType::GodotString)
            .done();

        builder
            .signal("stdout_emit")
            .with_param("message", VariantType::GodotString)
            .done();

        builder
            .signal("stderr_emit")
            .with_param("message", VariantType::GodotString)
            .done();

        builder.signal("stdin_request").done();
    }

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[method]
    fn initialize(
        &self,
        #[base] owner: TRef<Reference>,
        module: Instance<WasmModule, Shared>,
        #[opt] host: Option<Dictionary>,
        #[opt] config: Option<Variant>,
    ) -> Option<Ref<Reference>> {
        if self.initialize_(owner.as_ref(), module, host, config) {
            Some(owner.claim())
        } else {
            None
        }
    }

    #[method]
    fn call_wasm(
        &self,
        #[base] base: TRef<Reference>,
        name: String,
        args: VariantArray,
    ) -> Option<VariantArray> {
        self.unwrap_data(base, move |m| {
            m.acquire_store(move |m, mut store| {
                let f = match site_context!(m.instance.get_core())?.get_export(&mut store, &name) {
                    Some(f) => match f {
                        Extern::Func(f) => f,
                        _ => bail_with_site!("Export {} is not a function", &name),
                    },
                    None => bail_with_site!("Export {} does not exists", &name),
                };

                store.gc();
                let mut scope = RootScope::new(store);
                let mut store = scope.as_context_mut();

                let ty = f.ty(&store);
                let pi = ty.params();
                let ri = ty.results();
                let mut arr = Vec::with_capacity(pi.len().max(ri.len()));

                let pl = pi.len();
                for (t, v) in pi.zip(&args) {
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

                let ret = VariantArray::new();
                for (t, v) in ri.zip(arr) {
                    ret.push(unsafe { from_raw(&mut store, t, v)? });
                }

                Ok(ret.into_shared())
            })
        })
    }

    /// Emit trap when returning from host. Only used for host binding.
    /// Returns previous error message, if any.
    #[method]
    fn signal_error(&self, #[base] base: TRef<Reference>, msg: String) -> Option<String> {
        self.unwrap_data(base, |m| {
            m.acquire_store(|_, mut store| Ok(store.data_mut().error_signal.replace(msg)))
        })
        .flatten()
    }

    /// Cancel effect of signal_error.
    /// Returns previous error message, if any.
    #[method]
    fn signal_error_cancel(&self, #[base] base: TRef<Reference>) -> Option<String> {
        self.unwrap_data(base, |m| {
            m.acquire_store(|_, mut store| Ok(store.data_mut().error_signal.take()))
        })
        .flatten()
    }

    #[method]
    fn reset_epoch(&self, #[base] _base: TRef<Reference>) {
        #[cfg(feature = "epoch-timeout")]
        self.unwrap_data(_base, |m| {
            m.acquire_store(|_, mut store| {
                store.set_epoch_deadline(store.data().config.epoch_timeout);
                Ok(())
            })
        });

        #[cfg(not(feature = "epoch-timeout"))]
        godot_error!("Feature epoch-timeout not enabled!");
    }

    #[method]
    fn register_object(&self, #[base] _base: TRef<Reference>, _obj: Variant) -> Option<usize> {
        #[cfg(feature = "object-registry-compat")]
        return self.unwrap_data(_base, |m| {
            if _obj.is_nil() {
                bail_with_site!("Value is null!");
            }
            m.acquire_store(|_, mut store| Ok(store.data_mut().get_registry_mut()?.register(_obj)))
        });

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            None
        }
    }

    #[method]
    fn registry_get(&self, #[base] _base: TRef<Reference>, _ix: usize) -> Option<Variant> {
        #[cfg(feature = "object-registry-compat")]
        return self
            .unwrap_data(_base, |m| {
                m.acquire_store(|_, store| Ok(store.data().get_registry()?.get(_ix)))
            })
            .flatten();

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            None
        }
    }

    #[method]
    fn registry_set(
        &self,
        #[base] _base: TRef<Reference>,
        _ix: usize,
        _obj: Variant,
    ) -> Option<Variant> {
        #[cfg(feature = "object-registry-compat")]
        return self
            .unwrap_data(_base, |m| {
                m.acquire_store(|_, mut store| {
                    let reg = store.data_mut().get_registry_mut()?;
                    if _obj.is_nil() {
                        Ok(reg.unregister(_ix))
                    } else {
                        Ok(reg.replace(_ix, _obj))
                    }
                })
            })
            .flatten();

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            None
        }
    }

    #[method]
    fn unregister_object(&self, #[base] _base: TRef<Reference>, _ix: usize) -> Option<Variant> {
        #[cfg(feature = "object-registry-compat")]
        return self
            .unwrap_data(_base, |m| {
                m.acquire_store(|_, mut store| {
                    Ok(store.data_mut().get_registry_mut()?.unregister(_ix))
                })
            })
            .flatten();

        #[cfg(not(feature = "object-registry-compat"))]
        {
            godot_error!("Feature object-registry-compat not enabled!");
            None
        }
    }

    #[method]
    fn stdin_add_line(&self, #[base] _base: TRef<Reference>, line: GodotString) {
        #[cfg(feature = "wasi")]
        self.unwrap_data(_base, |m| {
            if let Some(stdin) = &m.wasi_stdin {
                stdin.add_line(line)?;
            }
            Ok(())
        });

        #[cfg(not(feature = "wasi"))]
        godot_error!("Feature wasi not enabled!");
    }

    #[method]
    fn stdin_close(&self, #[base] _base: TRef<Reference>) {
        #[cfg(feature = "wasi")]
        self.unwrap_data(_base, |m| {
            if let Some(stdin) = &m.wasi_stdin {
                stdin.close_pipe();
            }
            Ok(())
        });

        #[cfg(not(feature = "wasi"))]
        godot_error!("Feature wasi not enabled!");
    }

    #[method]
    fn has_memory(&self, #[base] base: TRef<Reference>) -> bool {
        self.unwrap_data(base, |m| m.acquire_store(|_, _| Ok(self.memory.is_some())))
            .unwrap_or_default()
    }

    #[method]
    fn memory_set_name(&self, #[base] base: TRef<Reference>, name: GodotString) -> bool {
        self.unwrap_data(base, |m| {
            m.acquire_store(|m, store| {
                // SAFETY: Nobody else can access memory
                #[allow(mutable_transmutes)]
                unsafe {
                    *mem::transmute::<_, &mut Option<Memory>>(&self.memory) = match &m.instance {
                        InstanceType::Core(inst) => inst.get_memory(store, &name.to_string()),
                        #[allow(unreachable_patterns)]
                        _ => None,
                    };
                }
                Ok(self.memory.is_some())
            })
        })
        .unwrap_or_default()
    }

    #[method]
    fn memory_size(&self, #[base] base: TRef<Reference>) -> usize {
        self.get_memory(base, |store, mem| Ok(mem.data_size(store)))
            .unwrap_or_default()
    }

    #[method]
    fn memory_read(
        &self,
        #[base] base: TRef<Reference>,
        i: usize,
        n: usize,
    ) -> Option<PoolArray<u8>> {
        self.read_memory(base, i, n, |s| Ok(<PoolArray<u8>>::from_slice(s)))
    }

    #[method]
    fn memory_write(&self, #[base] base: TRef<Reference>, i: usize, a: PoolArray<u8>) -> bool {
        let a = &*a.read();
        self.write_memory(base, i, a.len(), |s| {
            s.copy_from_slice(a);
            Ok(())
        })
        .is_some()
    }

    #[method]
    fn get_8(&self, #[base] base: TRef<Reference>, i: usize) -> Option<u8> {
        self.read_memory(base, i, 1, |s| Ok(s[0]))
    }

    #[method]
    fn put_8(&self, #[base] base: TRef<Reference>, i: usize, v: u8) -> bool {
        self.write_memory(base, i, 1, |s| {
            s[0] = v;
            Ok(())
        })
        .is_some()
    }

    #[method]
    fn get_16(&self, #[base] base: TRef<Reference>, i: usize) -> Option<u16> {
        self.read_memory(base, i, 2, |s| {
            Ok(u16::from_le_bytes(s.try_into().unwrap()))
        })
    }

    #[method]
    fn put_16(&self, #[base] base: TRef<Reference>, i: usize, v: u16) -> bool {
        self.write_memory(base, i, 2, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[method]
    fn get_32(&self, #[base] base: TRef<Reference>, i: usize) -> Option<u32> {
        self.read_memory(base, i, 4, |s| {
            Ok(u32::from_le_bytes(s.try_into().unwrap()))
        })
    }

    #[method]
    fn put_32(&self, #[base] base: TRef<Reference>, i: usize, v: u32) -> bool {
        self.write_memory(base, i, 4, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[method]
    fn get_64(&self, #[base] base: TRef<Reference>, i: usize) -> Option<i64> {
        self.read_memory(base, i, 8, |s| {
            Ok(i64::from_le_bytes(s.try_into().unwrap()))
        })
    }

    #[method]
    fn put_64(&self, #[base] base: TRef<Reference>, i: usize, v: i64) -> bool {
        self.write_memory(base, i, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[method]
    fn get_float(&self, #[base] base: TRef<Reference>, i: usize) -> Option<f32> {
        self.read_memory(base, i, 4, |s| {
            Ok(f32::from_le_bytes(s.try_into().unwrap()))
        })
    }

    #[method]
    fn put_float(&self, #[base] base: TRef<Reference>, i: usize, v: f32) -> bool {
        self.write_memory(base, i, 4, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[method]
    fn get_double(&self, #[base] base: TRef<Reference>, i: usize) -> Option<f64> {
        self.read_memory(base, i, 8, |s| {
            Ok(f64::from_le_bytes(s.try_into().unwrap()))
        })
    }

    #[method]
    fn put_double(&self, #[base] base: TRef<Reference>, i: usize, v: f64) -> bool {
        self.write_memory(base, i, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    #[method]
    fn put_array(&self, #[base] base: TRef<Reference>, i: usize, v: Variant) -> bool {
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

        self.get_memory(base, |mut store, mem| {
            let data = mem.data_mut(&mut store);
            match v.dispatch() {
                VariantDispatch::ByteArray(v) => {
                    let s = &*v.read();
                    let e = i + s.len();
                    let Some(d) = data.get_mut(i..e) else {
                        bail_with_site!("Index out of range ({}..{})", i, e);
                    };

                    d.copy_from_slice(s);
                    Ok(())
                }
                VariantDispatch::Int32Array(v) => f::<4, _>(data, i, &v.read(), |s, d| {
                    *d = s.to_le_bytes();
                }),
                VariantDispatch::Float32Array(v) => f::<4, _>(data, i, &v.read(), |s, d| {
                    *d = s.to_le_bytes();
                }),
                VariantDispatch::Vector2Array(v) => f::<8, _>(data, i, &v.read(), |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[4..]).unwrap() = s.y.to_le_bytes();
                }),
                VariantDispatch::Vector3Array(v) => f::<12, _>(data, i, &v.read(), |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[4..8]).unwrap() = s.y.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[8..]).unwrap() = s.z.to_le_bytes();
                }),
                VariantDispatch::ColorArray(v) => f::<16, _>(data, i, &v.read(), |s, d| {
                    *<&mut [u8; 4]>::try_from(&mut d[..4]).unwrap() = s.r.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[4..8]).unwrap() = s.g.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[8..12]).unwrap() = s.b.to_le_bytes();
                    *<&mut [u8; 4]>::try_from(&mut d[12..]).unwrap() = s.a.to_le_bytes();
                }),
                _ => bail_with_site!("Unknown value type {:?}", v.get_type()),
            }
        })
        .is_some()
    }

    #[method]
    fn get_array(
        &self,
        #[base] base: TRef<Reference>,
        i: usize,
        n: usize,
        t: i64,
    ) -> Option<Variant> {
        fn f<const N: usize, T: Send + Copy + PoolElement>(
            s: &[u8],
            i: usize,
            n: usize,
            f: impl Fn(&[u8; N]) -> T + Send + Sync,
        ) -> Result<PoolArray<T>, Error> {
            let e = i + n * N;
            let Some(s) = s.get(i..e) else {
                bail_with_site!("Index out of range ({}..{})", i, e);
            };

            Ok(PoolArray::from_vec(
                s.par_chunks_exact(N)
                    .map(|s| f(s.try_into().unwrap()))
                    .collect(),
            ))
        }

        self.get_memory(base, |store, mem| {
            let data = mem.data(&store);
            match t {
                20 => {
                    // PoolByteArray
                    let e = i + n;
                    let Some(s) = data.get(i..e) else {
                        bail_with_site!("Index out of range ({}..{})", i, e);
                    };

                    Ok(Variant::new(PoolArray::from_slice(s)))
                }
                21 => Ok(Variant::new(f::<4, _>(data, i, n, |s| {
                    i32::from_le_bytes(*s)
                })?)), // PoolInt32Array
                22 => Ok(Variant::new(f::<4, _>(data, i, n, |s| {
                    f32::from_le_bytes(*s)
                })?)), // PoolFloat32Array
                24 => Ok(Variant::new(f::<8, _>(data, i, n, |s| Vector2 {
                    x: f32::from_le_bytes(s[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(s[4..].try_into().unwrap()),
                })?)), // PoolVector2Array
                25 => Ok(Variant::new(f::<12, _>(data, i, n, |s| Vector3 {
                    x: f32::from_le_bytes(s[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(s[4..8].try_into().unwrap()),
                    z: f32::from_le_bytes(s[8..].try_into().unwrap()),
                })?)), // PoolVector3Array
                26 => Ok(Variant::new(f::<16, _>(data, i, n, |s| Color {
                    r: f32::from_le_bytes(s[..4].try_into().unwrap()),
                    g: f32::from_le_bytes(s[4..8].try_into().unwrap()),
                    b: f32::from_le_bytes(s[8..12].try_into().unwrap()),
                    a: f32::from_le_bytes(s[12..].try_into().unwrap()),
                })?)), // PoolColorArray
                ..=26 => bail_with_site!("Unsupported type ID {}", t),
                _ => bail_with_site!("Unknown type {}", t),
            }
        })
    }

    #[method]
    fn read_struct(
        &self,
        #[base] base: TRef<Reference>,
        format: GodotString,
        p: usize,
    ) -> Option<VariantArray> {
        self.get_memory(base, |store, mem| {
            read_struct(mem.data(store), p, &format.to_string())
        })
    }

    #[method]
    fn write_struct(
        &self,
        #[base] base: TRef<Reference>,
        format: GodotString,
        p: usize,
        arr: VariantArray,
    ) -> usize {
        self.get_memory(base, |store, mem| {
            write_struct(mem.data_mut(store), p, &format.to_string(), arr)
        })
        .unwrap_or_default()
    }
}
