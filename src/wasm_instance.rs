use std::collections::hash_map::{Entry, HashMap};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::{ffi, fmt, mem, ptr};

use anyhow::{bail, Result as AnyResult};
use cfg_if::cfg_if;
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, RawMutex};
use rayon::prelude::*;
use scopeguard::guard;
use tracing::{debug, debug_span, error, info, instrument, trace_span, warn, Level};
#[cfg(feature = "wasi")]
use wasi_isolated_fs::bindings::wasi_snapshot_preview1::add_to_linker;
#[cfg(feature = "wasi")]
use wasi_isolated_fs::context::WasiContext as WasiCtx;
#[cfg(feature = "wasi")]
use wasi_isolated_fs::stdio::StdinProvider;
#[cfg(feature = "component-model")]
use wasmtime::component::Instance as InstanceComp;
#[cfg(feature = "wasi")]
use wasmtime::Linker;
#[cfg(feature = "memory-limiter")]
use wasmtime::ResourceLimiter;
use wasmtime::{
    AsContextMut, Extern, Func, FuncType, Instance as InstanceWasm, Memory, SharedMemory, Store,
    StoreContextMut,
};

use crate::godot_util::{
    option_to_variant, variant_to_option, PackedArrayLike, PhantomProperty, SendSyncWrapper,
    StructPacking,
};
use crate::rw_struct::{read_struct, write_struct};
#[cfg(feature = "wasi")]
use crate::wasi_ctx::stdio::PackedByteArrayReader;
#[cfg(feature = "wasi")]
use crate::wasi_ctx::WasiContext;
use crate::wasm_config::Config;
#[cfg(any(feature = "object-registry-compat", feature = "object-registry-extern"))]
use crate::wasm_config::ExternBindingType;
#[cfg(feature = "wasi")]
use crate::wasm_config::{PipeBindingType, PipeBufferType};
use crate::wasm_engine::{get_engine, ModuleData, ModuleType, WasmModule};
#[cfg(feature = "object-registry-extern")]
use crate::wasm_externref::Funcs as ExternrefFuncs;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_objregistry::{Funcs as ObjregistryFuncs, ObjectRegistry};
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::reset_epoch;
#[cfg(feature = "object-registry-extern")]
use crate::wasm_util::EXTERNREF_MODULE;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_util::OBJREGISTRY_MODULE;
use crate::wasm_util::{
    config_store_common, raw_call, HasEpochTimeout, HostModuleCache, MEMORY_EXPORT,
};
use crate::{bail_with_site, site_context, variant_dispatch};

enum MemoryType {
    Memory(Memory),
    SharedMemory(SharedMemory),
}

#[derive(GodotClass)]
#[class(base=RefCounted, init, tool)]
/// Class for WebAssembly instance.
///
/// Instantiate `WasmModule` to be able to call into WebAssembly.
/// Unlike `WasmModule`, `WasmInstance` can't be serialized.
///
/// ## Struct Format String
///
/// The format string used for `read_struct()` and `write_struct()` is defined as a list of items.
/// Each item contains a type, optionally preceded by a repetition count.
/// The valid types are as follows:
///
/// | String | Godot Type | Byte Length | Description |
/// |:------:|:----------:|:-----------:|:------------|
/// | `x` | | 1 | Padding byte, will not be read/written. Padding bytes are not automatically added. |
/// | `b` | `int` | 1 | Signed 8-bit number |
/// | `B` | `int` | 1 | Unsigned 8-bit number |
/// | `h` | `int` | 2 | Signed 16-bit number |
/// | `H` | `int` | 2 | Unsigned 16-bit number |
/// | `i` | `int` | 4 | Signed 32-bit number |
/// | `I` | `int` | 4 | Unsigned 32-bit number |
/// | `l` | `int` | 8 | Signed 64-bit number |
/// | `L` | `int` | 8 | Unsigned 64-bit number |
/// | `f` | `float` | 4 | 32-bit floating-point number |
/// | `d` | `float` | 8 | 64-bit floating-point number |
/// | `v2f` | `Vector2` | 8 | 2D vector as 2 32-bit floating-point number |
/// | `v2d` | `Vector2` | 16 | 2D vector as 2 64-bit floating-point number |
/// | `v2i` | `Vector2i` | 8 | 2D vector as 2 32-bit signed integer number |
/// | `v2l` | `Vector2i` | 16 | 2D vector as 2 64-bit signed integer number |
/// | `v3f` | `Vector3` | 12 | 3D vector as a 3 32-bit floating-point number |
/// | `v3d` | `Vector3` | 24 | 3D vector as a 3 64-bit floating-point number |
/// | `v3i` | `Vector3i` | 12 | 3D vector as a 3 32-bit signed integer number |
/// | `v3l` | `Vector3i` | 24 | 3D vector as a 3 64-bit signed integer number |
/// | `v4f` | `Vector4` | 12 | 4D vector as a 4 32-bit floating-point number |
/// | `v4d` | `Vector4` | 24 | 4D vector as a 4 64-bit floating-point number |
/// | `v4i` | `Vector4i` | 12 | 4D vector as a 4 32-bit signed integer number |
/// | `v4l` | `Vector4i` | 24 | 4D vector as a 4 64-bit signed integer number |
/// | `pf` | `Plane` | 16 | Plane represented as abcd 32-bit floating-point number |
/// | `pd` | `Plane` | 32 | Plane represented as abcd 64-bit floating-point number |
/// | `qf` | `Quat` | 16 | Quaternion represented as xyzw 32-bit floating-point number |
/// | `qd` | `Quat` | 32 | Quaternion represented as xyzw 64-bit floating-point number |
/// | `Cf` | `Color` | 16 | Color represented as rgba 32-bit floating-point number |
/// | `Cd` | `Color` | 32 | Color represented as rgba 64-bit floating-point number |
/// | `Cb` | `Color` | 4 | Color represented as rgba 8-bit integer |
/// | `rf` | `Rect2` | 16 | Rect2 represented as 4 32-bit floating-point number |
/// | `rd` | `Rect2` | 32 | Rect2 represented as 4 64-bit floating-point number |
/// | `ri` | `Rect2i` | 16 | Rect2i represented as 4 32-bit signed integer number |
/// | `rl` | `Rect2i` | 32 | Rect2i represented as 4 64-bit signed integer number |
/// | `af` | `Aabb` | 24 | Aabb represented as 6 32-bit floating-point number |
/// | `ad` | `Aabb` | 48 | Aabb represented as 6 64-bit floating-point number |
/// | `mf` | `Basis` | 36 | Basis represented as 9 row-major 32-bit floating-point number |
/// | `md` | `Basis` | 72 | Basis represented as 9 row-major 64-bit floating-point number |
/// | `Mf` | `Projection` | 64 | Projection represented as 16 column-major 32-bit floating-point number |
/// | `Md` | `Projection` | 128 | Projection represented as 16 column-major 64-bit floating-point number |
/// | `tf` | `Transform2D` | 24 | 2D transform represented as 6 32-bit floating-point number |
/// | `td` | `Transform2D` | 48 | 2D transform represented as 6 64-bit floating-point number |
/// | `Tf` | `Transform2D` | 48 | 3D transform represented as 12 32-bit floating-point number |
/// | `Td` | `Transform2D` | 96 | 3D transform represented as 12 64-bit floating-point number |
pub struct WasmInstance {
    base: Base<RefCounted>,
    data: OnceCell<InstanceData<StoreData>>,
    memory: Option<MemoryType>,

    /// Reference to the module that is used to instantiate this object.
    #[var(get = get_module)]
    #[allow(dead_code)]
    module: PhantomProperty<Option<Gd<WasmModule>>>,
}

impl Debug for WasmInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_tuple("WasmInstance").field(&self.base).finish()
    }
}

pub struct InstanceData<T> {
    pub store: Mutex<Store<T>>,
    pub instance: InstanceType,
    pub module: Gd<WasmModule>,

    #[cfg(feature = "wasi")]
    pub wasi_stdin: Option<StdinProvider>,
}

#[allow(dead_code)]
pub enum InstanceType {
    NoInstance,
    Core(InstanceWasm),
    #[cfg(feature = "component-model")]
    Component(InstanceComp),
}

impl InstanceType {
    pub fn get_core(&self) -> AnyResult<&InstanceWasm> {
        if let Self::Core(m) = self {
            Ok(m)
        } else {
            bail!("Instance is not a core instance")
        }
    }

    #[allow(dead_code)]
    #[cfg(feature = "component-model")]
    pub fn get_component(&self) -> AnyResult<&InstanceComp> {
        if let Self::Component(m) = self {
            Ok(m)
        } else {
            bail!("Instance is not a component")
        }
    }
}

pub struct InnerLock {
    mutex_raw: *const RawMutex,
}

// SAFETY: Store data is safely contained within instance data?
unsafe impl Send for InnerLock {}
unsafe impl Sync for InnerLock {}

impl Default for InnerLock {
    fn default() -> Self {
        Self {
            mutex_raw: ptr::null(),
        }
    }
}

#[derive(Default)]
pub struct StoreData {
    inner_lock: InnerLock,
    pub error_signal: Option<String>,

    #[cfg(feature = "epoch-timeout")]
    pub epoch_timeout: u64,
    #[cfg(feature = "epoch-timeout")]
    pub epoch_autoreset: bool,

    #[cfg(feature = "memory-limiter")]
    pub memory_limits: MemoryLimit,

    #[cfg(feature = "object-registry-compat")]
    pub object_registry: Option<ObjectRegistry>,

    #[cfg(feature = "object-registry-extern")]
    pub use_extern: bool,

    #[cfg(feature = "wasi")]
    pub wasi_ctx: Option<WasiCtx>,
}

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

impl AsRef<InnerLock> for StoreData {
    fn as_ref(&self) -> &InnerLock {
        &self.inner_lock
    }
}

impl AsMut<InnerLock> for StoreData {
    fn as_mut(&mut self) -> &mut InnerLock {
        &mut self.inner_lock
    }
}

impl HasEpochTimeout for StoreData {
    #[cfg(feature = "epoch-timeout")]
    fn get_epoch_timeout(&self) -> u64 {
        self.epoch_timeout
    }

    #[cfg(feature = "wasi")]
    fn get_wasi_ctx(&mut self) -> Option<&mut WasiCtx> {
        self.wasi_ctx.as_mut()
    }
}

#[cfg(feature = "memory-limiter")]
pub struct MemoryLimit {
    pub max_memory: u64,
    pub max_table_entries: u64,
}

#[cfg(feature = "memory-limiter")]
impl Default for MemoryLimit {
    fn default() -> Self {
        Self {
            max_memory: u64::MAX,
            max_table_entries: u64::MAX,
        }
    }
}

#[cfg(feature = "memory-limiter")]
impl MemoryLimit {
    pub fn from_config(config: &Config) -> Self {
        let mut ret = Self::default();
        if let Some(v) = config.max_memory {
            ret.max_memory = v;
        }
        if let Some(v) = config.max_entries {
            ret.max_table_entries = v;
        }
        ret
    }
}

#[cfg(feature = "memory-limiter")]
impl ResourceLimiter for MemoryLimit {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        max: Option<usize>,
    ) -> AnyResult<bool> {
        if max.is_some_and(|max| desired > max) {
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
        current: usize,
        desired: usize,
        max: Option<usize>,
    ) -> AnyResult<bool> {
        if max.is_some_and(|max| desired > max) {
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

struct InstanceArgs<'a, T> {
    store: StoreContextMut<'a, T>,
    #[allow(dead_code)]
    config: &'a Config,
    insts: HashMap<InstanceId, Option<InstanceWasm>>,
    host: Option<HostModuleCache<T>>,
    #[cfg(feature = "object-registry-compat")]
    objregistry_funcs: ObjregistryFuncs,
    #[cfg(feature = "object-registry-extern")]
    externref_funcs: ExternrefFuncs,
    #[cfg(feature = "wasi")]
    wasi_linker: Option<Linker<T>>,
}

impl<T> InstanceData<T>
where
    T: Send + AsRef<StoreData> + AsMut<StoreData> + HasEpochTimeout,
{
    #[instrument(level = Level::DEBUG, skip_all, fields(?obj, ?module))]
    pub fn instantiate<C: GodotClass>(
        obj: &Gd<C>,
        mut store: Store<T>,
        config: &Config,
        module: Gd<WasmModule>,
        host: Option<Dictionary>,
    ) -> AnyResult<Self> {
        config_store_common(&mut store, config)?;

        #[cfg(feature = "wasi")]
        let mut wasi_stdin = None;

        #[cfg(feature = "wasi")]
        let mut wasi_linker = None;
        #[cfg(feature = "wasi")]
        if config.with_wasi {
            let _s = debug_span!("instantiate.wasi").entered();
            let mut builder = WasiCtx::builder();

            let StoreData { wasi_ctx, .. } = store.data_mut().as_mut();

            if config.wasi_stdin == PipeBindingType::Instance {
                if let Some(data) = config.wasi_stdin_data.clone() {
                    let data = SendSyncWrapper::new(data);
                    builder.stdin_read_builder(Box::new(move || {
                        Ok(Box::new(PackedByteArrayReader::from((*data).clone())))
                    }))
                } else {
                    let signal =
                        SendSyncWrapper::new(Signal::from_object_signal(obj, c"stdin_request"));
                    builder.stdin_signal(Box::new(move || signal.emit(&[])))
                }?;
            }
            if config.wasi_stdout == PipeBindingType::Instance {
                let signal = Signal::from_object_signal(obj, c"stdout_emit");
                match config.wasi_stdout_buffer {
                    PipeBufferType::Unbuffered | PipeBufferType::BlockBuffer => {
                        builder.stdout_block_buffer(Box::new(WasiContext::emit_binary(signal)))
                    }
                    PipeBufferType::LineBuffer => {
                        builder.stdout_line_buffer(Box::new(WasiContext::emit_string(signal)))
                    }
                }?;
            }
            if config.wasi_stderr == PipeBindingType::Instance {
                let signal = Signal::from_object_signal(obj, c"stderr_emit");
                match config.wasi_stderr_buffer {
                    PipeBufferType::Unbuffered | PipeBufferType::BlockBuffer => {
                        builder.stderr_block_buffer(Box::new(WasiContext::emit_binary(signal)))
                    }
                    PipeBufferType::LineBuffer => {
                        builder.stderr_line_buffer(Box::new(WasiContext::emit_string(signal)))
                    }
                }?;
            }

            match &config.wasi_context {
                Some(ctx) => WasiContext::build_ctx(ctx, &mut builder, config),
                None => WasiContext::init_ctx_no_context(&mut builder, config),
            }?;
            let ctx = builder.build()?;
            wasi_stdin = ctx.stdin_provider().map(|v| v.dup());
            *wasi_ctx = Some(ctx);
            let mut r = <Linker<T>>::new(store.engine());
            add_to_linker(&mut r, |data| {
                data.as_mut()
                    .wasi_ctx
                    .as_mut()
                    .expect("WASI context required, but none supplied")
            })?;
            wasi_linker = Some(r);
        }

        #[cfg(feature = "object-registry-compat")]
        if config.extern_bind == ExternBindingType::Registry {
            store.data_mut().as_mut().object_registry = Some(ObjectRegistry::default());
        }
        #[cfg(feature = "object-registry-extern")]
        {
            store.data_mut().as_mut().use_extern = config.extern_bind == ExternBindingType::Native;
        }

        let instance = InstanceArgs {
            store: store.as_context_mut(),
            config,
            insts: HashMap::new(),
            host: host.map(HostModuleCache::new).transpose()?,
            #[cfg(feature = "object-registry-compat")]
            objregistry_funcs: ObjregistryFuncs::default(),
            #[cfg(feature = "object-registry-extern")]
            externref_funcs: ExternrefFuncs::default(),
            #[cfg(feature = "wasi")]
            wasi_linker,
        }
        .instantiate_wasm(module.bind().get_data()?)?;

        Ok(Self {
            instance: InstanceType::Core(instance),
            module,
            store: Mutex::new(store),
            #[cfg(feature = "wasi")]
            wasi_stdin,
        })
    }
}

impl<T> InstanceArgs<'_, T>
where
    T: Send + AsRef<StoreData> + AsMut<StoreData> + HasEpochTimeout,
{
    #[instrument(skip_all, fields(?module.module))]
    fn instantiate_wasm(&mut self, module: &ModuleData) -> AnyResult<InstanceWasm> {
        #[allow(irrefutable_let_patterns)]
        let ModuleType::Core(module_) = &module.module
        else {
            bail_with_site!("Cannot instantiate component")
        };

        let imports = module_
            .imports()
            .map(|i| {
                let _s = debug_span!("instantiate_wasm.import", import = ?i).entered();
                if let Some(v) = &mut self.host {
                    if let Some(v) = v.get_extern(&mut self.store, i.module(), i.name())? {
                        return Ok(v);
                    }
                }

                if let Some(o) = module.imports.get(i.module()) {
                    let _s = debug_span!("instantiate_wasm.import.recursive", ?o).entered();
                    let id = o.instance_id();
                    let mut v = match self.insts.entry(id) {
                        Entry::Vacant(v) => v.insert(None),
                        Entry::Occupied(v) => match v.into_mut() {
                            None => bail_with_site!("Recursive data structure"),
                            v => v,
                        },
                    };
                    let _s = trace_span!("instantiate_wasm.import.recursive.inner");
                    let t = loop {
                        let _s = _s.enter();
                        if let Some(v) = v {
                            break v;
                        }
                        let t = self.instantiate_wasm(o.bind().get_data()?)?;
                        v = self.insts.entry(id).or_insert(None);
                        *v = Some(t);
                    };
                    if let Some(v) = t.get_export(&mut self.store, i.name()) {
                        return Ok(v);
                    }
                }

                #[cfg(any(feature = "object-registry-compat", feature = "object-registry-extern"))]
                if let Some(v) = match (i.module(), &self.config) {
                    #[cfg(feature = "object-registry-compat")]
                    (
                        OBJREGISTRY_MODULE,
                        Config {
                            extern_bind: ExternBindingType::Registry,
                            ..
                        },
                    ) => self.objregistry_funcs.get_func(&mut self.store, i.name()),
                    #[cfg(feature = "object-registry-extern")]
                    (
                        EXTERNREF_MODULE,
                        Config {
                            extern_bind: ExternBindingType::Native,
                            ..
                        },
                    ) => self.externref_funcs.get_func(&mut self.store, i.name()),
                    _ => None,
                } {
                    return Ok(v.into());
                }

                #[cfg(feature = "wasi")]
                if let Some(v) = &self.wasi_linker {
                    if let Some(v) = v.get_by_import(&mut self.store, &i) {
                        return Ok(v);
                    }
                }

                bail_with_site!("Unknown import {:?}.{:?}", i.module(), i.name());
            })
            .collect::<AnyResult<Vec<_>>>()?;

        InstanceWasm::new(&mut self.store, module_, &imports)
    }
}

impl<T> InstanceData<T>
where
    T: AsRef<InnerLock> + AsMut<InnerLock>,
{
    #[instrument(skip_all)]
    pub fn acquire_store<F, R>(&self, f: F) -> R
    where
        for<'a> F: FnOnce(&Self, StoreContextMut<'a, T>) -> R,
    {
        let mut guard_ = self.store.lock();

        let _scope;
        // SAFETY: Context should be destroyed after function call
        unsafe {
            let p = &mut guard_.data_mut().as_mut().mutex_raw;
            debug!(old_handle = ?*p);
            let v = mem::replace(p, self.store.raw() as *const _);
            _scope = guard((p as *mut _, v), |(p, v)| {
                *p = v;
            });
        }

        f(self, guard_.as_context_mut())
    }
}

impl InnerLock {
    #[instrument(skip_all)]
    pub fn release_store<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard;
        if !self.mutex_raw.is_null() {
            // SAFETY: Pointer is valid and locked mutex
            unsafe {
                _guard = guard(&*self.mutex_raw, |v| {
                    v.lock();
                    debug!(handle = ?(v as *const RawMutex), "Locked store");
                });
                _guard.unlock();
                debug!(handle = ?(*_guard as *const RawMutex), "Unlocked store");
            }
        } else {
            warn!("Trying to release lock without locking first. This might be a bug.");
        }

        f()
    }
}

impl StoreData {
    #[inline]
    pub(crate) fn release_store<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.inner_lock.release_store(f)
    }

    #[cfg(feature = "object-registry-compat")]
    pub fn get_registry(&self) -> AnyResult<&ObjectRegistry> {
        match self.object_registry.as_ref() {
            Some(v) => Ok(v),
            None => bail_with_site!("Object registry not enabled!"),
        }
    }

    #[cfg(feature = "object-registry-compat")]
    pub fn get_registry_mut(&mut self) -> AnyResult<&mut ObjectRegistry> {
        match self.object_registry.as_mut() {
            Some(v) => Ok(v),
            None => bail_with_site!("Object registry not enabled!"),
        }
    }
}

impl WasmInstance {
    #[instrument(level = Level::ERROR)]
    fn emit_error_wrapper(&self, msg: String) {
        self.to_gd().emit_signal(
            &StringName::from(c"error_happened"),
            &[GString::from(msg).to_variant()],
        );
    }

    #[instrument(level = Level::TRACE)]
    pub fn get_data(&self) -> AnyResult<&InstanceData<StoreData>> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized instance")
        }
    }

    #[instrument(level = Level::DEBUG, skip(f))]
    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&InstanceData<StoreData>) -> AnyResult<R>,
    {
        match self.get_data().and_then(f) {
            Ok(v) => Some(v),
            Err(e) => {
                let s = format!("{e:?}");
                /*
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    &s,
                );
                */
                godot_error!("{s}");
                self.emit_error_wrapper(s);
                None
            }
        }
    }

    #[instrument(level = Level::DEBUG, skip_all, fields(?self, ?module))]
    pub fn initialize_(
        &self,
        module: Gd<WasmModule>,
        host: Option<Dictionary>,
        config: Option<Variant>,
    ) -> bool {
        let r = self.data.get_or_try_init(move || -> AnyResult<_> {
            let mut ret = InstanceData::instantiate(
                &self.to_gd(),
                Store::new(&site_context!(get_engine())?, StoreData::default()),
                &match config {
                    Some(v) => match Config::try_from_variant(&v) {
                        Ok(v) => v,
                        Err(e) => {
                            godot_error!("{:?}", e);
                            Config::default()
                        }
                    },
                    None => Config::default(),
                },
                module,
                host,
            )?;

            // SAFETY: Nobody else can access memory
            unsafe {
                *(ptr::addr_of!(self.memory) as *mut Option<MemoryType>) = match &ret.instance {
                    InstanceType::Core(inst) => {
                        match inst.get_export(ret.store.get_mut(), MEMORY_EXPORT) {
                            Some(Extern::Memory(mem)) => Some(MemoryType::Memory(mem)),
                            Some(Extern::SharedMemory(mem)) => Some(MemoryType::SharedMemory(mem)),
                            _ => None,
                        }
                    }
                    #[allow(unreachable_patterns)]
                    _ => None,
                };
            }
            Ok(ret)
        });
        if let Err(e) = r {
            let s = format!("{e:?}");
            godot_error!("{s}");
            self.emit_error_wrapper(s);
            false
        } else {
            true
        }
    }

    #[instrument(level = Level::TRACE, skip(f))]
    pub fn acquire_store<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(StoreContextMut<'_, StoreData>) -> AnyResult<R>,
    {
        self.unwrap_data(move |m| {
            m.acquire_store(move |_, s| {
                let _s = debug_span!("acquire_store.inner", ?self).entered();
                f(s)
            })
        })
    }

    #[instrument(level = Level::TRACE, skip(f))]
    fn get_memory<F, R>(&self, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(&'a mut [u8]) -> AnyResult<R>,
    {
        self.acquire_store(move |store| {
            let _s = debug_span!("get_memory.inner", ?self).entered();
            f(match &self.memory {
                Some(MemoryType::Memory(mem)) => mem.data_mut(store),
                // SAFETY: Externalize concurrent access to user
                #[allow(mutable_transmutes)]
                Some(MemoryType::SharedMemory(mem)) => unsafe {
                    mem::transmute::<&[_], &mut [u8]>(mem.data())
                },
                None => bail_with_site!("No memory exported"),
            })
        })
    }

    #[instrument(level = Level::DEBUG, skip(f))]
    fn read_memory<F, R>(&self, i: usize, n: usize, f: F) -> Option<R>
    where
        F: FnOnce(&[u8]) -> AnyResult<R>,
    {
        self.get_memory(|data| match data.get(i..i + n) {
            Some(s) => f(s),
            None => bail_with_site!("Index out of bound {}-{}", i, i + n),
        })
    }

    #[instrument(level = Level::DEBUG, skip(f))]
    fn write_memory<F, R>(&self, i: usize, n: usize, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(&'a mut [u8]) -> AnyResult<R>,
    {
        self.get_memory(|data| match data.get_mut(i..i + n) {
            Some(s) => f(s),
            None => bail_with_site!("Index out of bound {}-{}", i, i + n),
        })
    }
}

#[derive(Debug)]
struct WasmCallable {
    name: StringName,
    ty: FuncType,
    ptr: *const ffi::c_void,
    this: SendSyncWrapper<Gd<WasmInstance>>,
}

unsafe impl Send for WasmCallable {}
unsafe impl Sync for WasmCallable {}

impl PartialEq for WasmCallable {
    fn eq(&self, other: &Self) -> bool {
        (self.name == other.name)
            && (self.this == other.this)
            && FuncType::eq(&self.ty, &other.ty)
            && (self.ptr == other.ptr)
    }
}

impl Eq for WasmCallable {}

impl Hash for WasmCallable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.name, state);
        self.ty.hash(state);
        self.this.hash(state);
        self.ptr.hash(state);
    }
}

impl fmt::Display for WasmCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write_iter<I: fmt::Display>(
            it: impl IntoIterator<Item = I>,
            f: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            let mut start = true;
            for v in it {
                if !start {
                    f.write_str(", ")?;
                }
                start = false;
                v.fmt(f)?;
            }
            Ok(())
        }

        write!(f, "WasmCallable({:?}.{}<(", *self.this, self.name)?;

        write_iter(self.ty.params(), f)?;
        write!(f, "), (")?;
        write_iter(self.ty.results(), f)?;

        write!(f, ")>)")
    }
}

impl RustCallable for WasmCallable {
    #[instrument(skip(args), fields(args.len = args.len()))]
    fn invoke(&mut self, args: &[&Variant]) -> Result<Variant, ()> {
        let r = self
            .this
            .bind()
            .acquire_store(|#[allow(unused_mut)] mut store| {
                let _s = debug_span!("invoke.inner").entered();
                #[cfg(feature = "epoch-timeout")]
                reset_epoch(&mut store);

                // SAFETY: Function pointer is valid.
                let ret = unsafe {
                    let f = Func::from_raw(&mut store, self.ptr as *mut ffi::c_void).unwrap();
                    raw_call(store, &f, &self.ty, args.iter().copied())?
                };
                info!(ret.len = ret.len());
                Ok(ret)
            });
        match r {
            Some(v) => Ok(v.to_variant()),
            None => Err(()),
        }
    }
}

#[godot_api]
impl WasmInstance {
    /// Emitted if an error happened. Use it to handle errors.
    #[signal]
    fn error_happened(message: GString);
    /// Emitted whenever WASI stdout is written. Only usable with WASI.
    #[signal]
    fn stdout_emit(message: Variant);
    /// Emitted whenever WASI stderr is written. Only usable with WASI.
    #[signal]
    fn stderr_emit(message: Variant);
    /// Emitted whenever WASI stdin is tried to be read. Only usable with WASI.
    #[signal]
    fn stdin_request();

    /// Initialize and instantiates module.
    ///
    /// **⚠ MUST BE CALLED FOR THE FIRST TIME AND ONLY ONCE.**
    ///
    /// Returns itself if succeed, `null` otherwise.
    ///
    /// Arguments:
    /// - `module` : `WasmModule` to be instantiated.
    /// - `host` : Dictionary containing host module and functions to be bound.
    ///   It's value is a struct of the following:
    ///   - `params` : Array of parameter types.
    ///   - `results` : Array of result types.
    ///   - `callable` : `Callable` to be bound. Prefer this over object-method.
    ///   - `object` : Object to be bound.
    ///   - `method` : Method to be bound.
    /// - `config` : Configuration option.
    ///
    /// Usage:
    /// ```
    /// var module := WasmModule.new().initialize("...", {})
    /// var instance := WasmInstance.new().initialize(module, {}, {})
    ///
    /// if instance == null:
    ///   # Cannot instantiate module
    ///   pass
    /// ```
    #[func]
    #[instrument(skip(host, config), ret)]
    fn initialize(
        &self,
        module: Gd<WasmModule>,
        host: Variant,
        config: Variant,
    ) -> Option<Gd<WasmInstance>> {
        let Ok(host) = variant_to_option::<Dictionary>(host) else {
            error!("Host is not a dictionary!");
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

    /// Gets the module used to instantiate this object.
    #[func]
    #[instrument(ret)]
    fn get_module(&self) -> Option<Gd<WasmModule>> {
        self.unwrap_data(|m| Ok(m.module.clone()))
    }

    /// Calls into WASM.
    ///
    /// Arguments:
    /// - `name` : Name of the exported function.
    /// - `args` : Array of parameters.
    ///
    /// Returns an array of results, or `null` if failed.
    #[func]
    #[instrument(skip(args), fields(args.len = args.len()))]
    fn call_wasm(&self, name: StringName, args: VariantArray) -> Variant {
        option_to_variant(self.unwrap_data(move |m| {
            m.acquire_store(move |m, mut store| {
                let _s = debug_span!("call_wasm.inner").entered();

                let name = name.to_string();
                let f = match site_context!(m.instance.get_core())?.get_export(&mut store, &name) {
                    Some(Extern::Func(f)) => f,
                    Some(_) => bail_with_site!("Export {name} is not a function"),
                    None => bail_with_site!("Export {name} does not exists"),
                };
                let ty = f.ty(&store);

                #[cfg(feature = "epoch-timeout")]
                reset_epoch(&mut store);

                let ret = unsafe { raw_call(store, &f, &ty, args.iter_shared())? };
                info!(ret.len = ret.len());
                Ok(ret)
            })
        }))
    }

    /// Binds WASM function into a `Callable`.
    ///
    /// Arguments:
    /// - `name` : Name of the exported function.
    ///
    /// Returns a `Callable` that can be used to call into WASM.
    #[func]
    #[instrument(ret(Display))]
    fn bind_wasm(&self, name: StringName) -> Callable {
        self.unwrap_data(move |m| {
            m.acquire_store(move |m, mut store| {
                let _s = debug_span!("bind_wasm.inner").entered();
                let f = {
                    let name = name.to_string();
                    match site_context!(m.instance.get_core())?.get_export(&mut store, &name) {
                        Some(Extern::Func(f)) => f,
                        Some(_) => bail_with_site!("Export {name} is not a function"),
                        None => bail_with_site!("Export {name} does not exists"),
                    }
                };

                Ok(Callable::from_custom(WasmCallable {
                    name,
                    ty: f.ty(&store),
                    // SAFETY: Pointer is valid for the entire lifetime of callable.
                    ptr: unsafe { f.to_raw(store) },
                    this: SendSyncWrapper::new(self.to_gd()),
                }))
            })
        })
        .unwrap_or_else(Callable::invalid)
    }

    /// Emits trap when returning from host. Should only be used from imported host functions.
    ///
    /// Returns previous error message, if any.
    #[func]
    #[instrument(ret(level = Level::DEBUG))]
    fn signal_error(&self, msg: GString) -> Variant {
        option_to_variant(self.acquire_store(move |mut store| {
            Ok(store
                .data_mut()
                .error_signal
                .replace(msg.to_string())
                .unwrap_or_default())
        }))
    }

    /// Cancels effect of `signal_error`.
    ///
    /// Returns previous error message, if any.
    #[func]
    #[instrument(ret(level = Level::DEBUG))]
    fn signal_error_cancel(&self) -> Variant {
        option_to_variant(self.acquire_store(|mut store| {
            Ok(store.data_mut().error_signal.take().unwrap_or_default())
        }))
    }

    /// Resets epoch timeout. Should only be used from imported host functions.
    #[func]
    #[instrument]
    fn reset_epoch(&self) {
        cfg_if! {
            if #[cfg(feature = "epoch-timeout")] {
                self.acquire_store(|store| {
                        reset_epoch(store);
                        Ok(())
                });
            } else {
                godot_error!("Feature epoch-timeout not enabled!");
            }
        }
    }

    /// Registers value and returns it's index. Only usable with object registry.
    #[func]
    #[instrument(skip(_obj))]
    fn register_object(&self, _obj: Variant) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(self.acquire_store(move |mut store| Ok(store.data_mut().get_registry_mut()?.register(_obj) as u64)))
            } else {
                godot_error!("Feature object-registry-compat not enabled!");
                Variant::nil()
            }
        }
    }

    /// Gets registered value in index. Only usable with object registry.
    #[func]
    #[instrument]
    fn registry_get(&self, _ix: i64) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(
                    self.acquire_store(move |store| {
                            Ok(store.data().get_registry()?.get(usize::try_from(_ix)?))
                    })
                    .flatten(),
                )
            } else {
                godot_error!("Feature object-registry-compat not enabled!");
                Variant::nil()
            }
        }
    }

    /// Sets registered value in index. Only usable with object registry.
    #[func]
    #[instrument(skip(_obj))]
    fn registry_set(&self, _ix: i64, _obj: Variant) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(
                    self.acquire_store(move |mut store| {
                            let _ix = usize::try_from(_ix)?;
                            let reg = store.data_mut().get_registry_mut()?;
                            if _obj.is_nil() {
                                Ok(reg.unregister(_ix))
                            } else {
                                Ok(reg.replace(_ix, _obj))
                            }
                    })
                    .flatten(),
                )
            } else {
                godot_error!("Feature object-registry-compat not enabled!");
                Variant::nil()
            }
        }
    }

    /// Unregister and returns value in index. Only usable with object registry.
    #[func]
    #[instrument]
    fn unregister_object(&self, _ix: i64) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(
                    self.acquire_store(|mut store| {
                            Ok(store
                                .data_mut()
                                .get_registry_mut()?
                                .unregister(usize::try_from(_ix)?))
                    })
                    .flatten(),
                )
            } else {
                godot_error!("Feature object-registry-compat not enabled!");
                Variant::nil()
            }
        }
    }

    /// Returns `true` if exported memory exists.
    #[func]
    #[instrument(ret)]
    fn has_memory(&self) -> bool {
        self.unwrap_data(|_| Ok(self.memory.is_some()))
            .unwrap_or_default()
    }

    /// Sets custom exported memory name.
    ///
    /// Returns `true` if memory exists.
    ///
    /// Default exported memory name is `"memory"`.
    #[func]
    #[instrument(ret)]
    fn memory_set_name(&self, name: GString) -> bool {
        self.unwrap_data(move |m| {
            m.acquire_store(move |m, store| {
                // SAFETY: Nobody else can access memory
                unsafe {
                    *(ptr::addr_of!(self.memory) as *mut Option<MemoryType>) = match &m.instance {
                        InstanceType::Core(inst) => match inst.get_export(store, &name.to_string())
                        {
                            Some(Extern::Memory(mem)) => Some(MemoryType::Memory(mem)),
                            Some(Extern::SharedMemory(mem)) => Some(MemoryType::SharedMemory(mem)),
                            _ => None,
                        },
                        #[allow(unreachable_patterns)]
                        _ => None,
                    };
                }
                Ok(self.memory.is_some())
            })
        })
        .unwrap_or_default()
    }

    /// Inserts a line to stdin. Only usable with WASI.
    #[func]
    #[instrument]
    fn stdin_add_line(&self, _line: GString) {
        cfg_if! {
            if #[cfg(feature = "wasi")] {
                self.unwrap_data(move |m| {
                    if let Some(stdin) = &m.wasi_stdin {
                        stdin.write(_line.to_string().as_bytes());
                    }
                    Ok(())
                });
            } else {
                godot_error!("Feature wasi not enabled!");
            }
        }
    }

    /// Closes stdin. Only usable with WASI.
    #[func]
    #[instrument]
    fn stdin_close(&self) {
        cfg_if! {
            if #[cfg(feature = "wasi")] {
                self.unwrap_data(|m| {
                    if let Some(stdin) = &m.wasi_stdin {
                        stdin.close();
                    }
                    Ok(())
                });
            } else {
                godot_error!("Feature wasi not enabled!");
            }
        }
    }

    /// Returns memory size.
    #[func]
    #[instrument(ret)]
    fn memory_size(&self) -> i64 {
        self.get_memory(|data| Ok(data.len() as i64))
            .unwrap_or_default()
    }

    /// Reads a chunk of memory.
    #[func]
    #[instrument]
    fn memory_read(&self, i: i64, n: i64) -> PackedByteArray {
        self.read_memory(i as _, n as _, |s| Ok(PackedByteArray::from(s)))
            .unwrap_or_default()
    }

    /// Writes a chunk of memory.
    #[func]
    #[instrument(skip(a), fields(a.len = a.len()), ret)]
    fn memory_write(&self, i: i64, a: PackedByteArray) -> bool {
        self.write_memory(i as _, a.len(), move |s| {
            s.copy_from_slice(a.as_slice());
            Ok(())
        })
        .is_some()
    }

    /// Reads an unsigned 8-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn get_8(&self, i: i64) -> i64 {
        self.read_memory(i as _, 1, |s| Ok(s[0]))
            .unwrap_or_default()
            .into()
    }

    /// Writes an unsigned 8-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn put_8(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 1, |s| {
            s[0] = (v & 255) as _;
            Ok(())
        })
        .is_some()
    }

    /// Reads an unsigned 16-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn get_16(&self, i: i64) -> i64 {
        self.read_memory(i as _, 2, |s| Ok(u16::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    /// Writes an unsigned 16-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn put_16(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 2, |s| {
            s.copy_from_slice(&((v & 0xffff) as u16).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads an unsigned 32-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn get_32(&self, i: i64) -> i64 {
        self.read_memory(i as _, 4, |s| Ok(u32::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    /// Writes an unsigned 32-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn put_32(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 4, |s| {
            s.copy_from_slice(&((v & 0xffffffff) as u32).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads a signed 64-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn get_64(&self, i: i64) -> i64 {
        self.read_memory(i as _, 8, |s| Ok(i64::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
    }

    /// Writes a signed 64-bit integer.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn put_64(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads a 32-bit floating-point number.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn get_float(&self, i: i64) -> f64 {
        self.read_memory(i as _, 4, |s| Ok(f32::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    /// Writes a 32-bit floating-point number.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn put_float(&self, i: i64, v: f64) -> bool {
        self.write_memory(i as _, 4, |s| {
            s.copy_from_slice(&(v as f32).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads a 64-bit floating-point number.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn get_double(&self, i: i64) -> f64 {
        self.read_memory(i as _, 8, |s| Ok(f64::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
    }

    /// Writes a 64-bit floating-point number.
    #[func]
    #[instrument(level = Level::DEBUG, ret)]
    fn put_double(&self, i: i64, v: f64) -> bool {
        self.write_memory(i as _, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Writes a `PackedArray`. Does not support `PackedStringArray`.
    #[func]
    #[instrument(level = Level::DEBUG, skip(v), fields(v.type = ?v.get_type()), ret)]
    fn put_array(&self, i: i64, v: Variant) -> bool {
        #[instrument(level = Level::DEBUG, skip_all, fields(N, d.len = d.len(), i, s.len = s.len()))]
        fn f<const N: usize, T: Sync>(
            d: &mut [u8],
            i: usize,
            s: &[T],
            f: impl Fn(&T, &mut [u8; N]) + Send + Sync,
        ) -> AnyResult<()> {
            let e = i + s.len() * N;
            let Some(d) = d.get_mut(i..e) else {
                bail_with_site!("Index out of range ({i}..{e})");
            };

            s.par_iter()
                .zip(d.par_chunks_exact_mut(N))
                .for_each(|(s, d)| f(s, d.try_into().unwrap()));

            Ok(())
        }

        self.get_memory(move |data| {
            let i = i as usize;
            variant_dispatch!(v {
                PACKED_BYTE_ARRAY => {
                    let s = v.as_slice();
                    let e = i + s.len();
                    let Some(d) = data.get_mut(i..e) else {
                        bail_with_site!("Index out of range ({i}..{e})");
                    };

                    d.copy_from_slice(s);
                    Ok(())
                },
                PACKED_INT32_ARRAY => f(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes()),
                PACKED_INT64_ARRAY => f(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes()),
                PACKED_FLOAT32_ARRAY => f(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes()),
                PACKED_FLOAT64_ARRAY => f(data, i, v.as_slice(), |s, d| *d = s.to_le_bytes()),
                PACKED_VECTOR2_ARRAY => f(data, i, v.as_slice(), <_ as StructPacking<f32>>::write_array),
                PACKED_VECTOR3_ARRAY => f(data, i, v.as_slice(), <_ as StructPacking<f32>>::write_array),
                PACKED_COLOR_ARRAY => f(data, i, v.as_slice(), <_ as StructPacking<f32>>::write_array),
                _ => bail_with_site!("Unknown value type {:?}", v.get_type()),
            })
        })
        .is_some()
    }

    /// Reads a `PackedArray`. Does not support `PackedStringArray`.
    #[func]
    #[instrument(level = Level::DEBUG)]
    fn get_array(&self, i: i64, n: i64, t: VariantType) -> Variant {
        #[instrument(level = Level::DEBUG, skip_all, fields(N, s.len = s.len(), i, n))]
        fn f<const N: usize, R>(
            s: &[u8],
            i: usize,
            n: usize,
            f: impl Fn(&[u8; N]) -> R::Elem + Send + Sync,
        ) -> AnyResult<Variant>
        where
            R: PackedArrayLike + ToGodot,
            R::Elem: Send,
        {
            let e = i + n * N;
            let Some(s) = s.get(i..e) else {
                bail_with_site!("Index out of range ({i}..{e})");
            };

            let mut r = R::default();
            r.resize(n);
            s.par_chunks_exact(N)
                .zip(r.as_mut_slice())
                .for_each(|(s, d)| *d = f(s.try_into().unwrap()));

            Ok(r.to_variant())
        }

        option_to_variant(self.get_memory(move |data| {
            let data = &*data;
            let (i, n) = (i as usize, n as usize);
            match t {
                VariantType::PACKED_BYTE_ARRAY => {
                    let e = i + n;
                    let Some(s) = data.get(i..e) else {
                        bail_with_site!("Index out of range ({i}..{e})");
                    };

                    Ok(PackedByteArray::from(s).to_variant())
                }
                VariantType::PACKED_INT32_ARRAY => {
                    f::<4, PackedInt32Array>(data, i, n, |s| i32::from_le_bytes(*s))
                }
                VariantType::PACKED_INT64_ARRAY => {
                    f::<8, PackedInt64Array>(data, i, n, |s| i64::from_le_bytes(*s))
                }
                VariantType::PACKED_FLOAT32_ARRAY => {
                    f::<4, PackedFloat32Array>(data, i, n, |s| f32::from_le_bytes(*s))
                }
                VariantType::PACKED_FLOAT64_ARRAY => {
                    f::<8, PackedFloat64Array>(data, i, n, |s| f64::from_le_bytes(*s))
                }
                VariantType::PACKED_VECTOR2_ARRAY => {
                    f::<8, PackedVector2Array>(data, i, n, <_ as StructPacking<f32>>::read_array)
                }
                VariantType::PACKED_VECTOR3_ARRAY => {
                    f::<12, PackedVector3Array>(data, i, n, <_ as StructPacking<f32>>::read_array)
                }
                VariantType::PACKED_COLOR_ARRAY => {
                    f::<16, PackedColorArray>(data, i, n, <_ as StructPacking<f32>>::read_array)
                }
                _ => bail_with_site!("Unsupported type ID {t:?}"),
            }
        }))
    }

    /// Reads a structured data.
    #[func]
    #[instrument(level = Level::DEBUG)]
    fn read_struct(&self, format: GString, p: u64) -> Variant {
        option_to_variant(self.get_memory(move |data| {
            let mut f = Cursor::new(data);
            f.set_position(p);
            let ret = read_struct(f, format.chars())?;
            info!(ret.len = ret.len());
            Ok(ret)
        }))
    }

    /// Writes a structured data.
    #[func]
    #[instrument(level = Level::DEBUG, skip(arr), fields(arr.len = arr.len()), ret)]
    fn write_struct(&self, format: GString, p: u64, arr: VariantArray) -> u64 {
        self.get_memory(move |data| {
            let mut f = Cursor::new(data);
            f.set_position(p);
            write_struct(f, format.chars(), arr)
        })
        .unwrap_or_default() as _
    }
}
