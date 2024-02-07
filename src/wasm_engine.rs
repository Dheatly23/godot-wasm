use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(feature = "epoch-timeout")]
use std::{sync::Arc, thread, time};

use anyhow::{bail, Error};
use cfg_if::cfg_if;
use godot::engine::FileAccess;
use godot::prelude::*;
use once_cell::sync::{Lazy, OnceCell};
use parking_lot::Once;
#[cfg(feature = "epoch-timeout")]
use parking_lot::{Condvar, Mutex};
#[cfg(feature = "wasi-preview2")]
use wasmtime::component::Component;
use wasmtime::{Config, Engine, ExternType, Module, Precompiled, ResourcesRequired};

use crate::wasm_instance::WasmInstance;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_INTERVAL;
use crate::wasm_util::{
    from_signature, variant_to_option, PhantomProperty, VariantDispatch, MODULE_INCLUDES,
};
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

#[derive(GodotClass)]
#[class(base=Resource, init, tool)]
pub struct WasmModule {
    base: Base<Resource>,
    data: OnceCell<ModuleData>,

    #[var(get = get_name, usage_flags = [EDITOR, READ_ONLY])]
    #[allow(dead_code)]
    name: PhantomProperty<GString>,
    #[var(get = serialize, set = deserialize_bytes, usage_flags = [STORAGE, INTERNAL])]
    #[allow(dead_code)]
    bytes_data: PhantomProperty<PackedByteArray>,
    _bytes_data: OnceCell<PackedByteArray>,
}

pub struct ModuleData {
    name: GString,
    pub module: ModuleType,
    pub imports: HashMap<String, Gd<WasmModule>>,
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
                /*
                let s = format!("{:?}", e);
                error(
                    e.downcast_ref::<Site>()
                        .copied()
                        .unwrap_or_else(|| godot_site!()),
                    s,
                );
                */
                godot_error!("{:?}", e);
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
        imports: Option<Dictionary>,
    ) -> Result<HashMap<String, Gd<WasmModule>>, Error> {
        let mut deps_map = HashMap::new();
        let Some(imports) = imports else {
            return Ok(deps_map);
        };
        #[allow(irrefutable_let_patterns)]
        if let ModuleType::Core(_module) = &module {
            deps_map = HashMap::with_capacity(imports.len() as _);
            for (k, v) in imports.iter_shared() {
                let k = site_context!(String::try_from_variant(&k))?;
                let v = site_context!(<Gd<WasmModule>>::try_from_variant(&v))?;
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

    fn name_from_module(module: &ModuleType) -> GString {
        #[allow(unreachable_patterns)]
        match module {
            ModuleType::Core(m) => Some(m),
            _ => None,
        }
        .and_then(|m| m.name())
        .map_or_else(GString::new, GString::from)
    }

    fn _initialize(&self, data: Variant, imports: Option<Dictionary>) -> bool {
        match self.data.get_or_try_init(move || -> Result<_, Error> {
            let module = match VariantDispatch::from(&data) {
                VariantDispatch::PackedByteArray(v) => Self::load_module(v.as_slice())?,
                VariantDispatch::String(v) => Self::load_module(v.to_string().as_bytes())?,
                VariantDispatch::Object(v) => match v
                    .try_cast::<FileAccess>()
                    .map_err(|v| v.try_cast::<WasmModule>())
                {
                    Ok(v) => Self::load_module(&v.get_buffer(v.get_length() as _).to_vec())?,
                    Err(Ok(v)) => v.bind().get_data()?.module.clone(),
                    Err(Err(v)) => bail_with_site!("Unknown module value {}", v),
                },
                _ => bail_with_site!("Unknown module value {}", data),
            };

            let imports = Self::process_deps_map(&module, imports)?;

            Ok(ModuleData {
                name: Self::name_from_module(&module),
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
        deps_map: &HashMap<String, Gd<WasmModule>>,
    ) -> Result<(), Error> {
        for i in module.imports() {
            if MODULE_INCLUDES.iter().any(|j| *j == i.module()) {
                continue;
            }

            let ti = i.ty();
            let j = match deps_map.get(i.module()) {
                None if ti.func().is_some() => continue,
                None => bail_with_site!("Unknown module {}", i.module()),
                Some(m) => match &m.bind().get_data()?.module {
                    ModuleType::Core(m) => m.get_export(i.name()),
                    #[cfg(feature = "wasi-preview2")]
                    ModuleType::Component(_) => {
                        bail_with_site!("Import {} is a component", i.module())
                    }
                },
            };
            let j = match j {
                None if ti.func().is_some() => continue,
                Some(v) => v,
                None => {
                    bail_with_site!("No import in module {} named {}", i.module(), i.name())
                }
            };
            if !match (&ti, &j) {
                (ExternType::Func(f1), ExternType::Func(f2)) => f1 == f2,
                (ExternType::Global(g1), ExternType::Global(g2)) => g1 == g2,
                (ExternType::Table(t1), ExternType::Table(t2)) => {
                    t1.element() == t2.element()
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

    fn _deserialize(&self, data: PackedByteArray, imports: Option<Dictionary>) -> bool {
        match self.data.get_or_try_init(move || -> Result<_, Error> {
            let data = data.as_slice();
            // SAFETY: Assume the supplied data is safe to deserialize.
            let module = unsafe {
                match ENGINE.detect_precompiled(data) {
                    Some(Precompiled::Module) => {
                        ModuleType::Core(site_context!(Module::deserialize(&ENGINE, data))?)
                    }
                    #[cfg(feature = "wasi-preview2")]
                    Some(Precompiled::Component) => {
                        ModuleType::Component(site_context!(Component::deserialize(&ENGINE, data))?)
                    }
                    _ => bail_with_site!("Unsupported data content"),
                }
            };

            let imports = Self::process_deps_map(&module, imports)?;

            Ok(ModuleData {
                name: Self::name_from_module(&module),
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

    fn _deserialize_file(&self, path: String, imports: Option<Dictionary>) -> bool {
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
                name: Self::name_from_module(&module),
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

#[godot_api]
impl WasmModule {
    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[func]
    fn initialize(&self, data: Variant, imports: Dictionary) -> Option<Gd<WasmModule>> {
        if self._initialize(data, Some(imports)) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    #[func]
    fn get_name(&self) -> GString {
        self.unwrap_data(|m| Ok(m.name.clone())).unwrap_or_default()
    }

    #[func]
    fn get_imported_modules(&self) -> Array<Variant> {
        self.unwrap_data(|m| {
            Ok(<Array<Variant>>::from_iter(
                m.imports.values().map(|v| v.clone().to_variant()),
            ))
        })
        .unwrap_or_default()
    }

    #[func]
    fn deserialize(&self, data: PackedByteArray, imports: Dictionary) -> Option<Gd<WasmModule>> {
        if self._deserialize(data, Some(imports)) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    #[func]
    fn deserialize_file(&self, path: GString, imports: Dictionary) -> Option<Gd<WasmModule>> {
        if self._deserialize_file(path.to_string(), Some(imports)) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    #[func]
    fn deserialize_bytes(&self, data: PackedByteArray) {
        self._deserialize(data, None);
    }

    #[func]
    fn serialize(&self) -> PackedByteArray {
        self.unwrap_data(|m| {
            self._bytes_data.get_or_try_init(|| {
                Ok(PackedByteArray::from(
                    &match &m.module {
                        ModuleType::Core(m) => m.serialize(),
                        #[cfg(feature = "wasi-preview2")]
                        ModuleType::Component(m) => m.serialize(),
                    }?[..],
                ))
            })
        })
        .cloned()
        .unwrap_or_default()
    }

    /// Gets exported functions
    #[func]
    fn get_exports(&self) -> Dictionary {
        self.unwrap_data(|m| {
            let mut ret = Dictionary::new();
            let params_str = StringName::from_latin1_with_nul(b"params\0");
            let results_str = StringName::from_latin1_with_nul(b"results\0");
            for i in site_context!(m.module.get_core())?.exports() {
                if let ExternType::Func(f) = i.ty() {
                    let (p, r) = from_signature(&f)?;
                    ret.set(
                        i.name(),
                        Dictionary::from_iter([
                            (params_str.to_variant(), p),
                            (results_str.to_variant(), r),
                        ]),
                    );
                }
            }
            Ok(ret)
        })
        .unwrap_or_default()
    }

    /// Gets host imports signature
    #[func]
    fn get_host_imports(&self) -> Dictionary {
        self.unwrap_data(|m| {
            let mut ret = Dictionary::new();
            let params_str = StringName::from_latin1_with_nul(b"params\0");
            let results_str = StringName::from_latin1_with_nul(b"results\0");

            for i in site_context!(m.module.get_core())?.imports() {
                let ExternType::Func(f) = i.ty() else {
                    continue;
                };

                if let Some(m) = m.imports.get(i.module()) {
                    match &m.bind().get_data()?.module {
                        ModuleType::Core(m) if m.get_export(i.name()).is_some() => continue,
                        #[cfg(feature = "wasi-preview2")]
                        ModuleType::Component(_) => {
                            bail_with_site!("Import {} is a component", i.module())
                        }
                        _ => (),
                    }
                }

                let (p, r) = from_signature(&f)?;
                let mut v = match ret.get(i.module()) {
                    Some(v) => Dictionary::from_variant(&v),
                    None => {
                        let v = Dictionary::new();
                        ret.insert(i.module(), v.clone());
                        v
                    }
                };
                v.insert(
                    i.name(),
                    Dictionary::from_iter(
                        [(params_str.to_variant(), p), (results_str.to_variant(), r)].into_iter(),
                    ),
                );
            }

            Ok(ret)
        })
        .unwrap_or_default()
    }

    #[func]
    fn has_function(&self, name: StringName) -> bool {
        self.unwrap_data(|m| {
            Ok(matches!(
                site_context!(m.module.get_core())?.get_export(&name.to_string()),
                Some(ExternType::Func(_))
            ))
        })
        .unwrap_or_default()
    }

    #[func]
    fn get_signature(&self, name: StringName) -> Dictionary {
        self.unwrap_data(|m| {
            if let Some(ExternType::Func(f)) =
                site_context!(m.module.get_core())?.get_export(&name.to_string())
            {
                let (p, r) = from_signature(&f)?;
                Ok(Dictionary::from_iter([("params", p), ("results", r)]))
            } else {
                bail_with_site!("No function named {}", name);
            }
        })
        .unwrap_or_default()
    }

    #[func]
    fn get_resources_required(&self) -> Variant {
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
                return Ok(Variant::nil());
            };

            Ok([
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
            .collect::<Dictionary>()
            .to_variant())
        })
        .unwrap_or_default()
    }

    #[func]
    fn get_total_resources_required(&self) -> Variant {
        fn f(module: &ModuleData) -> Option<ResourcesRequired> {
            match &module.module {
                ModuleType::Core(m) => Some(m.resources_required()),
                #[cfg(feature = "wasi-preview2")]
                ModuleType::Component(m) => m.resources_required(),
            }
            .into_iter()
            .chain(
                module
                    .imports
                    .values()
                    .flat_map(|m| m.bind().unwrap_data(|m| Ok(f(m))).flatten()),
            )
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
                return Ok(Variant::nil());
            };

            Ok([
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
            .collect::<Dictionary>()
            .to_variant())
        })
        .unwrap_or_default()
    }

    // Instantiate module
    #[func]
    fn instantiate(&self, host: Variant, config: Variant) -> Option<Gd<WasmInstance>> {
        let Ok(host) = variant_to_option::<Dictionary>(host) else {
            godot_error!("Host is not a dictionary!");
            return None;
        };
        let config = if config.is_nil() { None } else { Some(config) };

        let inst = WasmInstance::new_gd();
        if inst.bind().initialize_(self.to_gd(), host, config) {
            Some(inst)
        } else {
            godot_error!("Error instantiating");
            None
        }
    }
}
