use std::collections::HashMap;
use std::mem::transmute;

use anyhow::{bail, Error};
use gdnative::export::user_data::Map;
use gdnative::prelude::*;
use lazy_static::lazy_static;
use parking_lot::{Once, OnceState};
use wasmer::{Cranelift, Engine, ExportType, ExternType, Features, Module, Store, Target};

use crate::variant_typecast;
use crate::wasm_instance::WasmInstance;
use crate::wasm_util::{from_signature, HOST_MODULE, MODULE_INCLUDES};

lazy_static! {
    pub static ref ENGINE: Store = Store::new(&Engine::headless());
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
    pub module: Module,
    pub imports: HashMap<String, Instance<WasmModule, Shared>>,
    pub exports: Vec<ExportType>,
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

    fn _initialize(&self, name: GodotString, data: Variant, imports: Dictionary) -> bool {
        let f = move || -> Result<(), Error> {
            let compile_engine = {
                let mut compiler = Cranelift::default();
                let target = Target::default();
                let mut features = Features::default();
                compiler
                    .canonicalize_nans(false)
                    .opt_level(wasmer::CraneliftOptLevel::SpeedAndSize);
                features
                    .reference_types(true)
                    .simd(true)
                    .bulk_memory(true)
                    .multi_value(true)
                    .multi_memory(true)
                    .memory64(true);

                Store::new(&Engine::new(Box::new(compiler), target, features))
            };

            let module = variant_typecast!((data) {
                m: ByteArray => Module::new(&compile_engine, &*m.read())?,
                m: String => Module::new(&compile_engine, &m)?,
                m: Ref<gdnative::api::File> => unsafe {
                    let m = m.assume_safe();
                    Module::new(&compile_engine, &*m.get_buffer(m.get_len()).read())?
                },
                _ @ v => bail!("Unknown module value {}", v),
            });

            let mut deps_map = HashMap::with_capacity(imports.len() as _);
            for (k, v) in imports.iter() {
                let k = String::from_variant(&k)?;
                let v = <Instance<WasmModule, Shared>>::from_variant(&v)?;
                deps_map.insert(k, v);
            }

            for i in module.imports() {
                if MODULE_INCLUDES.iter().any(|j| *j == i.module()) {
                    continue;
                }

                match deps_map.get(i.module()) {
                    None => bail!("Unknown module {}", i.module()),
                    Some(m) => m
                        .script()
                        .map(|m| {
                            if !m
                                .get_data()?
                                .exports
                                .iter()
                                .any(|j| i.name() == j.name() && i.ty() == j.ty())
                            {
                                bail!("No import in module {} named {}", i.module(), i.name());
                            }
                            Ok(())
                        })
                        .unwrap()?,
                }
            }

            // SAFETY: Should be called only once and nobody else can read module data
            #[allow(mutable_transmutes)]
            let this = unsafe { transmute::<&Self, &mut Self>(self) };
            this.data = Some(ModuleData {
                name,
                module: unsafe { Module::deserialize(&*ENGINE, module.serialize()?)? },
                imports: deps_map,
                exports: module.exports().collect(),
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

#[methods]
impl WasmModule {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .property::<Option<GodotString>>("name")
            .with_getter(|v, _| match v.get_data() {
                Ok(m) => Some(GodotString::clone(&m.name)),
                Err(e) => {
                    godot_error!("{}", e);
                    None
                }
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
        match self
            .get_data()
            .and_then(|m| Ok(VariantArray::from_iter(m.imports.values().cloned()).into_shared()))
        {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    /// Gets exported functions
    #[method]
    fn get_exports(&self) -> Option<Dictionary> {
        match self.get_data().and_then(|m| {
            let ret = Dictionary::new();
            let params_str = GodotString::from_str("params");
            let results_str = GodotString::from_str("results");
            for i in &m.exports {
                if let ExternType::Function(f) = i.ty() {
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
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    /// Gets host imports signature
    #[method]
    fn get_host_imports(&self) -> Option<Dictionary> {
        match self.get_data().and_then(|m| {
            let ret = Dictionary::new();
            let params_str = GodotString::from_str("params");
            let results_str = GodotString::from_str("results");
            for i in m.module.imports() {
                if i.module() != HOST_MODULE {
                    continue;
                }
                if let ExternType::Function(f) = i.ty() {
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
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn has_function(&self, name: String) -> bool {
        match self.get_data().and_then(|m| {
            Ok(m.exports
                .iter()
                .any(|v| matches!(v.ty(), ExternType::Function(_)) && v.name() == name))
        }) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn get_signature(&self, name: String) -> Option<Dictionary> {
        match self.get_data().and_then(|m| {
            let f = match m
                .exports
                .iter()
                .filter_map(|v| {
                    if let ExternType::Function(f) = v.ty() {
                        if v.name() == name {
                            return Some(f);
                        }
                    }
                    None
                })
                .next()
            {
                Some(v) => v,
                None => bail!("No function named {}", name),
            };
            let (p, r) = from_signature(f)?;
            Ok(Dictionary::from_iter([("params", p), ("results", r)].into_iter()).into_shared())
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    // Instantiate module
    #[method]
    fn instantiate(
        &self,
        #[base] owner: TRef<Reference>,
        #[opt] host: Option<Dictionary>,
    ) -> Option<Instance<WasmInstance, Shared>> {
        let inst = WasmInstance::new_instance();
        if let Ok(true) = inst.map(|v, _| {
            if let Some(i) = Instance::from_base(owner.claim()) {
                v.initialize_(i, host)
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
