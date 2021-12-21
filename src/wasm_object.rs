use std::iter::FromIterator;

use anyhow::{bail, Result};
use gdnative::prelude::*;
use wasmtime::{Linker, Store};

use crate::wasm_engine::*;
use crate::wasm_externref_godot::*;
use crate::wasm_store::*;

type StoreData = (Instance<WasmEngine, Shared>,);

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::nativescript::user_data::MutexData<WasmObject>)]
pub struct WasmObject {
    data: Option<WasmObjectData<StoreData>>,
}

pub struct WasmObjectData<T> {
    pub(crate) store: Store<T>,
    pub(crate) inst: wasmtime::Instance,
}

impl WasmObject {
    /// Create new WasmEngine
    fn new(_owner: &Reference) -> Self {
        Self { data: None }
    }

    fn get_data(&self) -> &WasmObjectData<StoreData> {
        self.data.as_ref().expect("Object uninitialized!")
    }

    fn get_data_mut(&mut self) -> &mut WasmObjectData<StoreData> {
        self.data.as_mut().expect("Object uninitialized!")
    }
}

// Godot exported methods
#[methods]
impl WasmObject {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .add_property::<Instance<WasmEngine, Shared>>("engine")
            .with_getter(|this, _| this.get_data().store.data().0.clone())
            .done();
    }

    /// Initialize WASM object. MUST be called before calling anything else.
    #[export]
    fn initialize(
        &mut self,
        _owner: TRef<Reference>,
        engine: Instance<WasmEngine, Shared>,
        name: String,
        #[opt] host_bindings: Option<Dictionary>,
    ) -> Variant {
        let eobj = engine.clone();
        let host;

        if let Some(host_bindings) = host_bindings {
            host = match create_hostmap(host_bindings) {
                Ok(v) => v,
                Err(e) => {
                    godot_error!("{:?}", e);
                    return Variant::new();
                }
            };
        } else {
            host = HostMap::default();
        }

        let (store, inst) = match unsafe { engine.assume_safe() }.map(move |v, _| -> Result<_> {
            let WasmEngine { engine, modules } = v;
            let modules = modules.read();

            let ModuleData { module, deps } = match modules.get(&name) {
                Some(v) => v,
                None => bail!("No module named {}", name),
            };

            let mut store = Store::new(&engine, (eobj,));
            let mut linker = Linker::new(&engine);

            register_godot_externref(&mut linker)?;
            register_hostmap(&mut linker, host)?;

            let mut it = modules.iter();
            let mut prev = 0;
            for &i in deps.iter() {
                match it.nth(i - prev) {
                    Some((k, x)) => linker.module(&mut store, k, &x.module)?,
                    None => unreachable!("Iterator overrun"),
                };
                prev = i;
            }

            let inst = linker.instantiate(&mut store, module)?;

            Ok((store, inst))
        }) {
            Ok(Ok(v)) => v,
            Err(_) => {
                godot_error!("Cannot call into engine!");
                return Variant::new();
            }
            Ok(Err(e)) => {
                godot_error!("{:?}", e);
                return Variant::new();
            }
        };
        self.data = Some(WasmObjectData { store, inst });

        return _owner.to_variant();
    }

    /// Check if function exists
    #[export]
    fn is_function_exists(&mut self, _owner: &Reference, name: String) -> bool {
        let WasmObjectData {
            ref mut inst,
            ref mut store,
            ..
        } = self.get_data_mut();
        inst.get_func(store, &name).is_some()
    }

    /// Gets exported functions
    #[export]
    fn get_exports(&mut self, _owner: &Reference) -> VariantArray {
        let WasmObjectData {
            ref mut inst,
            ref mut store,
            ..
        } = self.get_data_mut();
        VariantArray::from_iter(inst.exports(&mut *store).filter_map(|v| {
            let ret = GodotString::from(v.name()).to_variant();
            v.into_func().map(|_| ret)
        }))
        .into_shared()
    }

    /// Call WASM function
    #[export]
    fn call_wasm(&mut self, _owner: &Reference, name: String, args: VariantArray) -> Variant {
        let WasmObjectData {
            ref inst,
            ref mut store,
            ..
        } = self.get_data_mut();

        call_func(store, inst, name, args.iter())
    }
}
