use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display, Error as FmtError, Formatter};
use std::path::PathBuf;
#[cfg(feature = "epoch-timeout")]
use std::{thread, time};

use anyhow::{bail, Result as AnyResult};
use cfg_if::cfg_if;
use godot::classes::FileAccess;
use godot::prelude::*;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
#[cfg(feature = "component-model")]
use wasmtime::component::Component;
use wasmtime::{Config, Engine, ExternType, Module, Precompiled, ResourcesRequired};

use crate::godot_util::{from_var_any, variant_to_option, PhantomProperty};
use crate::wasm_instance::WasmInstance;
use crate::wasm_util::from_signature;
#[cfg(feature = "epoch-timeout")]
use crate::wasm_util::EPOCH_INTERVAL;
use crate::{bail_with_site, site_context, variant_dispatch};

cfg_if! {
    if #[cfg(feature = "epoch-timeout")] {
        type EngineData = (Engine, Option<thread::JoinHandle<()>>);
    } else {
        type EngineData = Engine;
    }
}

static ENGINE: RwLock<Option<EngineData>> = RwLock::new(None);

pub fn get_engine() -> Result<Engine, EngineUninitError> {
    cfg_if! {
        if #[cfg(feature = "epoch-timeout")] {
            ENGINE.read().as_ref().map(|(e, _)| e.clone()).ok_or(EngineUninitError)
        } else {
            ENGINE.read().clone().ok_or(EngineUninitError)
        }
    }
}

pub fn init_engine() {
    let mut guard = ENGINE.write();
    if guard.is_none() {
        eprintln!("Initializing godot-wasm engine");
        let mut config = Config::new();
        config
            .cranelift_opt_level(wasmtime::OptLevel::Speed)
            .cranelift_nan_canonicalization(cfg!(feature = "deterministic-wasm"))
            .epoch_interruption(true)
            .debug_info(true)
            .wasm_reference_types(true)
            .wasm_function_references(true)
            .wasm_gc(true)
            .wasm_simd(true)
            .wasm_relaxed_simd(true)
            .relaxed_simd_deterministic(cfg!(feature = "deterministic-wasm"))
            .wasm_tail_call(true)
            .wasm_bulk_memory(true)
            .wasm_multi_value(true)
            .wasm_multi_memory(true)
            .wasm_memory64(true)
            .wasm_custom_page_sizes(true);
        #[cfg(feature = "wasm-threads")]
        config.wasm_threads(true);
        #[cfg(feature = "component-model")]
        config.wasm_component_model(true);

        let e = Engine::new(&config).unwrap();
        cfg_if! {
            if #[cfg(feature = "epoch-timeout")] {
                *guard = Some((e, None));
            } else {
                *guard = Some(e);
            }
        }
    }
}

pub fn deinit_engine() {
    eprintln!("Deinitializing godot-wasm engine");
    cfg_if! {
        if #[cfg(feature = "epoch-timeout")] {
            if let Some((engine, Some(handle))) = ENGINE.write().take() {
                // Make sure epoch will time out.
                for _ in 0..100 {
                    engine.increment_epoch();
                }
                drop(engine);
                handle.join().unwrap();
            }
        } else {
            *ENGINE.write() = None;
        }
    }
}

#[cfg(feature = "epoch-timeout")]
pub fn start_epoch() -> Result<(), EngineUninitError> {
    let mut guard = ENGINE.write();
    let (_, handle) = guard.as_mut().ok_or(EngineUninitError)?;
    if handle.is_none() {
        *handle = Some(thread::spawn(|| {
            let mut timeout = time::Instant::now();
            loop {
                thread::sleep(
                    (timeout + EPOCH_INTERVAL).saturating_duration_since(time::Instant::now()),
                );
                let Some(guard) = ENGINE.try_read() else {
                    break;
                };
                let Some((engine, _)) = guard.as_ref() else {
                    break;
                };
                let t = time::Instant::now();
                while timeout < t {
                    engine.increment_epoch();
                    timeout += EPOCH_INTERVAL;
                }
            }
        }));
    }
    Ok(())
}

pub struct EngineUninitError;

impl Debug for EngineUninitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        write!(f, "engine is not yet initialized")
    }
}

impl Display for EngineUninitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        <Self as Debug>::fmt(self, f)
    }
}

impl Error for EngineUninitError {}

#[derive(GodotClass)]
#[class(base=Resource, init, tool)]
/// Class for WebAssembly module.
///
/// This class load and compile WebAssembly binary into memory.
/// You only need to do it once since instantiation is very cheap.
/// It inherits `Resource` and can be (de)serialized.
/// **However,** be careful deserializing from untrusted source as compiled module
/// has _minimal_ validation and can be **exploited** to run arbitrary code.
///
/// It is also a `tool` class, allowing editor code can use WebAssembly too.
///
/// 📌 Use `initialize()` to properly initialize object.
/// **Uninitialized object should not be used.**
/// ```
pub struct WasmModule {
    base: Base<Resource>,
    data: OnceCell<ModuleData>,

    /// Property is `true` if module is a core module.
    #[var(get = get_is_core_module, usage_flags = [EDITOR, READ_ONLY])]
    #[allow(dead_code)]
    is_core_module: PhantomProperty<bool>,

    /// Property is `true` if module is a component.
    #[var(get = get_is_component, usage_flags = [EDITOR, READ_ONLY])]
    #[allow(dead_code)]
    is_component: PhantomProperty<bool>,

    /// Name of the module, if any.
    #[var(get = get_name, usage_flags = [EDITOR, READ_ONLY])]
    #[allow(dead_code)]
    name: PhantomProperty<GString>,

    /// **⚠ THIS PROPERTY IS HIDDEN AND SHOULD NOT BE USED DIRECTLY**
    ///
    /// Serialized module data. It is only used for storage only and should not be used in code.
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
    #[cfg(feature = "component-model")]
    Component(Component),
}

impl ModuleType {
    pub fn get_core(&self) -> AnyResult<&Module> {
        #[allow(irrefutable_let_patterns)]
        if let Self::Core(m) = self {
            Ok(m)
        } else {
            bail!("Module is not a core module")
        }
    }

    #[cfg(feature = "component-model")]
    pub fn get_component(&self) -> AnyResult<&Component> {
        if let Self::Component(m) = self {
            Ok(m)
        } else {
            bail!("Module is not a component")
        }
    }
}

impl WasmModule {
    pub fn get_data(&self) -> AnyResult<&ModuleData> {
        if let Some(data) = self.data.get() {
            Ok(data)
        } else {
            bail_with_site!("Uninitialized module")
        }
    }

    pub fn unwrap_data<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&ModuleData) -> AnyResult<R>,
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

    fn load_module(bytes: &[u8]) -> AnyResult<ModuleType> {
        cfg_if! {
            if #[cfg(feature = "component-model")] {
                let bytes = site_context!(wat::parse_bytes(bytes))?;
                if wasmparser::Parser::is_component(&bytes) {
                    Ok(ModuleType::Component(site_context!(
                        Component::from_binary(&get_engine()?, &bytes,)
                    )?))
                } else {
                    Ok(ModuleType::Core(site_context!(Module::from_binary(
                        &get_engine()?, &bytes
                    ))?))
                }
            } else {
                Ok(ModuleType::Core(site_context!(Module::new(
                    &get_engine()?, bytes
                ))?))
            }
        }
    }

    fn process_deps_map(
        module: &ModuleType,
        imports: Option<Dictionary>,
    ) -> AnyResult<HashMap<String, Gd<WasmModule>>> {
        let mut deps_map = HashMap::new();
        let Some(imports) = imports else {
            return Ok(deps_map);
        };
        #[allow(irrefutable_let_patterns)]
        if let ModuleType::Core(_module) = &module {
            deps_map = imports
                .iter_shared()
                .map(|(k, v)| -> AnyResult<_> {
                    Ok((
                        site_context!(from_var_any::<String>(k))?,
                        site_context!(from_var_any::<Gd<WasmModule>>(v))?,
                    ))
                })
                .collect::<AnyResult<_>>()?;
        }
        #[cfg(feature = "component-model")]
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
        let r = self.data.get_or_try_init(move || -> AnyResult<_> {
            let module = variant_dispatch!(data {
                PACKED_BYTE_ARRAY => Self::load_module(data.as_slice())?,
                STRING => Self::load_module(data.to_string().as_bytes())?,
                OBJECT => match data
                    .try_cast::<FileAccess>()
                    .map_err(|v| v.try_cast::<WasmModule>())
                {
                    Ok(v) => Self::load_module(v.get_buffer(v.get_length() as _).as_slice())?,
                    Err(Ok(v)) => v.bind().get_data()?.module.clone(),
                    Err(Err(v)) => bail_with_site!("Unknown module value {}", v),
                },
                _ => bail_with_site!("Unknown module value {}", data),
            });

            let imports = Self::process_deps_map(&module, imports)?;

            Ok(ModuleData {
                name: Self::name_from_module(&module),
                module,
                imports,
            })
        });
        if let Err(e) = r {
            godot_error!("{:?}", e);
            false
        } else {
            true
        }
    }

    fn _deserialize(&self, data: PackedByteArray, imports: Option<Dictionary>) -> bool {
        let r = self.data.get_or_try_init(move || -> AnyResult<_> {
            let engine = site_context!(get_engine())?;
            let data = data.as_slice();
            // SAFETY: Assume the supplied data is safe to deserialize.
            let module = unsafe {
                match engine.detect_precompiled(data) {
                    Some(Precompiled::Module) => {
                        ModuleType::Core(site_context!(Module::deserialize(&engine, data))?)
                    }
                    #[cfg(feature = "component-model")]
                    Some(Precompiled::Component) => {
                        ModuleType::Component(site_context!(Component::deserialize(&engine, data))?)
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
        });
        if let Err(e) = r {
            godot_error!("{:?}", e);
            false
        } else {
            true
        }
    }

    fn _deserialize_file(&self, path: String, imports: Option<Dictionary>) -> bool {
        let r = self.data.get_or_try_init(move || -> AnyResult<_> {
            let engine = site_context!(get_engine())?;
            let path = PathBuf::from(path);
            // SAFETY: Assume the supplied file is safe to deserialize.
            let module = unsafe {
                match site_context!(engine.detect_precompiled_file(&path))? {
                    Some(Precompiled::Module) => {
                        ModuleType::Core(site_context!(Module::deserialize_file(&engine, path))?)
                    }
                    #[cfg(feature = "component-model")]
                    Some(Precompiled::Component) => ModuleType::Component(site_context!(
                        Component::deserialize_file(&engine, path)
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
        });
        if let Err(e) = r {
            godot_error!("{:?}", e);
            false
        } else {
            true
        }
    }
}

#[godot_api]
impl WasmModule {
    /// Initialize and loads module.
    ///
    /// **⚠ MUST BE CALLED FOR THE FIRST TIME AND ONLY ONCE.**
    ///
    /// Returns itself if succeed, `null` otherwise.
    ///
    /// Arguments:
    /// - `data` : Can be one of these things:
    ///   - `PackedByteArray` containing WASM binary or WAT text data.
    ///   - `String` containing WAT text data.
    ///   - `FileAccess` with WASM file open.
    ///   - `WasmModule` (for cloning without recompiling).
    /// - `import` : Maps name to other `WasmModule` to used as imports. Currently does not work with component.
    ///
    /// Usage:
    /// ```
    /// var module := WasmModule.new().initialize("...", {})
    ///
    /// if module == null:
    ///   # Cannot compile module
    ///   pass
    /// ```
    #[func]
    fn initialize(&self, data: Variant, imports: Dictionary) -> Option<Gd<WasmModule>> {
        if self._initialize(data, Some(imports)) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    /// Gets the module name, if exists.
    #[func]
    fn get_name(&self) -> GString {
        self.unwrap_data(|m| Ok(m.name.clone())).unwrap_or_default()
    }

    /// Returns `true` if module is a core module.
    #[func]
    fn get_is_core_module(&self) -> bool {
        self.unwrap_data(|m| Ok(matches!(m.module, ModuleType::Core(_))))
            .unwrap_or_default()
    }

    /// Returns `true` if module is a component.
    #[func]
    fn get_is_component(&self) -> bool {
        cfg_if! {
            if #[cfg(feature = "wasi-preview2")] {
                self.unwrap_data(|m| Ok(matches!(m.module, ModuleType::Component(_)))).unwrap_or_default()
            } else {
                false
            }
        }
    }

    /// Gets all the module it imported.
    #[func]
    fn get_imported_modules(&self) -> VariantArray {
        self.unwrap_data(|m| {
            Ok(VariantArray::from_iter(
                m.imports.values().map(|v| v.clone().to_variant()),
            ))
        })
        .unwrap_or_default()
    }

    /// Deserialize compiled module data.
    ///
    /// **⚠ DO NOT USE THIS WITH UNTRUSTED DATA**
    #[func]
    fn deserialize(&self, data: PackedByteArray, imports: Dictionary) -> Option<Gd<WasmModule>> {
        if self._deserialize(data, Some(imports)) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    /// Deserialize file containing compiled module data.
    ///
    /// **⚠ DO NOT USE THIS WITH UNTRUSTED DATA**
    #[func]
    fn deserialize_file(&self, path: GString, imports: Dictionary) -> Option<Gd<WasmModule>> {
        if self._deserialize_file(path.to_string(), Some(imports)) {
            Some(self.to_gd())
        } else {
            None
        }
    }

    /// Deserialize compiled module data.
    ///
    /// **⚠ DO NOT USE THIS WITH UNTRUSTED DATA**
    #[func]
    fn deserialize_bytes(&self, data: PackedByteArray) {
        self._deserialize(data, None);
    }

    /// Serialize compiled module data.
    ///
    /// Serialized data is very fast to load as no compilation step is needed.
    /// But the data can only be deserialized with the same godot-wasm binary.
    /// Whenever you upgrade module, make sure to reimport all `WasmModule`.
    #[func]
    fn serialize(&self) -> PackedByteArray {
        self.unwrap_data(|m| {
            self._bytes_data.get_or_try_init(|| {
                Ok(PackedByteArray::from(
                    &match &m.module {
                        ModuleType::Core(m) => m.serialize(),
                        #[cfg(feature = "component-model")]
                        ModuleType::Component(m) => m.serialize(),
                    }?[..],
                ))
            })
        })
        .cloned()
        .unwrap_or_default()
    }

    /// Gets exported functions.
    ///
    /// The resulting dictionary is the function name as it's key,
    /// with the value is a struct of the following:
    /// - `params` : Array of parameter types.
    /// - `results` : Array of result types.
    #[func]
    fn get_exports(&self) -> Dictionary {
        self.unwrap_data(|m| {
            let mut ret = Dictionary::new();
            let params_str = StringName::from(c"params");
            let results_str = StringName::from(c"results");
            for i in site_context!(m.module.get_core())?.exports() {
                if let ExternType::Func(f) = i.ty() {
                    let (p, r) = from_signature(&f);
                    ret.set(
                        i.name(),
                        [(params_str.clone(), p), (results_str.clone(), r)]
                            .into_iter()
                            .collect::<Dictionary>(),
                    );
                }
            }
            Ok(ret)
        })
        .unwrap_or_default()
    }

    /// Gets host imports signature.
    ///
    /// The resulting value is a mapping of module name, then function names,
    /// of the following struct:
    /// - `params` : Array of parameter types.
    /// - `results` : Array of result types.
    #[func]
    fn get_host_imports(&self) -> Dictionary {
        self.unwrap_data(|m| {
            let mut ret = Dictionary::new();
            let params_str = StringName::from(c"params");
            let results_str = StringName::from(c"results");

            for i in site_context!(m.module.get_core())?.imports() {
                let ExternType::Func(f) = i.ty() else {
                    continue;
                };

                if let Some(m) = m.imports.get(i.module()) {
                    match &m.bind().get_data()?.module {
                        ModuleType::Core(m) if m.get_export(i.name()).is_some() => continue,
                        #[cfg(feature = "component-model")]
                        ModuleType::Component(_) => {
                            bail_with_site!("Import {} is a component", i.module())
                        }
                        _ => (),
                    }
                }

                let (p, r) = from_signature(&f);
                let mut v = match ret.get(i.module()) {
                    Some(v) => Dictionary::from_variant(&v),
                    None => {
                        let v = Dictionary::new();
                        ret.set(i.module(), v.clone());
                        v
                    }
                };
                v.set(
                    i.name(),
                    [(params_str.clone(), p), (results_str.clone(), r)]
                        .into_iter()
                        .collect::<Dictionary>(),
                );
            }

            Ok(ret)
        })
        .unwrap_or_default()
    }

    /// Returns `true` if exported function extsts.
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

    /// Gets the signature of exported function.
    #[func]
    fn get_signature(&self, name: StringName) -> Dictionary {
        self.unwrap_data(|m| {
            let Some(ExternType::Func(f)) =
                site_context!(m.module.get_core())?.get_export(&name.to_string())
            else {
                bail_with_site!("No function named {}", name);
            };

            let (p, r) = from_signature(&f);
            Ok([
                (StringName::from(c"params"), p),
                (StringName::from(c"results"), r),
            ]
            .into_iter()
            .collect())
        })
        .unwrap_or_default()
    }

    /// Gets statistics about memories and tables required to instantiate this module (without imports).
    ///
    /// You can use this for minimal checks against resource exhaustion.
    #[func]
    fn get_resources_required(&self) -> Variant {
        self.unwrap_data(|m| {
            let v = match &m.module {
                ModuleType::Core(m) => Some(m.resources_required()),
                #[cfg(feature = "component-model")]
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

    /// Gets statistics about memories and tables required to instantiate this module.
    ///
    /// You can use this for minimal checks against resource exhaustion.
    #[func]
    fn get_total_resources_required(&self) -> Variant {
        fn f(module: &ModuleData) -> Option<ResourcesRequired> {
            match &module.module {
                ModuleType::Core(m) => Some(m.resources_required()),
                #[cfg(feature = "component-model")]
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

    /// Instantiate module.
    ///
    /// See `WasmInstance.initialize` for more info.
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
