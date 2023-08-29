use std::any::Any;
use std::collections::HashMap;
use std::mem::{size_of, transmute};
use std::ptr;
use std::sync::Arc;

use anyhow::{bail, Error};
use godot::prelude::*;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, Once, OnceState, RawMutex};
use scopeguard::guard;
#[cfg(feature = "wasi")]
use wasmtime::Linker;
#[cfg(feature = "memory-limiter")]
use wasmtime::ResourceLimiter;
use wasmtime::{
    AsContextMut, Extern, Instance as InstanceWasm, Memory, Store, StoreContextMut, UpdateDeadline,
    ValRaw,
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
use crate::wasm_engine::{ModuleData, WasmModule, ENGINE};
#[cfg(feature = "object-registry-extern")]
use crate::wasm_externref::EXTERNREF_LINKER;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_objregistry::{ObjectRegistry, OBJREGISTRY_LINKER};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_DEADLINE;
#[cfg(feature = "object-registry-extern")]
use crate::wasm_util::EXTERNREF_MODULE;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_util::OBJREGISTRY_MODULE;
use crate::wasm_util::{
    from_raw, make_host_module, option_to_variant, to_raw, variant_to_option, HOST_MODULE,
    MEMORY_EXPORT,
};
use crate::{bail_with_site, site_context};

#[derive(GodotClass)]
#[class(base=RefCounted, init)]
pub struct WasmInstance {
    #[base]
    base: Base<RefCounted>,
    once: Once,
    data: Option<InstanceData>,

    #[var(get = get_module)]
    #[allow(dead_code)]
    module: Option<i64>,
}

pub struct InstanceData {
    store: Mutex<Store<StoreData>>,
    instance: InstanceWasm,
    module: Gd<WasmModule>,

    #[cfg(feature = "wasi")]
    wasi_stdin: Option<Arc<InnerStdin<dyn Any + Send + Sync>>>,
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

impl InstanceData {
    pub fn instantiate(
        mut store: Store<StoreData>,
        module: Gd<WasmModule>,
        host: Option<Dictionary>,
    ) -> Result<Self, Error> {
        #[cfg(feature = "epoch-timeout")]
        if store.data().config.with_epoch {
            store.epoch_deadline_trap();
            EPOCH.spawn_thread(|| ENGINE.increment_epoch());
        } else {
            store.epoch_deadline_callback(|_| Ok(UpdateDeadline::Continue(EPOCH_DEADLINE)));
        }

        #[cfg(feature = "memory-limiter")]
        {
            if let Some(v) = store.data().config.max_memory {
                store.data_mut().memory_limits.max_memory = v;
            }
            if let Some(v) = store.data().config.max_entries {
                store.data_mut().memory_limits.max_table_entries = v;
            }
            store.limiter(|data| &mut data.memory_limits);
        }

        #[cfg(feature = "wasi")]
        let mut wasi_stdin = None;

        #[cfg(feature = "wasi")]
        let wasi_linker = if store.data().config.with_wasi {
            let mut builder = WasiCtxBuilder::new();

            let StoreData {
                wasi_ctx, config, ..
            } = store.data_mut();

            if config.wasi_stdin == PipeBindingType::Instance {
                if let Some(data) = config.wasi_stdin_data.clone() {
                    builder = builder.stdin(Box::new(ByteBufferReadPipe::new(data)));
                } else {
                    // TODO: Emit signal
                    let (outer, inner) = OuterStdin::new(move || {});
                    builder = builder.stdin(Box::new(outer));
                    wasi_stdin = Some(inner as _);
                }
            }
            if config.wasi_stdout == PipeBindingType::Instance {
                builder = builder.stdout(match config.wasi_stdout_buffer {
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
                builder = builder.stderr(match config.wasi_stderr_buffer {
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
                Some(ctx) => Some(WasiContext::build_ctx(ctx.share(), builder, &*config)?),
                None => Some(WasiContext::init_ctx_no_context(
                    builder.inherit_stdout().inherit_stderr().build(),
                    &*config,
                )?),
            };
            let mut r = <Linker<StoreData>>::new(&ENGINE);
            add_to_linker(&mut r, |data| data.wasi_ctx.as_mut().unwrap())?;
            Some(r)
        } else {
            None
        };

        #[allow(unreachable_patterns)]
        match store.data().config.extern_bind {
            ExternBindingType::None => (),
            #[cfg(feature = "object-registry-compat")]
            ExternBindingType::Registry => {
                store.data_mut().object_registry = Some(ObjectRegistry::default());
            }
            _ => panic!("Unimplemented binding"),
        }

        type InstMap = HashMap<InstanceId, InstanceWasm>;

        fn f(
            store: &mut Store<StoreData>,
            module: &ModuleData,
            insts: &mut InstMap,
            host: &Option<HashMap<String, Extern>>,
            #[cfg(feature = "wasi")] wasi_linker: Option<&Linker<StoreData>>,
        ) -> Result<InstanceWasm, Error> {
            let it = module.module.imports();
            let mut imports = Vec::with_capacity(it.len());

            for i in it {
                match (i.module(), &store.data().config) {
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
                        if let Some(v) = OBJREGISTRY_LINKER.get_by_import(&mut *store, &i) {
                            imports.push(v);
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
                        if let Some(v) = EXTERNREF_LINKER.get_by_import(&mut *store, &i) {
                            imports.push(v);
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
                                let t = f(
                                    &mut *store,
                                    v.bind().get_data()?,
                                    &mut *insts,
                                    host,
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

                bail!("Unknown import {:?}.{:?}", i.module(), i.name());
            }

            #[cfg(feature = "epoch-timeout")]
            store.set_epoch_deadline(store.data().config.epoch_timeout);
            InstanceWasm::new(store, &module.module, &imports)
        }

        let host = host.map(|h| make_host_module(&mut store, h)).transpose()?;

        let sp = &mut store;
        let instance = f(
            sp,
            module.bind().get_data()?,
            &mut HashMap::new(),
            &host,
            #[cfg(feature = "wasi")]
            wasi_linker.as_ref(),
        )?;

        Ok(Self {
            instance,
            module,
            store: Mutex::new(store),
            #[cfg(feature = "wasi")]
            wasi_stdin,
        })
    }

    fn acquire_store<F, R>(&self, f: F) -> R
    where
        for<'a> F: FnOnce(&Self, StoreContextMut<'a, StoreData>) -> R,
    {
        let mut guard_ = self.store.lock();

        let _scope;
        // SAFETY: Context should be destroyed after function call
        unsafe {
            let p = &mut guard_.data_mut().mutex_raw as *mut _;
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
    pub fn get_data(&self) -> Result<&InstanceData, Error> {
        if let OnceState::Done = self.once.state() {
            Ok(self.data.as_ref().unwrap())
        } else {
            bail_with_site!("Uninitialized module")
        }
    }

    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&InstanceData) -> Result<R, Error>,
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
        let mut r = true;
        let ret = &mut r;

        self.once.call_once(move || {
            match InstanceData::instantiate(
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
                        transmute::<&Option<InstanceData>, &mut Option<InstanceData>>(&self.data)
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

    fn get_memory<F, R>(&self, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(StoreContextMut<'a, StoreData>, Memory) -> Result<R, Error>,
    {
        self.unwrap_data(|m| {
            m.acquire_store(
                |m, mut store| match m.instance.get_memory(&mut store, MEMORY_EXPORT) {
                    Some(mem) => f(store, mem),
                    None => bail_with_site!("No memory exported"),
                },
            )
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
    ) -> Gd<WasmInstance> {
        let Ok(host) = variant_to_option::<Dictionary>(host) else {
            panic!("Host is not a dictionary!")
        };
        let config = if config.is_nil() { None } else { Some(config) };

        let ret = if self.initialize_(module, host, config) {
            <Gd<WasmInstance>>::try_from_instance_id(self.base.instance_id())
        } else {
            None
        };
        ret.unwrap()
    }

    #[func]
    fn get_module(&self) -> Variant {
        match self.unwrap_data(|m| Ok(m.module.share())) {
            Some(v) => v.to_variant(),
            None => Variant::nil(),
        }
    }

    #[func]
    fn call_wasm(&self, name: StringName, args: Array<Variant>) -> Array<Variant> {
        self.unwrap_data(move |m| {
            m.acquire_store(move |m, mut store| {
                let name = name.to_string();
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
    fn signal_error(&self, msg: StringName) -> Variant {
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
                    m.instance.get_export(&mut store, MEMORY_EXPORT),
                    Some(Extern::Memory(_))
                ))
            })
        })
        .unwrap_or_default()
    }

    #[func]
    fn stdin_add_line(&self, line: GodotString) {
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
        fn f<T: Copy>(d: &mut [u8], i: usize, s: &[T]) -> Result<(), Error> {
            let l = s.len() * size_of::<T>();
            let e = i + l;

            if let Some(d) = d.get_mut(i..e) {
                let ps = s.as_ptr() as *const u8;
                let pd = d.as_mut_ptr();

                // SAFETY: Source and destination is of the same size.
                // alignment of destination should be enforced externally.
                unsafe {
                    ptr::copy_nonoverlapping(ps, pd, l);
                }

                #[cfg(target_endian = "big")]
                if size_of::<T>() > 1 {
                    for d in d.chunks_mut(size_of::<T>()) {
                        debug_assert_eq!(d.len(), size_of::<T>());
                        d.reverse();
                    }
                }
            } else {
                bail_with_site!("Index out of range ({}..{})", i, e);
            }

            Ok(())
        }

        self.get_memory(|mut store, mem| {
            let i = i as usize;
            let data = mem.data_mut(&mut store);
            if let Ok(v) = PackedByteArray::try_from_variant(&v) {
                f(data, i, &v.to_vec())
            } else if let Ok(v) = PackedInt32Array::try_from_variant(&v) {
                f(data, i, &v.to_vec())
            } else if let Ok(v) = PackedFloat32Array::try_from_variant(&v) {
                f(data, i, &v.to_vec())
            } else if let Ok(v) = PackedVector2Array::try_from_variant(&v) {
                f(data, i, &v.to_vec())
            } else if let Ok(v) = PackedVector3Array::try_from_variant(&v) {
                f(data, i, &v.to_vec())
            } else if let Ok(v) = PackedColorArray::try_from_variant(&v) {
                f(data, i, &v.to_vec())
            } else {
                bail_with_site!("Unknown value")
            }
        })
        .is_some()
    }

    #[func]
    fn get_array(&self, i: i64, n: i64, t: i64) -> Variant {
        fn f<T, A>(s: &[u8], i: usize, n: usize) -> Result<A, Error>
        where
            T: Copy,
            for<'a> A: From<&'a [T]>,
        {
            let l = n * size_of::<T>();
            let e = i + l;

            if let Some(s) = s.get(i..e) {
                let mut d = Vec::with_capacity(n);

                let ps = s.as_ptr();
                let pd = d.spare_capacity_mut().as_mut_ptr() as *mut u8;

                // SAFETY: Source and destination are of same size.
                // alignment of source should be enforced externally.
                unsafe {
                    ptr::copy_nonoverlapping(ps, pd, l);

                    #[cfg(target_endian = "big")]
                    if size_of::<T>() > 1 {
                        // SAFETY: destination size is l
                        for d in ptr::slice_from_raw_parts_mut(pd, l).chunks_mut(size_of::<T>()) {
                            debug_assert_eq!(d.len(), size_of::<T>());
                            d.reverse();
                        }
                    }

                    // SAFETY: value is initialized
                    d.set_len(n);
                }

                Ok(A::from(&d))
            } else {
                bail_with_site!("Index out of range ({}..{})", i, e);
            }
        }

        option_to_variant(self.get_memory(|store, mem| {
            let (i, n) = (i as usize, n as usize);
            let data = mem.data(&store);
            match t {
                20 => f::<u8, PackedByteArray>(data, i, n).map(Variant::from), // PoolByteArray
                21 => f::<i32, PackedInt32Array>(data, i, n).map(Variant::from), // PoolInt32Array
                22 => f::<f32, PackedFloat32Array>(data, i, n).map(Variant::from), // PoolFloat32Array
                24 => f::<Vector2, PackedVector2Array>(data, i, n).map(Variant::from), // PoolVector2Array
                25 => f::<Vector3, PackedVector3Array>(data, i, n).map(Variant::from), // PoolVector3Array
                26 => f::<Color, PackedColorArray>(data, i, n).map(Variant::from), // PoolColorArray
                ..=26 => bail_with_site!("Unsupported type ID {}", t),
                _ => bail_with_site!("Unknown type {}", t),
            }
        }))
    }
}
