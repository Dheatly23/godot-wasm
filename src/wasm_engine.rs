use anyhow::{bail, Result};
use gdnative::prelude::*;
use hashbrown::{HashMap, HashSet};
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

    fn _load_modules(&self, modules: impl Iterator<Item = (String, Module)>) -> Result<()> {
        use std::mem::transmute;

        enum Marker {
            Unmarked,
            TempMarked,
            PermMarked(usize),
        }
        struct Data(ModuleData, Marker);

        let mut modules: HashMap<String, Data> = modules
            .map(|(k, v)| {
                (
                    k,
                    Data(
                        ModuleData {
                            module: v,
                            deps: Vec::new(),
                        },
                        Marker::Unmarked,
                    ),
                )
            })
            .collect();
        let mut orig = self.modules.write();

        fn f(
            orig: &IndexMap<String, ModuleData>,
            modules: &mut HashMap<String, Data>,
            k: &str,
            mut ix: usize,
        ) -> Result<usize> {
            // SAFETY: We never insert/delete modules,
            // so map items never gets moved
            let v = unsafe { transmute::<&mut Data, &mut Data>(modules.get_mut(k).unwrap()) };
            v.1 = Marker::TempMarked;

            let mut deps = HashSet::new();
            for i in v.0.module.imports() {
                let name = i.module();
                if MODULE_INCLUDES.contains(&name) {
                    continue; // Ignore host function(s)
                }
                match orig.get_full(name) {
                    Some((ix, _, d)) => {
                        deps.insert(ix);
                        deps.extend(d.deps.iter());
                    }
                    None => match modules.get(name) {
                        Some(Data(_, Marker::Unmarked)) => ix = f(&*orig, modules, name, ix)?,
                        None => bail!("Unknown module {}", name),
                        Some(Data(_, Marker::TempMarked)) => {
                            bail!("Detected cycle on module {}", name)
                        }
                        Some(Data(d, Marker::PermMarked(ix))) => {
                            deps.insert(*ix);
                            deps.extend(d.deps.iter());
                        }
                    },
                };
            }

            v.0.deps.extend(deps.drain());
            v.0.deps.shrink_to_fit();
            v.0.deps.sort_unstable();

            v.1 = Marker::PermMarked(ix);
            Ok(ix + 1)
        }

        let mut ix = orig.len();
        while let Some((k, _)) = modules
            .iter()
            .filter(|(_, Data(_, v))| matches!(v, Marker::Unmarked))
            .next()
        {
            // SAFETY: We never insert/delete modules,
            // so map items never gets moved
            let k = unsafe { transmute::<&str, &str>(k) };

            ix = f(&orig, &mut modules, k, ix)?;
        }

        let mut modules: Vec<_> = modules.drain().collect();
        modules.sort_unstable_by_key(|(_, Data(_, v))| match v {
            Marker::PermMarked(ix) => *ix,
            _ => unreachable!("Algorithm error, temp/unmarked module"),
        });

        for (k, Data(m, v)) in modules.into_iter() {
            match v {
                Marker::PermMarked(i) if i == orig.len() => (),
                _ => unreachable!("Algorithm error, value got skipped"),
            };
            orig.insert(k, m);
        }

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

    /// Load multiple modules
    #[export]
    fn load_modules(&self, _owner: &Reference, modules: Dictionary) -> u32 {
        match self._load_modules(modules.iter().map(|(k, v)| {
            let k = String::from_variant(&k).unwrap();
            let v = String::from_variant(&v).unwrap();

            let v = match Module::from_file(&self.engine, v) {
                Ok(v) => v,
                Err(e) => panic!("Load WASM module failed: {}", e),
            };

            (k, v)
        })) {
            Ok(()) => 0,
            Err(e) => {
                godot_error!("{}", e);
                GodotError::Failed as _
            }
        }
    }
}
