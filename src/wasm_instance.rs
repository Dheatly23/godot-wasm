use std::any::Any;
use std::collections::HashMap;
use std::mem::transmute;
use std::ptr;
use std::sync::Arc;

use anyhow::{bail, Error};
use gdnative::core_types::PoolElement;
use gdnative::export::user_data::Map;
use gdnative::log::{error, godot_site, Site};
use gdnative::prelude::*;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, Once, OnceState, RawMutex};
use scopeguard::guard;
#[cfg(feature = "wasi")]
use wasmtime::Linker;
use wasmtime::{
    AsContextMut, Extern, Instance as InstanceWasm, Memory, ResourceLimiter, Store,
    StoreContextMut, UpdateDeadline, ValRaw,
};
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
use crate::wasm_config::{Config, ExternBindingType};
#[cfg(feature = "wasi")]
use crate::wasm_config::{PipeBindingType, PipeBufferType};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_engine::EPOCH;
use crate::wasm_engine::{ModuleData, ModuleType, WasmModule, ENGINE};
#[cfg(feature = "object-registry-extern")]
use crate::wasm_externref::Funcs as ExternrefFuncs;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_objregistry::{Funcs as ObjregistryFuncs, ObjectRegistry};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_DEADLINE;
#[cfg(feature = "object-registry-extern")]
use crate::wasm_util::EXTERNREF_MODULE;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_util::OBJREGISTRY_MODULE;
use crate::wasm_util::{from_raw, make_host_module, to_raw, HOST_MODULE, MEMORY_EXPORT};
use crate::{bail_with_site, site_context};

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::ArcData<WasmInstance>)]
pub struct WasmInstance {
    once: Once,
    data: Option<InstanceData<StoreData>>,
}

pub struct InstanceData<T> {
    pub store: Mutex<Store<T>>,
    pub instance: InstanceWasm,
    pub module: Instance<WasmModule, Shared>,

    #[cfg(feature = "wasi")]
    pub wasi_stdin: Option<Arc<InnerStdin<dyn Any + Send + Sync>>>,
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
    pub wasi_ctx: Option<WasiCtx>,
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

#[cfg(feature = "memory-limiter")]
pub struct MemoryLimit {
    max_memory: u64,
    max_table_entries: u64,
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
        owner: &Reference,
        mut store: Store<T>,
        module: Instance<WasmModule, Shared>,
        host: Option<Dictionary>,
    ) -> Result<Self, Error> {
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
                    builder.stdin(Box::new(ByteBufferReadPipe::new(data)));
                } else {
                    let (outer, inner) = OuterStdin::new(move || unsafe {
                        let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                            return;
                        };
                        owner.emit_signal("stdin_request", &[]);
                    });
                    builder.stdin(Box::new(outer));
                    wasi_stdin = Some(inner as _);
                }
            }
            if config.wasi_stdout == PipeBindingType::Instance {
                builder.stdout(match config.wasi_stdout_buffer {
                    PipeBufferType::Unbuffered => {
                        Box::new(UnbufferedWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stdout_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        })) as _
                    }
                    PipeBufferType::LineBuffer => Box::new(LineWritePipe::new(move |buf| unsafe {
                        let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                            return;
                        };
                        owner.emit_signal(
                            "stdout_emit",
                            &[String::from_utf8_lossy(buf).to_variant()],
                        );
                    })) as _,
                    PipeBufferType::BlockBuffer => {
                        Box::new(BlockWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stdout_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        })) as _
                    }
                });
            }
            if config.wasi_stderr == PipeBindingType::Instance {
                builder.stderr(match config.wasi_stderr_buffer {
                    PipeBufferType::Unbuffered => {
                        Box::new(UnbufferedWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stderr_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        })) as _
                    }
                    PipeBufferType::LineBuffer => Box::new(LineWritePipe::new(move |buf| unsafe {
                        let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                            return;
                        };
                        owner.emit_signal(
                            "stderr_emit",
                            &[String::from_utf8_lossy(buf).to_variant()],
                        );
                    })) as _,
                    PipeBufferType::BlockBuffer => {
                        Box::new(BlockWritePipe::new(move |buf| unsafe {
                            let Some(owner) = Reference::try_from_instance_id(inst_id) else {
                                return;
                            };
                            owner.emit_signal(
                                "stderr_emit",
                                &[<PoolArray<u8>>::from_slice(buf).owned_to_variant()],
                            );
                        })) as _
                    }
                });
            }

            *wasi_ctx = match &config.wasi_context {
                Some(ctx) => Some(WasiContext::build_ctx(ctx.clone(), builder, &*config)?),
                None => Some(WasiContext::init_ctx_no_context(
                    builder.inherit_stdout().inherit_stderr().build(),
                    &*config,
                )?),
            };
            let mut r = <Linker<T>>::new(&ENGINE);
            add_to_linker(&mut r, |data| data.as_mut().wasi_ctx.as_mut().unwrap())?;
            Some(r)
        } else {
            None
        };

        #[cfg(feature = "object-registry-compat")]
        if store.data().as_ref().config.extern_bind == ExternBindingType::Registry {
            store.data_mut().as_mut().object_registry = Some(ObjectRegistry::default());
        }

        let host = host.map(|h| make_host_module(&mut store, h)).transpose()?;

        let sp = &mut store;
        let instance = module
            .script()
            .map(move |m| {
                let mut insts = HashMap::new();
                Self::instantiate_wasm(
                    sp,
                    m.get_data()?,
                    &mut insts,
                    &host,
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
            instance,
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
        host: &Option<HashMap<String, Extern>>,
        #[cfg(feature = "object-registry-compat")] objregistry_funcs: &mut ObjregistryFuncs,
        #[cfg(feature = "object-registry-extern")] externref_funcs: &mut ExternrefFuncs,
        #[cfg(feature = "wasi")] wasi_linker: Option<&Linker<T>>,
    ) -> Result<InstanceWasm, Error> {
        let ModuleType::Core(module_) = &module.module else {
            bail_with_site!("Cannot instantiate component")
        };
        let it = module_.imports();
        let mut imports = Vec::with_capacity(it.len());

        for i in it {
            match (i.module(), &store.data().as_ref().config) {
                (HOST_MODULE, _) => {
                    if let Some(host) = host.as_ref() {
                        if let Some(v) = host.get(i.name()) {
                            imports.push(v.clone());
                            continue;
                        }
                    }
                }
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
                                        host,
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

            bail!("Unknown import {:?}.{:?}", i.module(), i.name());
        }

        #[cfg(feature = "epoch-timeout")]
        store.set_epoch_deadline(store.data().as_ref().config.epoch_timeout);
        InstanceWasm::new(store, &module_, &imports)
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
            once: Once::new(),
            data: None,
        }
    }

    pub fn get_data(&self) -> Result<&InstanceData<StoreData>, Error> {
        if let OnceState::Done = self.once.state() {
            Ok(self.data.as_ref().unwrap())
        } else {
            bail_with_site!("Uninitialized module")
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
        let mut r = true;
        let ret = &mut r;

        self.once.call_once(move || {
            match InstanceData::instantiate(
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
                        wasi_ctx: None,
                    },
                ),
                module,
                host,
            ) {
                Ok(v) => {
                    // SAFETY: Should be called only once and nobody else can read module data
                    #[allow(mutable_transmutes)]
                    let data = unsafe {
                        transmute::<
                            &Option<InstanceData<StoreData>>,
                            &mut Option<InstanceData<StoreData>>,
                        >(&self.data)
                    };
                    *data = Some(v);
                }
                Err(e) => {
                    godot_error!("{}", e);
                    *ret = false;
                }
            }
        });

        r
    }

    fn get_memory<F, R>(&self, base: TRef<Reference>, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(StoreContextMut<'a, StoreData>, Memory) -> Result<R, Error>,
    {
        self.unwrap_data(base, |m| {
            m.acquire_store(
                |m, mut store| match m.instance.get_memory(&mut store, MEMORY_EXPORT) {
                    Some(mem) => f(store, mem),
                    None => bail_with_site!("No memory exported"),
                },
            )
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
                None => bail!("Index out of bound {}-{}", i, i + n),
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
                let f = match m.instance.get_export(&mut store, &name) {
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
        self.unwrap_data(base, |m| {
            m.acquire_store(|m, mut store| {
                Ok(matches!(
                    m.instance.get_export(&mut store, MEMORY_EXPORT),
                    Some(Extern::Memory(_))
                ))
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
        fn f<const N: usize, T>(
            d: &mut [u8],
            i: usize,
            s: &[T],
            f: impl Fn(&T, &mut [u8; N]),
        ) -> Result<(), Error> {
            let l = s.len() * N;
            let e = i + l;

            let Some(d) = d.get_mut(i..e) else {
                bail_with_site!("Index out of range ({}..{})", i, e);
            };

            for (s, d) in s.iter().zip(d.chunks_mut(N)) {
                f(s, d.try_into().unwrap())
            }

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
                    d[..4].copy_from_slice(&s.x.to_le_bytes());
                    d[4..].copy_from_slice(&s.y.to_le_bytes());
                }),
                VariantDispatch::Vector3Array(v) => f::<12, _>(data, i, &v.read(), |s, d| {
                    d[..4].copy_from_slice(&s.x.to_le_bytes());
                    d[4..8].copy_from_slice(&s.y.to_le_bytes());
                    d[8..].copy_from_slice(&s.z.to_le_bytes());
                }),
                VariantDispatch::ColorArray(v) => f::<16, _>(data, i, &v.read(), |s, d| {
                    d[..4].copy_from_slice(&s.r.to_le_bytes());
                    d[4..8].copy_from_slice(&s.g.to_le_bytes());
                    d[8..12].copy_from_slice(&s.b.to_le_bytes());
                    d[12..].copy_from_slice(&s.a.to_le_bytes());
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
        fn f<const N: usize, T: Copy + PoolElement>(
            s: &[u8],
            i: usize,
            n: usize,
            f: impl Fn(&[u8; N]) -> T,
        ) -> Result<PoolArray<T>, Error> {
            let l = n * N;
            let e = i + l;
            let Some(s) = s.get(i..e) else {
                bail_with_site!("Index out of range ({}..{})", i, e);
            };

            Ok(s.chunks(N).map(|s| f(s.try_into().unwrap())).collect())
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
}
