#[cfg(feature = "wasi")]
use std::any::Any;
#[cfg(feature = "wasm-threads")]
use std::cell::UnsafeCell;
use std::collections::hash_map::{Entry, HashMap};
use std::hash::{Hash, Hasher};
#[cfg(feature = "wasi")]
use std::sync::Arc;
use std::{fmt, mem, ptr};

use anyhow::{bail, Result as AnyResult};
use cfg_if::cfg_if;
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, RawMutex};
use rayon::prelude::*;
use scopeguard::guard;
#[cfg(feature = "component-model")]
use wasmtime::component::Instance as InstanceComp;
#[cfg(feature = "wasi")]
use wasmtime::component::ResourceTable;
#[cfg(feature = "wasi")]
use wasmtime::Linker;
#[cfg(feature = "memory-limiter")]
use wasmtime::ResourceLimiter;
#[cfg(feature = "wasm-threads")]
use wasmtime::SharedMemory;
use wasmtime::{
    AsContextMut, Extern, Func, FuncType, Instance as InstanceWasm, Memory, Store, StoreContextMut,
};
#[cfg(feature = "wasi")]
use wasmtime_wasi::preview1::{add_to_linker_sync, WasiP1Ctx};
#[cfg(feature = "wasi")]
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

#[cfg(feature = "wasi")]
use crate::godot_util::gstring_from_maybe_utf8;
use crate::godot_util::{
    option_to_variant, variant_to_option, PackedArrayLike, PhantomProperty, SendSyncWrapper,
    VariantDispatch,
};
use crate::rw_struct::{read_struct, write_struct};
#[cfg(feature = "wasi")]
use crate::wasi_ctx::stdio::{
    BlockWritePipe, ByteBufferReadPipe, InnerStdin, LineWritePipe, OuterStdin, StreamWrapper,
    UnbufferedWritePipe,
};
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
#[cfg(feature = "object-registry-extern")]
use crate::wasm_util::EXTERNREF_MODULE;
#[cfg(feature = "object-registry-compat")]
use crate::wasm_util::OBJREGISTRY_MODULE;
use crate::wasm_util::{config_store_common, raw_call, HostModuleCache, MEMORY_EXPORT};
use crate::{bail_with_site, site_context};

enum MemoryType {
    Memory(Memory),
    #[cfg(feature = "wasm-threads")]
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

pub struct InstanceData<T> {
    pub store: Mutex<Store<T>>,
    pub instance: InstanceType,
    pub module: Gd<WasmModule>,

    #[cfg(feature = "wasi")]
    pub wasi_stdin: Option<Arc<InnerStdin<dyn Any + Send + Sync>>>,
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

    #[cfg(feature = "wasi")]
    pub wasi_ctx: MaybeWasi,
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

#[allow(clippy::derivable_impls)]
impl Default for StoreData {
    fn default() -> Self {
        Self {
            inner_lock: InnerLock::default(),
            error_signal: None,

            #[cfg(feature = "epoch-timeout")]
            epoch_timeout: 0,
            #[cfg(feature = "epoch-timeout")]
            epoch_autoreset: false,

            #[cfg(feature = "memory-limiter")]
            memory_limits: MemoryLimit::default(),

            #[cfg(feature = "object-registry-compat")]
            object_registry: None,

            #[cfg(feature = "wasi")]
            wasi_ctx: MaybeWasi::NoCtx,
        }
    }
}

#[allow(dead_code)]
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

    fn table_growing(&mut self, current: u32, desired: u32, max: Option<u32>) -> AnyResult<bool> {
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
    T: Send + AsRef<StoreData> + AsMut<StoreData>,
{
    pub fn instantiate(
        _inst_id: InstanceId,
        mut store: Store<T>,
        config: &Config,
        module: Gd<WasmModule>,
        host: Option<Dictionary>,
    ) -> AnyResult<Self> {
        config_store_common(&mut store, config)?;

        #[cfg(feature = "wasi")]
        let mut wasi_stdin = None;

        #[cfg(feature = "wasi")]
        let wasi_linker = if config.with_wasi {
            let mut builder = WasiCtxBuilder::new();

            let StoreData { wasi_ctx, .. } = store.data_mut().as_mut();

            if config.wasi_stdin == PipeBindingType::Instance {
                if let Some(data) = config.wasi_stdin_data.clone() {
                    builder.stdin(StreamWrapper::from(ByteBufferReadPipe::new(data)));
                } else {
                    let (outer, inner) = OuterStdin::new(move || {
                        <Gd<RefCounted>>::from_instance_id(_inst_id)
                            .emit_signal(StringName::from(c"stdin_request"), &[]);
                    });
                    builder.stdin(outer);
                    wasi_stdin = Some(inner as _);
                }
            }
            if config.wasi_stdout == PipeBindingType::Instance {
                match config.wasi_stdout_buffer {
                    PipeBufferType::Unbuffered => {
                        builder.stdout(UnbufferedWritePipe::new(move |buf| {
                            <Gd<RefCounted>>::from_instance_id(_inst_id).emit_signal(
                                StringName::from(c"stdout_emit"),
                                &[PackedByteArray::from(buf).to_variant()],
                            );
                        }))
                    }
                    PipeBufferType::LineBuffer => {
                        builder.stdout(StreamWrapper::from(LineWritePipe::new(move |buf| {
                            <Gd<RefCounted>>::from_instance_id(_inst_id).emit_signal(
                                StringName::from(c"stdout_emit"),
                                &[gstring_from_maybe_utf8(buf).to_variant()],
                            );
                        })))
                    }
                    PipeBufferType::BlockBuffer => {
                        builder.stdout(StreamWrapper::from(BlockWritePipe::new(move |buf| {
                            <Gd<RefCounted>>::from_instance_id(_inst_id).emit_signal(
                                StringName::from(c"stdout_emit"),
                                &[gstring_from_maybe_utf8(buf).to_variant()],
                            );
                        })))
                    }
                };
            }
            if config.wasi_stderr == PipeBindingType::Instance {
                match config.wasi_stderr_buffer {
                    PipeBufferType::Unbuffered => {
                        builder.stderr(UnbufferedWritePipe::new(move |buf| {
                            <Gd<RefCounted>>::from_instance_id(_inst_id).emit_signal(
                                StringName::from(c"stderr_emit"),
                                &[PackedByteArray::from(buf).to_variant()],
                            );
                        }))
                    }
                    PipeBufferType::LineBuffer => {
                        builder.stderr(StreamWrapper::from(LineWritePipe::new(move |buf| {
                            <Gd<RefCounted>>::from_instance_id(_inst_id).emit_signal(
                                StringName::from(c"stderr_emit"),
                                &[gstring_from_maybe_utf8(buf).to_variant()],
                            );
                        })))
                    }
                    PipeBufferType::BlockBuffer => {
                        builder.stderr(StreamWrapper::from(BlockWritePipe::new(move |buf| {
                            <Gd<RefCounted>>::from_instance_id(_inst_id).emit_signal(
                                StringName::from(c"stderr_emit"),
                                &[gstring_from_maybe_utf8(buf).to_variant()],
                            );
                        })))
                    }
                };
            }

            match &config.wasi_context {
                Some(ctx) => WasiContext::build_ctx(ctx.clone(), &mut builder, config),
                None => WasiContext::init_ctx_no_context(&mut builder, config),
            }?;
            *wasi_ctx = MaybeWasi::Preview1(builder.build_p1());
            let mut r = <Linker<T>>::new(store.engine());
            add_to_linker_sync(&mut r, |data| match &mut data.as_mut().wasi_ctx {
                MaybeWasi::Preview1(v) => v,
                _ => panic!("WASI Preview 1 context required, but none supplied"),
            })?;
            Some(r)
        } else {
            None
        };

        #[cfg(feature = "object-registry-compat")]
        if config.extern_bind == ExternBindingType::Registry {
            store.data_mut().as_mut().object_registry = Some(ObjectRegistry::default());
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

impl<'a, T> InstanceArgs<'a, T>
where
    T: Send + AsRef<StoreData> + AsMut<StoreData>,
{
    fn instantiate_wasm(&mut self, module: &ModuleData) -> AnyResult<InstanceWasm> {
        #[allow(irrefutable_let_patterns)]
        let ModuleType::Core(module_) = &module.module
        else {
            bail_with_site!("Cannot instantiate component")
        };

        let it = module_.imports().map(|i| {
            let mut v = match &mut self.host {
                Some(v) => v.get_extern(&mut self.store, i.module(), i.name())?,
                None => None,
            };

            #[cfg(any(feature = "object-registry-compat", feature = "object-registry-extern"))]
            if v.is_none() {
                v = match (i.module(), &self.config) {
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
                }
                .map(|v| v.into());
            }

            #[cfg(feature = "wasi")]
            if v.is_none() {
                if let Some(l) = &self.wasi_linker {
                    v = l.get_by_import(&mut self.store, &i);
                }
            }

            if v.is_none() {
                if let Some(o) = module.imports.get(i.module()) {
                    let id = o.instance_id();
                    v = loop {
                        match self.insts.entry(id) {
                            Entry::Occupied(v) => match v.get() {
                                Some(v) => break v.get_export(&mut self.store, i.name()),
                                None => bail_with_site!("Recursive data structure"),
                            },
                            Entry::Vacant(v) => v.insert(None),
                        };
                        let t = self.instantiate_wasm(o.bind().get_data()?)?;
                        self.insts.insert(id, Some(t));
                    };
                }
            }

            match v {
                Some(v) => Ok(v),
                None => bail_with_site!("Unknown import {:?}.{:?}", i.module(), i.name()),
            }
        });
        let imports = it.collect::<AnyResult<Vec<_>>>()?;

        InstanceWasm::new(&mut self.store, module_, &imports)
    }
}

impl<T> InstanceData<T>
where
    T: AsRef<InnerLock> + AsMut<InnerLock>,
{
    pub fn acquire_store<F, R>(&self, f: F) -> R
    where
        for<'a> F: FnOnce(&Self, StoreContextMut<'a, T>) -> R,
    {
        let mut guard_ = self.store.lock();

        let _scope;
        // SAFETY: Context should be destroyed after function call
        unsafe {
            let p = &mut guard_.data_mut().as_mut().mutex_raw;
            let v = mem::replace(p, self.store.raw() as *const _);
            _scope = guard(p as *mut _, move |p| {
                *p = v;
            });
        }

        f(self, guard_.as_context_mut())
    }
}

impl InnerLock {
    pub fn release_store<F, R>(&mut self, f: F) -> R
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
    fn emit_error_wrapper(&self, msg: String) {
        let args = [GString::from(msg).to_variant()];

        self.base()
            .clone()
            .emit_signal(StringName::from(c"error_happened"), &args);
    }

    pub fn get_data(&self) -> AnyResult<&InstanceData<StoreData>> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized instance")
        }
    }

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

    pub fn initialize_(
        &self,
        module: Gd<WasmModule>,
        host: Option<Dictionary>,
        config: Option<Variant>,
    ) -> bool {
        let r = self.data.get_or_try_init(move || -> AnyResult<_> {
            let mut ret = InstanceData::instantiate(
                self.base().instance_id(),
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
                            #[cfg(feature = "wasm-threads")]
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

    fn get_memory<F, R>(&self, f: F) -> Option<R>
    where
        for<'a> F: FnOnce(&'a mut [u8]) -> AnyResult<R>,
    {
        self.unwrap_data(|m| {
            m.acquire_store(|_, store| match &self.memory {
                Some(MemoryType::Memory(mem)) => f(mem.data_mut(store)),
                #[cfg(feature = "wasm-threads")]
                Some(MemoryType::SharedMemory(mem)) => {
                    // SAFETY: Externalize concurrent access to user
                    #[allow(mutable_transmutes)]
                    let s = unsafe { mem::transmute::<&[UnsafeCell<u8>], &mut [u8]>(mem.data()) };
                    f(s)
                }
                None => bail_with_site!("No memory exported"),
            })
        })
    }

    fn read_memory<F, R>(&self, i: usize, n: usize, f: F) -> Option<R>
    where
        F: FnOnce(&[u8]) -> AnyResult<R>,
    {
        self.get_memory(|data| match data.get(i..i + n) {
            Some(s) => f(s),
            None => bail_with_site!("Index out of bound {}-{}", i, i + n),
        })
    }

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

struct WasmCallable {
    name: StringName,
    ty: FuncType,
    f: Func,
    this: SendSyncWrapper<Gd<WasmInstance>>,
}

impl PartialEq for WasmCallable {
    fn eq(&self, other: &Self) -> bool {
        (self.name == other.name)
            && (*self.this == *other.this)
            && FuncType::eq(&self.ty, &other.ty)
    }
}

impl Eq for WasmCallable {}

impl Hash for WasmCallable {
    fn hash<H: Hasher>(&self, state: &mut H) {
        <StringName as Hash>::hash(&self.name, state);
        self.ty.hash(state);
        self.this.hash(state);
    }
}

impl fmt::Debug for WasmCallable {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { this, name, ty, f } = self;

        fmt.debug_struct("WasmCallable")
            .field("object", &**this)
            .field("name", name)
            .field("type", ty)
            .field("func", f)
            .finish()
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
    fn invoke(&mut self, args: &[&Variant]) -> Result<Variant, ()> {
        let Self { ty, f, this, .. } = self;

        let r = this.bind().unwrap_data(|m| {
            m.acquire_store(|_, #[allow(unused_mut)] mut store| {
                #[cfg(feature = "epoch-timeout")]
                if let v @ 1.. = store.data().epoch_timeout {
                    store.set_epoch_deadline(v);
                }

                unsafe { raw_call(store, f, ty, args.iter().copied()) }
            })
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

    /// Gets the module used to instantiate this object.
    #[func]
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
    fn call_wasm(&self, name: StringName, args: VariantArray) -> Variant {
        option_to_variant(self.unwrap_data(move |m| {
            m.acquire_store(move |m, mut store| {
                let name = name.to_string();
                let f = match site_context!(m.instance.get_core())?.get_export(&mut store, &name) {
                    Some(Extern::Func(f)) => f,
                    Some(_) => bail_with_site!("Export {name} is not a function"),
                    None => bail_with_site!("Export {name} does not exists"),
                };
                let ty = f.ty(&store);

                #[cfg(feature = "epoch-timeout")]
                if let v @ 1.. = store.data().epoch_timeout {
                    store.set_epoch_deadline(v);
                }

                unsafe { raw_call(store, &f, &ty, args.iter_shared()) }
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
    fn bind_wasm_callable(&self, name: StringName) -> Callable {
        self.unwrap_data(|m| {
            m.acquire_store(|m, mut store| {
                let n = name.to_string();
                let f = match site_context!(m.instance.get_core())?.get_export(&mut store, &n) {
                    Some(Extern::Func(f)) => f,
                    Some(_) => bail_with_site!("Export {n} is not a function"),
                    None => bail_with_site!("Export {n} does not exists"),
                };
                let ty = f.ty(&store);

                let this = SendSyncWrapper::new(self.to_gd());
                Ok(Callable::from_custom(WasmCallable { name, ty, f, this }))
            })
        })
        .unwrap_or_else(Callable::invalid)
    }

    /// Emits trap when returning from host. Should only be used from imported host functions.
    ///
    /// Returns previous error message, if any.
    #[func]
    fn signal_error(&self, msg: GString) -> Variant {
        option_to_variant(self.unwrap_data(|m| {
            m.acquire_store(|_, mut store| {
                Ok(store
                    .data_mut()
                    .error_signal
                    .replace(msg.to_string())
                    .unwrap_or_default())
            })
        }))
    }

    /// Cancels effect of `signal_error`.
    ///
    /// Returns previous error message, if any.
    #[func]
    fn signal_error_cancel(&self) -> Variant {
        option_to_variant(self.unwrap_data(|m| {
            m.acquire_store(|_, mut store| {
                Ok(store.data_mut().error_signal.take().unwrap_or_default())
            })
        }))
    }

    /// Resets epoch timeout. Should only be used from imported host functions.
    #[func]
    fn reset_epoch(&self) {
        cfg_if! {
            if #[cfg(feature = "epoch-timeout")] {
                self.unwrap_data(|m| {
                    m.acquire_store(|_, mut store| {
                        if let v @ 1.. = store.data().epoch_timeout {
                            store.set_epoch_deadline(v);
                        }
                        Ok(())
                    })
                });
            } else {
                godot_error!("Feature epoch-timeout not enabled!");
            }
        }
    }

    /// Registers value and returns it's index. Only usable with object registry.
    #[func]
    fn register_object(&self, _obj: Variant) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(self.unwrap_data(|m| {
                    if _obj.is_nil() {
                        bail_with_site!("Value is null!");
                    }
                    m.acquire_store(|_, mut store| Ok(store.data_mut().get_registry_mut()?.register(_obj) as u64))
                }))
            } else {
                godot_error!("Feature object-registry-compat not enabled!");
                Variant::nil()
            }
        }
    }

    /// Gets registered value in index. Only usable with object registry.
    #[func]
    fn registry_get(&self, _ix: i64) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(
                    self.unwrap_data(|m| {
                        m.acquire_store(|_, store| {
                            Ok(store.data().get_registry()?.get(usize::try_from(_ix)?))
                        })
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
    fn registry_set(&self, _ix: i64, _obj: Variant) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(
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
                )
            } else {
                godot_error!("Feature object-registry-compat not enabled!");
                Variant::nil()
            }
        }
    }

    /// Unregister and returns value in index. Only usable with object registry.
    #[func]
    fn unregister_object(&self, _ix: i64) -> Variant {
        cfg_if! {
            if #[cfg(feature = "object-registry-compat")] {
                option_to_variant(
                    self.unwrap_data(|m| {
                        m.acquire_store(|_, mut store| {
                            Ok(store
                                .data_mut()
                                .get_registry_mut()?
                                .unregister(usize::try_from(_ix)?))
                        })
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
    fn has_memory(&self) -> bool {
        self.unwrap_data(|m| m.acquire_store(|_, _| Ok(self.memory.is_some())))
            .unwrap_or_default()
    }

    /// Sets custom exported memory name.
    ///
    /// Returns `true` if memory exists.
    ///
    /// Default exported memory name is `"memory"`.
    #[func]
    fn memory_set_name(&self, name: GString) -> bool {
        self.unwrap_data(|m| {
            m.acquire_store(|m, store| {
                // SAFETY: Nobody else can access memory
                unsafe {
                    *(ptr::addr_of!(self.memory) as *mut Option<MemoryType>) = match &m.instance {
                        InstanceType::Core(inst) => match inst.get_export(store, &name.to_string())
                        {
                            Some(Extern::Memory(mem)) => Some(MemoryType::Memory(mem)),
                            #[cfg(feature = "wasm-threads")]
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
    fn stdin_add_line(&self, _line: GString) {
        cfg_if! {
            if #[cfg(feature = "wasi")] {
                self.unwrap_data(|m| {
                    if let Some(stdin) = &m.wasi_stdin {
                        stdin.add_line(_line)?;
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
    fn stdin_close(&self) {
        cfg_if! {
            if #[cfg(feature = "wasi")] {
                self.unwrap_data(|m| {
                    if let Some(stdin) = &m.wasi_stdin {
                        stdin.close_pipe();
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
    fn memory_size(&self) -> i64 {
        self.get_memory(|data| Ok(data.len() as i64))
            .unwrap_or_default()
    }

    /// Reads a chunk of memory.
    #[func]
    fn memory_read(&self, i: i64, n: i64) -> PackedByteArray {
        self.read_memory(i as _, n as _, |s| Ok(PackedByteArray::from(s)))
            .unwrap_or_default()
    }

    /// Writes a chunk of memory.
    #[func]
    fn memory_write(&self, i: i64, a: PackedByteArray) -> bool {
        let a = a.to_vec();
        self.write_memory(i as _, a.len(), |s| {
            s.copy_from_slice(&a);
            Ok(())
        })
        .is_some()
    }

    /// Reads an unsigned 8-bit integer.
    #[func]
    fn get_8(&self, i: i64) -> i64 {
        self.read_memory(i as _, 1, |s| Ok(s[0]))
            .unwrap_or_default()
            .into()
    }

    /// Writes an unsigned 8-bit integer.
    #[func]
    fn put_8(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 1, |s| {
            s[0] = (v & 255) as _;
            Ok(())
        })
        .is_some()
    }

    /// Reads an unsigned 16-bit integer.
    #[func]
    fn get_16(&self, i: i64) -> i64 {
        self.read_memory(i as _, 2, |s| Ok(u16::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    /// Writes an unsigned 16-bit integer.
    #[func]
    fn put_16(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 2, |s| {
            s.copy_from_slice(&((v & 0xffff) as u16).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads an unsigned 32-bit integer.
    #[func]
    fn get_32(&self, i: i64) -> i64 {
        self.read_memory(i as _, 4, |s| Ok(u32::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    /// Writes an unsigned 32-bit integer.
    #[func]
    fn put_32(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 4, |s| {
            s.copy_from_slice(&((v & 0xffffffff) as u32).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads a signed 64-bit integer.
    #[func]
    fn get_64(&self, i: i64) -> i64 {
        self.read_memory(i as _, 8, |s| Ok(i64::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
    }

    /// Writes a signed 64-bit integer.
    #[func]
    fn put_64(&self, i: i64, v: i64) -> bool {
        self.write_memory(i as _, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads a 32-bit floating-point number.
    #[func]
    fn get_float(&self, i: i64) -> f64 {
        self.read_memory(i as _, 4, |s| Ok(f32::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
            .into()
    }

    /// Writes a 32-bit floating-point number.
    #[func]
    fn put_float(&self, i: i64, v: f64) -> bool {
        self.write_memory(i as _, 4, |s| {
            s.copy_from_slice(&(v as f32).to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Reads a 64-bit floating-point number.
    #[func]
    fn get_double(&self, i: i64) -> f64 {
        self.read_memory(i as _, 8, |s| Ok(f64::from_le_bytes(s.try_into().unwrap())))
            .unwrap_or_default()
    }

    /// Writes a 64-bit floating-point number.
    #[func]
    fn put_double(&self, i: i64, v: f64) -> bool {
        self.write_memory(i as _, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
        .is_some()
    }

    /// Writes a `PackedArray`. Does not support `PackedStringArray`.
    #[func]
    fn put_array(&self, i: i64, v: Variant) -> bool {
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

        self.get_memory(|data| {
            let i = i as usize;
            match VariantDispatch::from(&v) {
                VariantDispatch::PackedByteArray(v) => {
                    let s = v.as_slice();
                    let e = i + s.len();
                    let Some(d) = data.get_mut(i..e) else {
                        bail_with_site!("Index out of range ({i}..{e})");
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
                        *<&mut _>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                        *<&mut _>::try_from(&mut d[4..]).unwrap() = s.y.to_le_bytes();
                    })
                }
                VariantDispatch::PackedVector3Array(v) => {
                    f::<12, _>(data, i, v.as_slice(), |s, d| {
                        *<&mut _>::try_from(&mut d[..4]).unwrap() = s.x.to_le_bytes();
                        *<&mut _>::try_from(&mut d[4..8]).unwrap() = s.y.to_le_bytes();
                        *<&mut _>::try_from(&mut d[8..]).unwrap() = s.z.to_le_bytes();
                    })
                }
                VariantDispatch::PackedColorArray(v) => {
                    f::<16, _>(data, i, v.as_slice(), |s, d| {
                        *<&mut _>::try_from(&mut d[..4]).unwrap() = s.r.to_le_bytes();
                        *<&mut _>::try_from(&mut d[4..8]).unwrap() = s.g.to_le_bytes();
                        *<&mut _>::try_from(&mut d[8..12]).unwrap() = s.b.to_le_bytes();
                        *<&mut _>::try_from(&mut d[12..]).unwrap() = s.a.to_le_bytes();
                    })
                }
                _ => bail_with_site!("Unknown value type {:?}", v.get_type()),
            }
        })
        .is_some()
    }

    /// Reads a `PackedArray`. Does not support `PackedStringArray`.
    #[func]
    fn get_array(&self, i: i64, n: i64, t: VariantType) -> Variant {
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

        option_to_variant(self.get_memory(|data| {
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
                    f::<8, PackedVector2Array>(data, i, n, |s| Vector2 {
                        x: f32::from_le_bytes(s[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(s[4..].try_into().unwrap()),
                    })
                }
                VariantType::PACKED_VECTOR3_ARRAY => {
                    f::<12, PackedVector3Array>(data, i, n, |s| Vector3 {
                        x: f32::from_le_bytes(s[..4].try_into().unwrap()),
                        y: f32::from_le_bytes(s[4..8].try_into().unwrap()),
                        z: f32::from_le_bytes(s[8..].try_into().unwrap()),
                    })
                }
                VariantType::PACKED_COLOR_ARRAY => {
                    f::<16, PackedColorArray>(data, i, n, |s| Color {
                        r: f32::from_le_bytes(s[..4].try_into().unwrap()),
                        g: f32::from_le_bytes(s[4..8].try_into().unwrap()),
                        b: f32::from_le_bytes(s[8..12].try_into().unwrap()),
                        a: f32::from_le_bytes(s[12..].try_into().unwrap()),
                    })
                }
                _ => bail_with_site!("Unsupported type ID {t:?}"),
            }
        }))
    }

    /// Reads a structured data.
    #[func]
    fn read_struct(&self, format: GString, p: i64) -> Variant {
        option_to_variant(self.get_memory(|data| read_struct(data, p as _, format.chars())))
    }

    /// Writes a structured data.
    #[func]
    fn write_struct(&self, format: GString, p: i64, arr: VariantArray) -> i64 {
        self.get_memory(|data| write_struct(data, p as _, format.chars(), arr))
            .unwrap_or_default() as _
    }
}
