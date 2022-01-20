use std::iter::FromIterator;
use std::mem::transmute;

use anyhow::bail;
use gdnative::prelude::*;
use hashbrown::{hash_map::Entry, HashMap};
use parking_lot::Once;
use wasmtime::{Config, Engine, ExternType, Module};

use crate::thisobj::{node::THISOBJ_NODE, node2d::THISOBJ_NODE2D, object::THISOBJ_OBJECT};
use crate::wasm_externref_godot::GODOT_MODULE;
use crate::wasm_store::{from_signature, HOST_MODULE};
use crate::{TYPE_F32, TYPE_F64, TYPE_I32, TYPE_I64, TYPE_VARIANT};

const MODULE_INCLUDES: &[&str] = &[
    HOST_MODULE,
    GODOT_MODULE,
    THISOBJ_OBJECT,
    THISOBJ_NODE,
    THISOBJ_NODE2D,
];

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::nativescript::user_data::ArcData<WasmEngine>)]
pub struct WasmEngine {
    pub(crate) engine: Engine,
}

impl WasmEngine {
    /// Create new WasmEngine
    fn new(_owner: &Reference) -> Self {
        // Create new configuration with:
        // - Async disabled
        // - Fuel consumption disabled
        // - Only dynamic memory
        // - No guard region
        // - Reference Type proposal enabled
        let mut config = Config::new();
        config
            //.async_support(false)
            .consume_fuel(false)
            .wasm_reference_types(true)
            .static_memory_maximum_size(0)
            .dynamic_memory_guard_size(0);
        Self {
            engine: Engine::new(&config).expect("Cannot create engine"),
        }
    }
}

// Godot exported methods
#[methods]
impl WasmEngine {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .add_property::<u32>("TYPE_I32")
            .with_getter(|_, _| TYPE_I32)
            .done();
        builder
            .add_property::<u32>("TYPE_I64")
            .with_getter(|_, _| TYPE_I64)
            .done();
        builder
            .add_property::<u32>("TYPE_F32")
            .with_getter(|_, _| TYPE_F32)
            .done();
        builder
            .add_property::<u32>("TYPE_F64")
            .with_getter(|_, _| TYPE_F64)
            .done();
        builder
            .add_property::<u32>("TYPE_VARIANT")
            .with_getter(|_, _| TYPE_VARIANT)
            .done();
    }
}

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::nativescript::user_data::ArcData<WasmModule>)]
pub struct WasmModule {
    once: Once,
    pub(crate) data: Option<ModuleData>,
}

pub struct ModuleData {
    pub(crate) engine: Instance<WasmEngine, Shared>,
    pub(crate) module: Module,
    pub(crate) deps: Vec<Instance<WasmModule, Shared>>,
}

impl WasmModule {
    fn new(_owner: &Reference) -> Self {
        Self {
            once: Once::new(),
            data: None,
        }
    }
}

#[methods]
impl WasmModule {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .add_property::<Option<Instance<WasmEngine, Shared>>>("engine")
            .with_getter(|v, _| match v.data.as_ref() {
                Some(ModuleData { engine, .. }) => Some(engine.clone()),
                None => None,
            })
            .done();
    }

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[export]
    fn initialize(
        &self,
        owner: TRef<Reference>,
        engine: Instance<WasmEngine, Shared>,
        name: String,
        data: Variant,
        imports: VariantArray,
    ) -> Option<Ref<Reference>> {
        let mut r = true;
        let ret = &mut r;

        self.once.call_once(move || {
            // SAFETY: Engine is assumed to be a valid WasmEngine
            let e = unsafe { engine.assume_safe() };
            let module = e.map(|engine, _| {
                if let Ok(m) = ByteArray::from_variant(&data) {
                    Module::new_with_name(&engine.engine, &*m.read(), &name)
                } else if let Ok(m) = String::from_variant(&data) {
                    Module::new_with_name(&engine.engine, &m, &name)
                } else {
                    bail!("Module type is not string nor byte array");
                }
            });

            let module = match module {
                Ok(Ok(m)) => m,
                Ok(Err(e)) => {
                    godot_error!("{}", e);
                    *ret = false;
                    return;
                }
                Err(e) => {
                    godot_error!("{}", e);
                    *ret = false;
                    return;
                }
            };

            // SAFETY: Imports is assumed to be unique
            let imports = unsafe { imports.assume_unique() };

            let mut deps = Vec::with_capacity(imports.len() as usize);

            for m in imports.iter() {
                deps.push(match <Instance<WasmModule, Shared>>::from_variant(&m) {
                    Ok(m) => m,
                    Err(e) => {
                        godot_error!("{}", e);
                        *ret = false;
                        return;
                    }
                });
            }

            {
                let mut dname = HashMap::with_capacity(deps.len());
                for m in deps.iter() {
                    // SAFETY: m is assumed to be valid WasmModule
                    if unsafe {
                        m.assume_safe()
                            .map(|m, _| {
                                let m = &m.data.as_ref().expect("Uninitialized!").module;
                                // SAFETY: deps will outlast dname
                                match dname.entry(transmute::<&str, &str>(
                                    m.name().expect("Unnamed module"),
                                )) {
                                    entry @ Entry::Vacant(_) => {
                                        entry.or_insert(transmute::<&Module, &Module>(m));
                                        false
                                    }
                                    Entry::Occupied(e) => {
                                        godot_error!("Duplicate module name {}", e.key());
                                        true
                                    }
                                }
                            })
                            .unwrap_or(true)
                    } {
                        *ret = false;
                        return;
                    }
                }

                for i in module.imports() {
                    if MODULE_INCLUDES.contains(&i.module()) {
                        continue;
                    }
                    let m = match dname.get(i.module()) {
                        Some(m) => m,
                        None => {
                            godot_error!("Unknown imported module {}", i.module());
                            *ret = false;
                            return;
                        }
                    };
                    let j = match m.get_export(i.name().expect("Unnamed item")) {
                        Some(m) => m,
                        None => {
                            godot_error!(
                                "Unknown imported item {} in {}",
                                i.name().unwrap_or(""),
                                i.module()
                            );
                            *ret = false;
                            return;
                        }
                    };
                    match (i.ty(), j) {
                        (ExternType::Func(a), ExternType::Func(b)) if a == b => (),
                        (ExternType::Global(a), ExternType::Global(b)) if a == b => (),
                        (ExternType::Table(a), ExternType::Table(b)) if a == b => (),
                        (ExternType::Memory(a), ExternType::Memory(b)) if a == b => (),
                        _ => {
                            godot_error!(
                                "Imported item type mismatch! ({} in {})",
                                i.name().unwrap_or(""),
                                i.module()
                            );
                            *ret = false;
                            return;
                        }
                    };
                }
            }

            // SAFETY: Should be called only once
            #[allow(mutable_transmutes)]
            let this = unsafe { transmute::<&Self, &mut Self>(self) };
            this.data = Some(ModuleData {
                engine,
                module,
                deps,
            });
        });

        if r {
            Some(owner.claim())
        } else {
            None
        }
    }

    /// Gets exported functions
    #[export]
    fn get_exports(&self, _owner: &Reference) -> Variant {
        match self.data.as_ref() {
            Some(m) => VariantArray::from_iter(m.module.exports().filter_map(|v| {
                if matches!(v.ty(), ExternType::Func(_)) {
                    Some(GodotString::from(v.name()).to_variant())
                } else {
                    None
                }
            }))
            .owned_to_variant(),
            None => {
                godot_error!("Uninitialized!");
                Variant::new()
            }
        }
    }

    /// Gets host imports signature
    #[export]
    fn get_host_imports(&self, _owner: &Reference) -> Variant {
        let m = match self.data.as_ref() {
            Some(ModuleData { module, .. }) => module,
            None => {
                godot_error!("Uninitialized!");
                return Variant::new();
            }
        };

        Dictionary::from_iter(m.exports().filter_map(|v| {
            if let ExternType::Func(f) = v.ty() {
                match from_signature(f) {
                    Ok((p, r)) => {
                        let d = Dictionary::new();
                        d.insert(GodotString::from_str("params"), p);
                        d.insert(GodotString::from_str("results"), r);
                        Some((GodotString::from_str(v.name()), d.owned_to_variant()))
                    }
                    Err(e) => {
                        godot_error!("{}", e);
                        Some((GodotString::from_str(v.name()), Variant::new()))
                    }
                }
            } else {
                None
            }
        }))
        .owned_to_variant()
    }
}
