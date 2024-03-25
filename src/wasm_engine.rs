use std::collections::HashMap;
#[cfg(feature = "unsafe-module-serde")]
use std::path::PathBuf;
#[cfg(feature = "epoch-timeout")]
use std::{sync::Arc, thread, time};

use anyhow::{bail, Error};
use cfg_if::cfg_if;
use gdnative::export::user_data::Map;
use gdnative::log::{error, godot_site, Site};
use gdnative::prelude::*;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Once;
#[cfg(feature = "epoch-timeout")]
use parking_lot::{Condvar, Mutex};
#[cfg(feature = "wasi-preview2")]
use wasmtime::component::Component;
#[cfg(feature = "unsafe-module-serde")]
use wasmtime::Precompiled;
use wasmtime::{Config, Engine, ExternType, FuncType, Module, RefType, ResourcesRequired, ValType};

use crate::wasm_instance::WasmInstance;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_INTERVAL;
#[cfg(not(feature = "new-host-import"))]
use crate::wasm_util::HOST_MODULE;
use crate::wasm_util::{from_signature, MODULE_INCLUDES};
use crate::{bail_with_site, site_context};

#[cfg(feature = "epoch-timeout")]
#[derive(Default)]
pub struct EpochThreadHandle {
    inner: Arc<EpochThreadHandleInner>,
    once: Once,
}

#[cfg(feature = "epoch-timeout")]
#[derive(Default)]
pub struct EpochThreadHandleInner {
    mutex: Mutex<(bool, Option<thread::JoinHandle<()>>)>,
    cond: Condvar,
}

#[cfg(feature = "epoch-timeout")]
impl EpochThreadHandle {
    pub fn spawn_thread<F>(&self, f: F)
    where
        F: Fn() + 'static + Send,
    {
        self.once.call_once(move || {
            let mut guard = self.inner.mutex.lock();
            let (t, h) = &mut *guard;
            *t = false;

            let inner = self.inner.clone();
            *h = Some(thread::spawn(move || {
                let mut timeout = time::Instant::now();
                let mut guard = inner.mutex.lock();
                while !guard.0 {
                    while timeout.elapsed() >= EPOCH_INTERVAL {
                        f();
                        timeout += EPOCH_INTERVAL;
                    }
                    inner.cond.wait_until(&mut guard, timeout + EPOCH_INTERVAL);
                }
            }));
        });
    }

    pub fn stop_thread(&self) {
        let handle;

        {
            let mut guard = self.inner.mutex.lock();
            let (t, h) = &mut *guard;
            *t = true;
            handle = h.take();
        }

        self.inner.cond.notify_all();
        if let Some(handle) = handle {
            handle.join().unwrap();
        }
    }
}

#[cfg(feature = "epoch-timeout")]
impl Drop for EpochThreadHandle {
    fn drop(&mut self) {
        self.stop_thread();
    }
}

pub static ENGINE: Lazy<Engine> = Lazy::new(|| {
    let mut config = Config::new();
    config
        .cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize)
        .cranelift_nan_canonicalization(cfg!(feature = "deterministic-wasm"))
        .epoch_interruption(true)
        .wasm_reference_types(true)
        .wasm_simd(true)
        .wasm_relaxed_simd(true)
        .relaxed_simd_deterministic(cfg!(feature = "deterministic-wasm"))
        .wasm_tail_call(true)
        .wasm_bulk_memory(true)
        .wasm_multi_value(true)
        .wasm_multi_memory(true)
        .wasm_memory64(true);
    config.wasm_threads(false); // Disable threads for now
    #[cfg(feature = "wasi-preview2")]
    config.wasm_component_model(true);

    Engine::new(&config).unwrap()
});

#[cfg(feature = "epoch-timeout")]
pub static EPOCH: Lazy<EpochThreadHandle> = Lazy::new(EpochThreadHandle::default);

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::ArcData<WasmModule>)]
pub struct WasmModule {
    data: OnceCell<ModuleData>,
}

pub struct ModuleData {
    name: GodotString,
    pub module: ModuleType,
    pub imports: HashMap<String, Instance<WasmModule, Shared>>,
}

#[derive(Clone)]
pub enum ModuleType {
    Core(Module),
    #[cfg(feature = "wasi-preview2")]
    Component(Component),
}

impl ModuleType {
    pub fn get_core(&self) -> Result<&Module, Error> {
        #[allow(irrefutable_let_patterns)]
        if let Self::Core(m) = self {
            Ok(m)
        } else {
            bail!("Module is a component")
        }
    }

    #[cfg(feature = "wasi-preview2")]
    pub fn get_component(&self) -> Result<&Component, Error> {
        if let Self::Component(m) = self {
            Ok(m)
        } else {
            bail!("Module is a component")
        }
    }
}

impl WasmModule {
    fn new(_owner: &Reference) -> Self {
        Self {
            data: OnceCell::new(),
        }
    }

    pub fn get_data(&self) -> Result<&ModuleData, Error> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized module")
        }
    }

    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&ModuleData) -> Result<R, Error>,
    {
        match self.get_data().and_then(f) {
            Ok(v) => Some(v),
            Err(e) => {
                let s = format!("{:?}", e);
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    s,
                );
                None
            }
        }
    }

    fn load_module(bytes: &[u8]) -> Result<ModuleType, Error> {
        cfg_if! {
            if #[cfg(feature = "wasi-preview2")] {
                let bytes = site_context!(wat::parse_bytes(bytes))?;
                if wasmparser::Parser::is_component(&bytes) {
                    Ok(ModuleType::Component(site_context!(
                        Component::from_binary(&ENGINE, &bytes,)
                    )?))
                } else {
                    Ok(ModuleType::Core(site_context!(Module::from_binary(
                        &ENGINE, &bytes
                    ))?))
                }
            } else {
                Ok(ModuleType::Core(site_context!(Module::new(
                    &ENGINE, bytes
                ))?))
            }
        }
    }

    fn process_deps_map(
        module: &ModuleType,
        imports: Dictionary,
    ) -> Result<HashMap<String, Instance<WasmModule, Shared>>, Error> {
        let mut deps_map = HashMap::new();
        #[allow(irrefutable_let_patterns)]
        if let ModuleType::Core(_module) = &module {
            deps_map = HashMap::with_capacity(imports.len() as _);
            for (k, v) in imports.iter() {
                let k = site_context!(String::from_variant(&k))?;
                let v = site_context!(<Instance<WasmModule, Shared>>::from_variant(&v))?;
                deps_map.insert(k, v);
            }

            Self::validate_module(_module, &deps_map)?;
        }
        #[cfg(feature = "wasi-preview2")]
        if let ModuleType::Component(_) = module {
            if !imports.is_empty() {
                bail_with_site!("Imports not supported with component yet");
            }
        }

        Ok(deps_map)
    }

    fn _initialize(&self, name: GodotString, data: Variant, imports: Dictionary) -> bool {
        match self.data.get_or_try_init(move || -> Result<_, Error> {
            let module = match VariantDispatch::from(&data) {
                VariantDispatch::ByteArray(v) => Self::load_module(&v.read()),
                VariantDispatch::GodotString(v) => Self::load_module(v.to_string().as_bytes()),
                VariantDispatch::Object(v) => {
                    if let Ok(v) = <Ref<gdnative::api::File>>::from_variant(&v) {
                        let v = unsafe { v.assume_safe() };
                        Self::load_module(&v.get_buffer(v.get_len()).read())
                    } else {
                        let v = site_context!(<Instance<WasmModule, Shared>>::from_variant(&v))?;
                        let v = unsafe { v.assume_safe() };
                        v.map(|this, _| Ok(this.get_data()?.module.clone()))
                            .unwrap()
                    }
                }
                _ => bail_with_site!("Unknown module value {}", data),
            }?;

            let imports = Self::process_deps_map(&module, imports)?;

            Ok(ModuleData {
                name,
                module,
                imports,
            })
        }) {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{:?}", e);
                false
            }
        }
    }

    fn validate_module(
        module: &Module,
        deps_map: &HashMap<String, Instance<WasmModule, Shared>>,
    ) -> Result<(), Error> {
        for i in module.imports() {
            if MODULE_INCLUDES.iter().any(|j| *j == i.module()) {
                continue;
            }

            let ti = i.ty();
            let j = match deps_map.get(i.module()) {
                #[cfg(feature = "new-host-import")]
                None if ti.func().is_some() => continue,
                None => bail_with_site!("Unknown module {}", i.module()),
                Some(m) => m
                    .script()
                    .map(|m| -> Result<_, Error> {
                        match &m.get_data()?.module {
                            ModuleType::Core(m) => Ok(m.get_export(i.name())),
                            #[cfg(feature = "wasi-preview2")]
                            ModuleType::Component(_) => {
                                bail_with_site!("Import {} is a component", i.module())
                            }
                        }
                    })
                    .unwrap()?,
            };
            let j = match j {
                #[cfg(feature = "new-host-import")]
                None if ti.func().is_some() => continue,
                Some(v) => v,
                None => {
                    bail_with_site!("No import in module {} named {}", i.module(), i.name())
                }
            };
            if !match (&ti, &j) {
                (ExternType::Func(f1), ExternType::Func(f2)) => FuncType::matches(f1, f2),
                (ExternType::Global(g1), ExternType::Global(g2)) => {
                    ValType::matches(g1.content(), g2.content())
                        && g1.mutability() == g2.mutability()
                }
                (ExternType::Table(t1), ExternType::Table(t2)) => {
                    RefType::matches(t1.element(), t2.element())
                        && t1.minimum() <= t2.minimum()
                        && match (t1.maximum(), t2.maximum()) {
                            (None, _) => true,
                            (_, None) => false,
                            (Some(a), Some(b)) => a >= b,
                        }
                }
                (ExternType::Memory(m1), ExternType::Memory(m2)) => {
                    m1.is_64() == m2.is_64()
                        && m1.is_shared() == m2.is_shared()
                        && m1.minimum() <= m2.minimum()
                        && match (m1.maximum(), m2.maximum()) {
                            (None, _) => true,
                            (_, None) => false,
                            (Some(a), Some(b)) => a >= b,
                        }
                }
                (_, _) => false,
            } {
                bail_with_site!("Import type mismatch ({:?} != {:?})", ti, j)
            }
        }

        Ok(())
    }

    #[cfg(feature = "unsafe-module-serde")]
    fn _deserialize(&self, name: GodotString, data: PoolArray<u8>, imports: Dictionary) -> bool {
        match self.data.get_or_try_init(move || -> Result<_, Error> {
            let data = data.read();
            // SAFETY: Assume the supplied data is safe to deserialize.
            let module = unsafe {
                match ENGINE.detect_precompiled(&data) {
                    Some(Precompiled::Module) => {
                        ModuleType::Core(site_context!(Module::deserialize(&ENGINE, &*data))?)
                    }
                    #[cfg(feature = "wasi-preview2")]
                    Some(Precompiled::Component) => ModuleType::Component(site_context!(
                        Component::deserialize(&ENGINE, &*data)
                    )?),
                    _ => bail_with_site!("Unsupported data content"),
                }
            };

            let imports = Self::process_deps_map(&module, imports)?;

            Ok(ModuleData {
                name,
                module,
                imports,
            })
        }) {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{:?}", e);
                false
            }
        }
    }

    #[cfg(feature = "unsafe-module-serde")]
    fn _deserialize_file(&self, name: GodotString, path: String, imports: Dictionary) -> bool {
        match self.data.get_or_try_init(move || -> Result<_, Error> {
            let path = PathBuf::from(path);
            // SAFETY: Assume the supplied file is safe to deserialize.
            let module = unsafe {
                match site_context!(ENGINE.detect_precompiled_file(&path))? {
                    Some(Precompiled::Module) => {
                        ModuleType::Core(site_context!(Module::deserialize_file(&ENGINE, path))?)
                    }
                    #[cfg(feature = "wasi-preview2")]
                    Some(Precompiled::Component) => ModuleType::Component(site_context!(
                        Component::deserialize_file(&ENGINE, path)
                    )?),
                    _ => bail_with_site!("Unsupported data content"),
                }
            };

            let imports = Self::process_deps_map(&module, imports)?;

            Ok(ModuleData {
                name,
                module,
                imports,
            })
        }) {
            Ok(_) => true,
            Err(e) => {
                godot_error!("{:?}", e);
                false
            }
        }
    }
}

#[methods]
impl WasmModule {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .property::<Option<GodotString>>("name")
            .with_getter(|v, _| v.unwrap_data(|m| Ok(m.name.clone())))
            .done();

        builder
            .property::<bool>("is_component")
            .with_getter(|v, _| {
                v.unwrap_data(|_m| {
                    #[cfg(feature = "wasi-preview2")]
                    return Ok(matches!(_m.module, ModuleType::Component(_)));

                    #[cfg(not(feature = "wasi-preview2"))]
                    Ok(false)
                })
                .unwrap_or_default()
            })
            .done();

        builder
            .property::<bool>("is_core_module")
            .with_getter(|v, _| {
                v.unwrap_data(|_m| {
                    #[cfg(feature = "wasi-preview2")]
                    return Ok(matches!(_m.module, ModuleType::Core(_)));

                    #[cfg(not(feature = "wasi-preview2"))]
                    Ok(true)
                })
                .unwrap_or_default()
            })
            .done();
    }

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[method]
    fn initialize(
        &self,
        #[base] owner: TRef<Reference>,
        name: GodotString,
        data: Variant,
        imports: Dictionary,
    ) -> Option<Ref<Reference>> {
        if self._initialize(name, data, imports) {
            Some(owner.claim())
        } else {
            None
        }
    }

    #[method]
    fn deserialize(
        &self,
        #[base] _owner: TRef<Reference>,
        _name: GodotString,
        _data: PoolArray<u8>,
        _imports: Dictionary,
    ) -> Option<Ref<Reference>> {
        cfg_if! {
            if #[cfg(feature = "unsafe-module-serde")] {
                if self._deserialize(_name, _data, _imports) {
                    Some(_owner.claim())
                } else {
                    None
                }
            } else {
            panic!("Feature unsafe-module-serde is not enabled!");
            }
        }
    }

    #[method]
    fn deserialize_file(
        &self,
        #[base] _owner: TRef<Reference>,
        _name: GodotString,
        _path: String,
        _imports: Dictionary,
    ) -> Option<Ref<Reference>> {
        cfg_if! {
            if #[cfg(feature = "unsafe-module-serde")] {
                if self._deserialize_file(_name, _path, _imports) {
                    Some(_owner.claim())
                } else {
                    None
                }
            } else {
                panic!("Feature unsafe-module-serde is not enabled!");
            }
        }
    }

    #[method]
    fn serialize(&self) -> Option<PoolArray<u8>> {
        cfg_if! {
            if #[cfg(feature = "unsafe-module-serde")] {
                self.unwrap_data(|m| {
                    Ok(PoolArray::from_vec(match &m.module {
                        ModuleType::Core(m) => m.serialize(),
                        #[cfg(feature = "wasi-preview2")]
                        ModuleType::Component(m) => m.serialize(),
                    }?))
                })
            } else {
            panic!("Feature unsafe-module-serde is not enabled!");
            }
        }
    }

    #[method]
    fn get_imported_modules(&self) -> Option<VariantArray> {
        self.unwrap_data(|m| Ok(VariantArray::from_iter(m.imports.values().cloned()).into_shared()))
    }

    /// Gets exported functions
    #[method]
    fn get_exports(&self) -> Option<Dictionary> {
        self.unwrap_data(|m| {
            let ret = Dictionary::new();
            let params_str = GodotString::from_str("params");
            let results_str = GodotString::from_str("results");
            for i in site_context!(m.module.get_core())?.exports() {
                if let ExternType::Func(f) = i.ty() {
                    let (p, r) = from_signature(&f)?;
                    ret.insert(
                        i.name(),
                        Dictionary::from_iter(
                            [(params_str.to_variant(), p), (results_str.to_variant(), r)]
                                .into_iter(),
                        ),
                    );
                }
            }
            Ok(ret.into_shared())
        })
    }

    /// Gets host imports signature
    #[method]
    fn get_host_imports(&self) -> Option<Dictionary> {
        self.unwrap_data(|m| {
            let ret = Dictionary::new();
            let params_str = GodotString::from_str("params");
            let results_str = GodotString::from_str("results");

            cfg_if! {
                if #[cfg(feature = "new-host-import")] {
                    for i in site_context!(m.module.get_core())?.imports() {
                        let ExternType::Func(f) = i.ty() else {
                            continue;
                        };

                        if let Some(m) = m.imports.get(i.module()) {
                            if m.script()
                                .map(|m| -> Result<_, Error> {
                                    match &m.get_data()?.module {
                                        ModuleType::Core(m) => Ok(m.get_export(i.name())),
                                        #[cfg(feature = "wasi-preview2")]
                                        ModuleType::Component(_) => {
                                            bail_with_site!("Import {} is a component", i.module())
                                        }
                                    }
                                })
                                .unwrap()?
                                .is_some()
                            {
                                continue;
                            }
                        }

                        let (p, r) = from_signature(&f)?;
                        let v = match ret.get(i.module()) {
                            Some(v) => Dictionary::from_variant(&v).unwrap(),
                            None => {
                                let v = Dictionary::new_shared();
                                ret.insert(i.module(), v.new_ref());
                                v
                            }
                        };
                        unsafe {
                            v.assume_unique().insert(
                                i.name(),
                                Dictionary::from_iter(
                                    [(params_str.to_variant(), p), (results_str.to_variant(), r)]
                                        .into_iter(),
                                ),
                            )
                        };
                    }
                } else {
                    for i in site_context!(m.module.get_core())?.imports() {
                        if i.module() != HOST_MODULE {
                            continue;
                        }
                        if let ExternType::Func(f) = i.ty() {
                            let (p, r) = from_signature(&f)?;
                            ret.insert(
                                i.name(),
                                Dictionary::from_iter(
                                    [(params_str.to_variant(), p), (results_str.to_variant(), r)]
                                        .into_iter(),
                                ),
                            );
                        }
                    }
                }
            }

            Ok(ret.into_shared())
        })
    }

    #[method]
    fn has_function(&self, name: String) -> bool {
        self.unwrap_data(|m| {
            Ok(matches!(
                site_context!(m.module.get_core())?.get_export(&name),
                Some(ExternType::Func(_))
            ))
        })
        .unwrap_or_default()
    }

    #[method]
    fn get_signature(&self, name: String) -> Option<Dictionary> {
        self.unwrap_data(|m| {
            if let Some(ExternType::Func(f)) = site_context!(m.module.get_core())?.get_export(&name)
            {
                let (p, r) = from_signature(&f)?;
                Ok(Dictionary::from_iter([("params", p), ("results", r)]).into_shared())
            } else {
                bail_with_site!("No function named {}", name);
            }
        })
    }

    #[method]
    fn get_resources_required(&self) -> Option<Dictionary> {
        self.unwrap_data(|m| {
            let v = match &m.module {
                ModuleType::Core(m) => Some(m.resources_required()),
                #[cfg(feature = "wasi-preview2")]
                ModuleType::Component(m) => m.resources_required(),
            };
            let Some(ResourcesRequired {
                num_memories,
                max_initial_memory_size,
                num_tables,
                max_initial_table_size,
            }) = v
            else {
                return Ok(None);
            };

            Ok(Some(
                [
                    ("num_memories", num_memories.to_variant()),
                    ("num_tables", num_tables.to_variant()),
                    (
                        "max_initial_memory_size",
                        max_initial_memory_size.unwrap_or_default().to_variant(),
                    ),
                    (
                        "max_initial_table_size",
                        max_initial_table_size.unwrap_or_default().to_variant(),
                    ),
                ]
                .into_iter()
                .collect::<Dictionary<_>>()
                .into_shared(),
            ))
        })
        .flatten()
    }

    #[method]
    fn get_total_resources_required(&self) -> Option<Dictionary> {
        fn f(module: &ModuleData) -> Option<ResourcesRequired> {
            match &module.module {
                ModuleType::Core(m) => Some(m.resources_required()),
                #[cfg(feature = "wasi-preview2")]
                ModuleType::Component(m) => m.resources_required(),
            }
            .into_iter()
            .chain(module.imports.values().flat_map(|m| {
                m.script()
                    .map(|m| m.unwrap_data(|m| Ok(f(m))))
                    .unwrap()
                    .flatten()
            }))
            .reduce(|a, b| ResourcesRequired {
                num_memories: a.num_memories + b.num_memories,
                num_tables: a.num_tables + b.num_tables,
                max_initial_memory_size: a.max_initial_memory_size.max(b.max_initial_memory_size),
                max_initial_table_size: a.max_initial_table_size.max(b.max_initial_table_size),
            })
        }

        self.unwrap_data(|m| {
            let Some(ResourcesRequired {
                num_memories,
                max_initial_memory_size,
                num_tables,
                max_initial_table_size,
            }) = f(m)
            else {
                return Ok(None);
            };

            Ok(Some(
                [
                    ("num_memories", num_memories.to_variant()),
                    ("num_tables", num_tables.to_variant()),
                    (
                        "max_initial_memory_size",
                        max_initial_memory_size.unwrap_or_default().to_variant(),
                    ),
                    (
                        "max_initial_table_size",
                        max_initial_table_size.unwrap_or_default().to_variant(),
                    ),
                ]
                .into_iter()
                .collect::<Dictionary<_>>()
                .into_shared(),
            ))
        })
        .flatten()
    }

    // Instantiate module
    #[method]
    fn instantiate(
        &self,
        #[base] owner: TRef<Reference>,
        #[opt] host: Option<Dictionary>,
        #[opt] config: Option<Variant>,
    ) -> Option<Instance<WasmInstance, Shared>> {
        let inst = WasmInstance::new_instance();
        if let Ok(true) = inst.map(|v, b| {
            if let Some(i) = Instance::from_base(owner.claim()) {
                v.initialize_(b.as_ref(), i, host, config)
            } else {
                false
            }
        }) {
            Some(inst.into_shared())
        } else {
            None
        }
    }
}
