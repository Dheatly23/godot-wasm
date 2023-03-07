use std::collections::HashMap;
use std::mem::transmute;
#[cfg(feature = "epoch-timeout")]
use std::{sync::Arc, thread, time};

use anyhow::{anyhow, bail, Error};
use godot::engine::FileAccess;
use godot::prelude::*;
use lazy_static::lazy_static;
#[cfg(feature = "epoch-timeout")]
use parking_lot::{Condvar, Mutex};
use parking_lot::{Once, OnceState};
use wasmtime::{Config, Engine, ExternType, Module};

use crate::wasm_instance::WasmInstance;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_INTERVAL;
use crate::wasm_util::{from_signature, variant_to_option, HOST_MODULE, MODULE_INCLUDES};

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
    pub static ref ENGINE: Engine = Engine::new(
        Config::new()
            .cranelift_opt_level(wasmtime::OptLevel::SpeedAndSize)
            .epoch_interruption(true)
            .wasm_reference_types(true)
            .wasm_simd(true)
            .wasm_bulk_memory(true)
            .wasm_multi_value(true)
            .wasm_multi_memory(true)
            .wasm_memory64(true)
    )
    .unwrap();
}

#[cfg(feature = "epoch-timeout")]
lazy_static! {
    pub static ref EPOCH: EpochThreadHandle = EpochThreadHandle::default();
}

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct WasmModule {
    #[base]
    base: Base<RefCounted>,
    once: Once,
    data: Option<ModuleData>,
}

pub struct ModuleData {
    name: GodotString,
    pub module: Module,
    pub imports: HashMap<String, Gd<WasmModule>>,
}

impl WasmModule {
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

    fn _initialize(&self, name: GodotString, data: Variant, imports: Dictionary) -> bool {
        let f = move || -> Result<(), Error> {
            let module = if let Ok(v) = PackedByteArray::try_from_variant(&data) {
                Module::new(&ENGINE, &v.to_vec())?
            } else if let Ok(v) = String::try_from_variant(&data) {
                Module::new(&ENGINE, &v)?
            } else if let Ok(v) = <Gd<FileAccess>>::try_from_variant(&data) {
                Module::new(&ENGINE, &v.get_buffer(v.get_length()).to_vec())?
            } else {
                bail!("Unknown module value {}", data)
            };

            let mut deps_map = HashMap::with_capacity(imports.len() as _);
            for (k, v) in imports.iter_shared() {
                let k = String::try_from_variant(&k).map_err(|e| anyhow!("{:?}", e))?;
                let v = <Gd<WasmModule>>::try_from_variant(&v).map_err(|e| anyhow!("{:?}", e))?;
                deps_map.insert(k, v);
            }

            for i in module.imports() {
                if MODULE_INCLUDES.iter().any(|j| *j == i.module()) {
                    continue;
                }

                match deps_map.get(i.module()) {
                    None => bail!("Unknown module {}", i.module()),
                    Some(m) => match (i.ty(), m.bind().get_data()?.module.get_export(i.name())) {
                        (_, None) => {
                            bail!("No import in module {} named {}", i.module(), i.name())
                        }
                        (ExternType::Func(f1), Some(ExternType::Func(f2))) if f1 == f2 => (),
                        (ExternType::Global(g1), Some(ExternType::Global(g2))) if g1 == g2 => (),
                        (ExternType::Table(t1), Some(ExternType::Table(t2))) if t1 == t2 => (),
                        (ExternType::Memory(m1), Some(ExternType::Memory(m2))) if m1 == m2 => (),
                        (e1, Some(e2)) => {
                            bail!("Import type mismatch ({:?} != {:?})", e1, e2)
                        }
                    },
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
                godot_error!("{}", e);
                *ret = false;
            }
        });

        r
    }
}

#[godot_api]
impl RefCountedVirtual for WasmModule {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            base,
            once: Once::new(),
            data: None,
        }
    }
}

#[godot_api]
impl WasmModule {
    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[func]
    fn initialize(&self, name: GodotString, data: Variant, imports: Dictionary) -> Gd<WasmModule> {
        let ret = if self._initialize(name, data, imports) {
            <Gd<WasmModule>>::try_from_instance_id(self.base.instance_id())
        } else {
            None
        };
        ret.unwrap()
    }

    #[func]
    fn get_imported_modules(&self) -> Array<Variant> {
        self.unwrap_data(|m| {
            Ok(<Array<Variant>>::from_iter(
                m.imports.values().map(|v| v.share().to_variant()),
            ))
        })
        .unwrap_or_default()
    }

    /// Gets exported functions
    #[func]
    fn get_exports(&self) -> Dictionary {
        self.unwrap_data(|m| {
            let mut ret = Dictionary::new();
            let params_str = GodotString::from("params");
            let results_str = GodotString::from("results");
            for i in m.module.exports() {
                if let ExternType::Func(f) = i.ty() {
                    let (p, r) = from_signature(&f)?;
                    ret.set(
                        i.name(),
                        Dictionary::from_iter(
                            [(params_str.to_variant(), p), (results_str.to_variant(), r)]
                                .into_iter(),
                        ),
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
            let params_str = GodotString::from("params");
            let results_str = GodotString::from("results");
            for i in m.module.imports() {
                if i.module() != HOST_MODULE {
                    continue;
                }
                if let ExternType::Func(f) = i.ty() {
                    let (p, r) = from_signature(&f)?;
                    ret.set(
                        i.name(),
                        Dictionary::from_iter(
                            [(params_str.to_variant(), p), (results_str.to_variant(), r)]
                                .into_iter(),
                        ),
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
                m.module.get_export(&name.to_string()),
                Some(ExternType::Func(_))
            ))
        })
        .unwrap_or_default()
    }

    #[func]
    fn get_signature(&self, name: StringName) -> Dictionary {
        self.unwrap_data(|m| {
            if let Some(ExternType::Func(f)) = m.module.get_export(&name.to_string()) {
                let (p, r) = from_signature(&f)?;
                Ok(Dictionary::from_iter(
                    [("params", p), ("results", r)].into_iter(),
                ))
            } else {
                bail!("No function named {}", name);
            }
        })
        .unwrap_or_default()
    }

    // Instantiate module
    #[func]
    fn instantiate(&self, host: Variant, config: Variant) -> Gd<WasmInstance> {
        let Ok(host) = variant_to_option::<Dictionary>(host) else {
            panic!("Host is not a dictionary!")
        };
        let config = if config.is_nil() { None } else { Some(config) };

        let inst = <Gd<WasmInstance>>::new_default();
        if inst
            .bind()
            .initialize_(Gd::from_instance_id(self.base.instance_id()), host, config)
        {
            inst
        } else {
            panic!("Error instantiating")
        }
    }
}
