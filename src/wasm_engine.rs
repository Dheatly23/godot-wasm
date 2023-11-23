use std::collections::HashMap;
use std::mem::transmute;
#[cfg(feature = "epoch-timeout")]
use std::{sync::Arc, thread, time};

use anyhow::{bail, Error};
use gdnative::export::user_data::Map;
use gdnative::prelude::*;
use lazy_static::lazy_static;
#[cfg(feature = "epoch-timeout")]
use parking_lot::{Condvar, Mutex};
use parking_lot::{Once, OnceState};
#[cfg(feature = "wasi-preview2")]
use wasmtime::component::Component;
use wasmtime::{Config, Engine, ExternType, Module};

use crate::wasm_instance::WasmInstance;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_INTERVAL;
use crate::wasm_util::{from_signature, HOST_MODULE, MODULE_INCLUDES};

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

        Engine::new(&mut config).unwrap()
    };
}

#[cfg(feature = "epoch-timeout")]
lazy_static! {
    pub static ref EPOCH: EpochThreadHandle = EpochThreadHandle::default();
}

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::ArcData<WasmModule>)]
pub struct WasmModule {
    once: Once,
    data: Option<ModuleData>,
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
            once: Once::new(),
            data: None,
        }
    }

    pub fn get_data(&self) -> Result<&ModuleData, Error> {
        if let OnceState::Done = self.once.state() {
            Ok(self.data.as_ref().unwrap())
        } else {
            bail!("Uninitialized module")
        }
    }

    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&ModuleData) -> Result<R, Error>,
    {
        match self.get_data().and_then(f) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{:?}", e);
                None
            }
        }
    }

    fn load_module(bytes: &[u8]) -> Result<ModuleType, Error> {
        #[cfg(feature = "wasi-preview2")]
        {
            let bytes = wat::parse_bytes(bytes)?;
            if wasmparser::Parser::is_component(&bytes) {
                Ok(ModuleType::Component(Component::new(&ENGINE, &bytes)?))
            } else {
                Ok(ModuleType::Core(Module::new(&ENGINE, &bytes)?))
            }
        }

        #[cfg(not(feature = "wasi-preview2"))]
        Ok(ModuleType::Core(Module::new(&ENGINE, bytes)?))
    }

    fn _initialize(&self, name: GodotString, data: Variant, imports: Dictionary) -> bool {
        let f = move || -> Result<(), Error> {
            let module = match VariantDispatch::from(&data) {
                VariantDispatch::ByteArray(v) => Self::load_module(&*v.read()),
                VariantDispatch::GodotString(v) => Self::load_module(v.to_string().as_bytes()),
                VariantDispatch::Object(v) => {
                    if let Ok(v) = <Ref<gdnative::api::File>>::from_variant(&v) {
                        let v = unsafe { v.assume_safe() };
                        Self::load_module(&*v.get_buffer(v.get_len()).read())
                    } else {
                        let v = <Instance<WasmModule, Shared>>::from_variant(&v)?;
                        let v = unsafe { v.assume_safe() };
                        v.map(|this, _| Ok(this.get_data()?.module.clone()))
                            .unwrap()
                    }
                }
                _ => bail!("Unknown module value {}", data),
            }?;

            let mut deps_map = HashMap::new();
            #[allow(irrefutable_let_patterns)]
            if let ModuleType::Core(module) = &module {
                deps_map = HashMap::with_capacity(imports.len() as _);
                for (k, v) in imports.iter() {
                    let k = String::from_variant(&k)?;
                    let v = <Instance<WasmModule, Shared>>::from_variant(&v)?;
                    deps_map.insert(k, v);
                }

                Self::validate_module(module, &deps_map)?;
            }
            #[cfg(feature = "wasi-preview2")]
            if let ModuleType::Component(_) = module {
                if imports.len() > 0 {
                    bail!("Imports not supported with component yet");
                }
            }

            // SAFETY: Should be called only once and nobody else can read module data
            #[allow(mutable_transmutes)]
            let this = unsafe { transmute::<&Self, &mut Self>(self) };
            this.data = Some(ModuleData {
                name,
                module,
                imports: deps_map,
            });

            Ok(())
        };

        let mut r = true;
        let ret = &mut r;

        self.once.call_once(move || match f() {
            Ok(()) => (),
            Err(e) => {
                godot_error!("{:?}", e);
                *ret = false;
            }
        });

        r
    }

    fn validate_module(
        module: &Module,
        deps_map: &HashMap<String, Instance<WasmModule, Shared>>,
    ) -> Result<(), Error> {
        for i in module.imports() {
            if MODULE_INCLUDES.iter().any(|j| *j == i.module()) {
                continue;
            }

            let j = match deps_map.get(i.module()) {
                None => bail!("Unknown module {}", i.module()),
                Some(m) => m
                    .script()
                    .map(|m| -> Result<_, Error> {
                        match &m.get_data()?.module {
                            ModuleType::Core(m) => Ok(m.get_export(i.name())),
                            #[cfg(feature = "wasi-preview2")]
                            ModuleType::Component(_) => {
                                bail!("Import {} is a component", i.module())
                            }
                        }
                    })
                    .unwrap()?,
            };
            let j = match j {
                Some(v) => v,
                None => {
                    bail!("No import in module {} named {}", i.module(), i.name())
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
                bail!("Import type mismatch ({:?} != {:?})", i, j)
            }
        }

        Ok(())
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
            for i in m.module.get_core()?.exports() {
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
            for i in m.module.get_core()?.imports() {
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
            Ok(ret.into_shared())
        })
    }

    #[method]
    fn has_function(&self, name: String) -> bool {
        self.unwrap_data(|m| {
            Ok(matches!(
                m.module.get_core()?.get_export(&name),
                Some(ExternType::Func(_))
            ))
        })
        .unwrap_or_default()
    }

    #[method]
    fn get_signature(&self, name: String) -> Option<Dictionary> {
        self.unwrap_data(|m| {
            if let Some(ExternType::Func(f)) = m.module.get_core()?.get_export(&name) {
                let (p, r) = from_signature(&f)?;
                Ok(Dictionary::from_iter([("params", p), ("results", r)]).into_shared())
            } else {
                bail!("No function named {}", name);
            }
        })
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
