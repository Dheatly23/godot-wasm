use std::cell::UnsafeCell;
use std::hint::unreachable_unchecked;
use std::iter::FromIterator;
use std::mem::transmute;

use anyhow::bail;
use gdnative::prelude::*;
use hashbrown::{hash_map::Entry, HashMap};
use parking_lot::Once;
use wasmtime::{Config, Engine, ExternType, Module};

use crate::thisobj::{node::THISOBJ_NODE, node2d::THISOBJ_NODE2D, object::THISOBJ_OBJECT};
use crate::wasm_externref_godot::register_godot_externref;
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub(crate) enum LinkerCacheIndex {
    #[allow(dead_code)]
    Default = 0,
    Object,
    Reference,
    Node,
    Node2D,
    End,
}

type LinkerType = wasmtime::Linker<crate::thisobj::StoreData>;

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::nativescript::user_data::ArcData<WasmEngine>)]
pub struct WasmEngine {
    pub(crate) engine: Engine,
    linker_cache: (Vec<Once>, Vec<UnsafeCell<Option<Box<LinkerType>>>>),
}

unsafe impl Sync for WasmEngine {}

impl WasmEngine {
    /// Create new WasmEngine
    #[profiled]
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
        let len = LinkerCacheIndex::End as usize;
        let mut ret = Self {
            engine: Engine::new(&config).expect("Cannot create engine"),
            linker_cache: (Vec::new(), Vec::new()),
        };
        ret.linker_cache.0.resize_with(len, Once::default);
        ret.linker_cache.1.resize_with(len, UnsafeCell::default);
        unsafe {
            let ptr = ret.linker_cache.1[0].get();
            ret.linker_cache.0[0].call_once(|| {
                let mut linker = LinkerType::new(&ret.engine);
                register_godot_externref(&mut linker).unwrap();
                *ptr = Some(Box::new(linker));
            });
        }
        ret
    }

    pub(crate) fn get_default_linker_cache(&self) -> LinkerType {
        unsafe { (**(*self.linker_cache.1[0].get()).as_ref().unwrap()).clone() }
    }

    pub(crate) fn get_linker_cache(
        &self,
        index: LinkerCacheIndex,
        f: impl FnOnce() -> LinkerType,
    ) -> LinkerType {
        let index = match index {
            LinkerCacheIndex::Default => unsafe {
                return (**(*self.linker_cache.1[0].get()).as_ref().unwrap()).clone();
            },
            LinkerCacheIndex::End => panic!("Out of bound!"),
            i => i as usize,
        };
        let ptr = self.linker_cache.1[index].get();
        unsafe {
            let ptr = ptr;
            self.linker_cache.0[index].call_once(move || *ptr = Some(Box::new(f())));
        }
        unsafe {
            match (*ptr).as_ref() {
                Some(l) => (**l).clone(),
                None => unreachable_unchecked(),
            }
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

    #[export]
    #[profiled]
    fn create_module(
        &self,
        owner: TRef<Reference>,
        name: String,
        data: Variant,
        imports: VariantArray,
    ) -> Option<Instance<WasmModule, Shared>> {
        let ret = WasmModule {
            once: Once::new(),
            data: None,
        };
        if ret._initialize(owner.cast_instance().unwrap().claim(), name, data, imports) {
            Some(Instance::emplace(ret).into_shared())
        } else {
            None
        }
    }

    #[export]
    #[profiled]
    fn create_modules(
        &self,
        owner: TRef<Reference>,
        modules: VariantArray,
        imports: VariantArray,
    ) -> Option<VariantArray> {
        #[derive(FromVariant)]
        struct ModuleInput {
            name: String,
            data: Variant,
        }

        #[derive(Clone, Copy, PartialEq, Eq)]
        enum DepMark {
            Unmarked,
            TempMarked,
            PermMarked(usize),
        }

        // modules gets mutated in check_recursive
        #[allow(unused_mut)]
        let mut modules = {
            let m = unsafe { modules.assume_unique() };
            let mut r = HashMap::with_capacity(m.len() as usize);
            for i in m.iter() {
                match ModuleInput::from_variant(&i) {
                    Ok(ModuleInput { name, data }) => {
                        let data = if let Ok(m) = ByteArray::from_variant(&data) {
                            Module::new_with_name(&self.engine, &*m.read(), &name)
                        } else if let Ok(m) = String::from_variant(&data) {
                            Module::new_with_name(&self.engine, &m, &name)
                        } else {
                            godot_error!("Module type is not string nor byte array");
                            return None;
                        };
                        let data = match data {
                            Ok(d) => d,
                            Err(e) => {
                                godot_error!("{}", e);
                                return None;
                            }
                        };
                        match r.entry(name) {
                            Entry::Vacant(e) => {
                                e.insert((data, DepMark::Unmarked));
                            }
                            Entry::Occupied(e) => {
                                godot_error!("Duplicate module name {}", e.key());
                                return None;
                            }
                        }
                    }
                    Err(e) => {
                        godot_error!("{}", e);
                        return None;
                    }
                }
            }
            r
        };

        let imports = {
            let m = unsafe { imports.assume_unique() };
            let mut r = HashMap::with_capacity(m.len() as usize);
            for i in m.iter() {
                match <Instance<WasmModule, Shared>>::from_variant(&i) {
                    Ok(i) => {
                        // SAFETY: Import should be safe to get
                        match unsafe {
                            i.assume_safe().map(|i, _| {
                                let m = &i.data.as_ref().expect("Uninitialized!").module;
                                // SAFETY: Key lives shorter than value (?)
                                (
                                    transmute::<&str, &str>(m.name().expect("Unnamed module")),
                                    transmute::<&Module, &Module>(m),
                                )
                            })
                        } {
                            Ok((k, m)) => match r.entry(k) {
                                Entry::Vacant(e) => {
                                    e.insert((m, i));
                                }
                                Entry::Occupied(e) => {
                                    godot_error!("Duplicate module name {}", e.key());
                                    return None;
                                }
                            },
                            Err(e) => {
                                godot_error!("{}", e);
                                return None;
                            }
                        }
                    }
                    Err(e) => {
                        godot_error!("{}", e);
                        return None;
                    }
                }
            }
            r
        };

        fn check_recursive(
            engine: &Instance<WasmEngine, Shared>,
            (m, mark): &(Module, DepMark),
            modules: &HashMap<String, (Module, DepMark)>,
            imports: &HashMap<&str, (&Module, Instance<WasmModule, Shared>)>,
            module_list: &mut Vec<Instance<WasmModule, Shared>>,
        ) -> Option<usize> {
            // SAFETY: Nobody else holds mark at the moment
            #[allow(mutable_transmutes)]
            unsafe {
                *transmute::<&DepMark, &mut DepMark>(mark) = DepMark::TempMarked;
            }
            let mut deps = HashMap::new();
            for i in m.imports() {
                if MODULE_INCLUDES.contains(&i.module()) {
                    continue;
                }
                let j = match imports.get_key_value(i.module()) {
                    Some((k, (m, inst))) => {
                        deps.insert(*k, inst.clone());
                        match m.get_export(i.name().expect("Unnamed item")) {
                            Some(e) => e,
                            None => {
                                godot_error!("No export named {} in {}", i.name().unwrap(), k);
                                return None;
                            }
                        }
                    }
                    None => match modules.get_key_value(i.module()) {
                        Some((k, v)) => {
                            let ix = match v.1 {
                                DepMark::TempMarked => {
                                    godot_error!("Detected cycle involving {}", k);
                                    return None;
                                }
                                DepMark::PermMarked(ix) => ix,
                                DepMark::Unmarked => match check_recursive(
                                    engine,
                                    v,
                                    modules,
                                    imports,
                                    &mut *module_list,
                                ) {
                                    Some(ix) => ix,
                                    None => return None,
                                },
                            };
                            deps.insert(&**k, module_list[ix].clone());
                            match v.0.get_export(i.name().expect("Unnamed item")) {
                                Some(e) => e,
                                None => {
                                    godot_error!("No export named {} in {}", i.name().unwrap(), k);
                                    return None;
                                }
                            }
                        }
                        None => {
                            godot_error!("No module named {}", i.module());
                            return None;
                        }
                    },
                };
                if !cmp_extern_type(&i.ty(), &j) {
                    return None;
                }
            }
            let ix = module_list.len();
            // SAFETY: Nobody else holds mark at the moment
            #[allow(mutable_transmutes)]
            unsafe {
                *transmute::<&DepMark, &mut DepMark>(mark) = DepMark::PermMarked(ix);
            }
            module_list.push(
                Instance::emplace({
                    let once = Once::new();
                    once.call_once(|| ());
                    WasmModule {
                        once,
                        data: Some(ModuleData {
                            engine: engine.clone(),
                            module: m.clone(),
                            deps: deps.into_iter().map(|(_, v)| v).collect(),
                        }),
                    }
                })
                .into_shared(),
            );
            Some(ix)
        }

        let mut module_list = Vec::with_capacity(modules.len());

        while let Some((_, v)) = modules
            .iter()
            .filter(|(_, (_, mark))| mark == &DepMark::Unmarked)
            .next()
        {
            if let None = check_recursive(
                &owner.cast_instance().unwrap().claim(),
                v,
                &modules,
                &imports,
                &mut module_list,
            ) {
                return None;
            }
        }

        Some(VariantArray::from_iter(module_list.into_iter().map(|v| v.to_variant())).into_shared())
    }
}

fn cmp_extern_type(t1: &ExternType, t2: &ExternType) -> bool {
    match (t1, t2) {
        (ExternType::Func(a), ExternType::Func(b)) => a == b,
        (ExternType::Global(a), ExternType::Global(b)) => a == b,
        (ExternType::Table(a), ExternType::Table(b)) => a == b,
        (ExternType::Memory(a), ExternType::Memory(b)) => a == b,
        _ => false,
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

    fn _initialize(
        &self,
        engine: Instance<WasmEngine, Shared>,
        name: String,
        data: Variant,
        imports: VariantArray,
    ) -> bool {
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
                                    Entry::Vacant(e) => {
                                        e.insert(transmute::<&Module, &Module>(m));
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
                    if cmp_extern_type(&i.ty(), &j) {
                        godot_error!(
                            "Imported item type mismatch! ({} in {})",
                            i.name().unwrap_or(""),
                            i.module()
                        );
                        *ret = false;
                        return;
                    }
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

        r
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

        builder
            .add_property::<Option<GodotString>>("name")
            .with_getter(|v, _| match v.data.as_ref() {
                Some(ModuleData { module, .. }) => match module.name() {
                    Some(n) => Some(GodotString::from_str(n)),
                    None => None,
                },
                None => None,
            })
            .done();
    }

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[export]
    #[profiled]
    fn initialize(
        &self,
        owner: TRef<Reference>,
        engine: Instance<WasmEngine, Shared>,
        name: String,
        data: Variant,
        imports: VariantArray,
    ) -> Option<Ref<Reference>> {
        if self._initialize(engine, name, data, imports) {
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
