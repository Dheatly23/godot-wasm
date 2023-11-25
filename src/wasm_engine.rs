use std::collections::HashMap;
#[cfg(feature = "epoch-timeout")]
use std::{sync::Arc, thread, time};

use anyhow::{anyhow, bail, Error};
use godot::engine::FileAccess;
use godot::prelude::*;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use parking_lot::Once;
#[cfg(feature = "epoch-timeout")]
use parking_lot::{Condvar, Mutex};
#[cfg(feature = "wasi-preview2")]
use wasmtime::component::Component;
use wasmtime::{Config, Engine, ExternType, Module};

use crate::wasm_instance::WasmInstance;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_INTERVAL;
use crate::wasm_util::{from_signature, variant_to_option, HOST_MODULE, MODULE_INCLUDES};
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

lazy_static! {
    pub static ref ENGINE: Engine = {
        let mut config = Config::new();
        config.cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize)
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
    };
}

#[cfg(feature = "epoch-timeout")]
lazy_static! {
    pub static ref EPOCH: EpochThreadHandle = EpochThreadHandle::default();
}

#[derive(GodotClass)]
#[class(base=RefCounted, init)]
pub struct WasmModule {
    #[base]
    base: Base<RefCounted>,
    data: OnceCell<ModuleData>,

    #[var(get = get_name)]
    #[allow(dead_code)]
    name: GString,
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
                    &s,
                );
                */
                godot_error!("{:?}", e);
                None
            }
        }
    }

    fn load_module(bytes: &[u8]) -> Result<ModuleType, Error> {
        #[cfg(feature = "wasi-preview2")]
        {
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
        }

        #[cfg(not(feature = "wasi-preview2"))]
        Ok(ModuleType::Core(site_context!(Module::new(
            &ENGINE, bytes
        ))?))
    }

    fn _initialize(&self, name: GString, data: Variant, imports: Dictionary) -> bool {
        match self.data.get_or_try_init(move || -> Result<_, Error> {
            let module = if let Ok(v) = PackedByteArray::try_from_variant(&data) {
                Self::load_module(&v.to_vec())?
            } else if let Ok(v) = String::try_from_variant(&data) {
                Self::load_module(v.as_bytes())?
            } else if let Ok(v) = <Gd<FileAccess>>::try_from_variant(&data) {
                Self::load_module(&v.get_buffer(v.get_length() as _).to_vec())?
            } else if let Ok(v) = <Gd<WasmModule>>::try_from_variant(&data) {
                v.bind().get_data()?.module.clone()
            } else {
                bail!("Unknown module value {}", data)
            };

            let mut deps_map = HashMap::new();
            #[allow(irrefutable_let_patterns)]
            if let ModuleType::Core(module) = &module {
                deps_map = HashMap::with_capacity(imports.len() as _);
                for (k, v) in imports.iter_shared() {
                    let k = String::try_from_variant(&k)
                        .map_err(|e| site_context!(anyhow!("{:?}", e)))?;
                    let v = <Gd<WasmModule>>::try_from_variant(&v)
                        .map_err(|e| site_context!(anyhow!("{:?}", e)))?;
                    deps_map.insert(k, v);
                }

                Self::validate_module(module, &deps_map)?;
            }
            #[cfg(feature = "wasi-preview2")]
            if let ModuleType::Component(_) = module {
                if !imports.is_empty() {
                    bail_with_site!("Imports not supported with component yet");
                }
            }

            Ok(ModuleData {
                name,
                module,
                imports: deps_map,
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

            let j = match deps_map.get(i.module()) {
                None => bail!("Unknown module {}", i.module()),
                Some(m) => match &m.bind().get_data()?.module {
                    ModuleType::Core(m) => m.get_export(i.name()),
                    #[cfg(feature = "wasi-preview2")]
                    ModuleType::Component(_) => {
                        bail_with_site!("Import {} is a component", i.module())
                    }
                },
            };
            let j = match j {
                Some(v) => v,
                None => {
                    bail_with_site!("No import in module {} named {}", i.module(), i.name())
                }
            };
            let i = i.ty();
            if !match (&i, &j) {
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
                bail_with_site!("Import type mismatch ({:?} != {:?})", i, j)
            }
        }

        Ok(())
    }
}

#[godot_api]
impl WasmModule {
    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[func]
    fn initialize(&self, name: GString, data: Variant, imports: Dictionary) -> Gd<WasmModule> {
        let ret = if self._initialize(name, data, imports) {
            <Gd<WasmModule>>::try_from_instance_id(self.base.instance_id())
        } else {
            None
        };
        ret.unwrap()
    }

    #[func]
    fn get_name(&self) -> StringName {
        self.unwrap_data(|m| Ok(StringName::from(&m.name)))
            .unwrap_or_default()
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

    /// Gets exported functions
    #[func]
    fn get_exports(&self) -> Dictionary {
        self.unwrap_data(|m| {
            let mut ret = Dictionary::new();
            let params_str = StringName::from("params");
            let results_str = StringName::from("results");
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
            let params_str = StringName::from("params");
            let results_str = StringName::from("results");
            for i in site_context!(m.module.get_core())?.imports() {
                if i.module() != HOST_MODULE {
                    continue;
                }
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

    // Instantiate module
    #[func]
    fn instantiate(&self, host: Variant, config: Variant) -> Option<Gd<WasmInstance>> {
        let Ok(host) = variant_to_option::<Dictionary>(host) else {
            godot_error!("Host is not a dictionary!");
            return None;
        };
        let config = if config.is_nil() { None } else { Some(config) };

        let inst = WasmInstance::new_gd();
        if inst
            .bind()
            .initialize_(Gd::from_instance_id(self.base.instance_id()), host, config)
        {
            Some(inst)
        } else {
            godot_error!("Error instantiating");
            None
        }
    }
}
