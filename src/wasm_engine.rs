use anyhow::{bail, Result};
use gdnative::prelude::*;
use hashbrown::HashSet;
use indexmap::IndexMap;
use parking_lot::RwLock;
use wasmtime::{Config, Engine, Module};

use crate::wasm_externref_godot::GODOT_MODULE;
use crate::wasm_store::HOST_MODULE;
use crate::{TYPE_F32, TYPE_F64, TYPE_I32, TYPE_I64, TYPE_VARIANT};

const MODULE_INCLUDES: [&str; 2] = [HOST_MODULE, GODOT_MODULE];

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::nativescript::user_data::ArcData<WasmEngine>)]
pub struct WasmEngine {
    pub(crate) engine: Engine,
    pub(crate) modules: RwLock<IndexMap<String, ModuleData>>,
}

pub struct ModuleData {
    pub(crate) module: Module,
    pub(crate) deps: Vec<usize>,
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
            modules: RwLock::new(IndexMap::new()),
        }
    }

    fn _load_module(&self, name: String, module: Module) -> Result<()> {
        let mut deps = HashSet::new();
        let mut modules = self.modules.write();
        for i in module.imports() {
            let name = i.module();
            if MODULE_INCLUDES.contains(&name) {
                continue; // Ignore host function(s)
            }
            match modules.get_full(name) {
                Some((ix, _, d)) => {
                    deps.insert(ix);
                    deps.extend(d.deps.iter());
                }
                None => bail!("Unknown module {}", name),
            }
        }
        let mut deps: Vec<_> = deps.drain().collect();
        deps.shrink_to_fit();
        deps.sort_unstable();
        modules.insert(name, ModuleData { module, deps });
        Ok(())
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

    /// Load a module
    #[export]
    fn load_module(&self, _owner: &Reference, name: String, path: String) -> u32 {
        #[inline(always)]
        fn f(this: &WasmEngine, name: String, path: String) -> Result<()> {
            this._load_module(name, Module::from_file(&this.engine, path)?)
        }
        match f(self, name, path) {
            Err(e) => {
                godot_error!("Load WASM module failed: {}", e);
                GodotError::Failed as u32
            }
            Ok(_) => 0,
        }
    }

    /// Load a WAT module
    #[export]
    fn load_module_wat(&self, _owner: &Reference, name: String, code: String) -> u32 {
        #[inline(always)]
        fn f(this: &WasmEngine, name: String, code: String) -> Result<()> {
            this._load_module(name, Module::new(&this.engine, &code)?)
        }
        match f(self, name, code) {
            Err(e) => {
                godot_error!("Load WASM module failed: {}", e);
                GodotError::Failed as u32
            }
            Ok(_) => 0,
        }
    }
}
