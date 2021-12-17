use std::iter::FromIterator;

use anyhow::{bail, Result};
use gdnative::prelude::*;
use wasmtime::{Linker, Store, Val, ValType};

use crate::wasm_engine::*;

macro_rules! unwrap_ext {
    {$v:expr; $e:expr} => {
        match $v {
            Ok(v) => v,
            Err(_) => $e,
        }
    };
    {$v:expr; $e:ident => $ee:expr} => {
        match $v {
            Ok(v) => v,
            Err($e) => $ee,
        }
    };
}

type StoreData = Instance<WasmEngine, Shared>;

#[derive(NativeClass)]
#[inherit(Object)]
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
    fn new(_owner: &Object) -> Self {
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
    fn register_builder(builder: &ClassBuilder<Self>) {
        builder
            .add_property::<Instance<WasmEngine, Shared>>("engine")
            .with_getter(|this, _| this.get_data().store.data().clone())
            .done();
    }

    /// Initialize WASM object. MUST be called before calling anything else.
    #[export]
    fn initialize(
        &mut self,
        _owner: TRef<Object>,
        engine: Instance<WasmEngine, Shared>,
        name: String,
    ) -> Variant {
        let eobj = engine.clone();
        let (store, inst) = match unsafe { engine.assume_safe() }.map(move |v, _| -> Result<_> {
            let WasmEngine { engine, modules } = v;
            let modules = modules.read();

            let ModuleData { module, deps } = match modules.get(&name) {
                Some(v) => v,
                None => bail!("No module named {}", name),
            };

            let mut store = Store::new(&engine, eobj);
            let mut linker = Linker::new(&engine);

            let mut it = modules.iter();
            let mut prev = 0;
            for &i in deps.iter() {
                match it.nth(i - prev) {
                    Some((k, x)) => linker.module(&mut store, k, &x.module)?,
                    None => bail!("Iterator overrun"),
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
                godot_error!("Error! {}", e);
                return Variant::new();
            }
        };
        self.data = Some(WasmObjectData {
            store,
            inst,
        });

        return _owner.to_variant();
    }

    /// Check if function exists
    #[export]
    fn is_function_exists(&mut self, _owner: &Object, name: String) -> bool {
        let WasmObjectData {
            ref mut inst,
            ref mut store,
            ..
        } = self.get_data_mut();
        inst.get_func(store, &name).is_some()
    }

    /// Gets exported functions
    #[export]
    fn get_exports(&mut self, _owner: &Object) -> VariantArray {
        let WasmObjectData {
            ref mut inst,
            ref mut store,
            ..
        } = self.get_data_mut();
        VariantArray::from_iter(inst.exports(&mut *store).filter_map(|v| {
            let ret = GodotString::from(v.name()).to_variant();
            if v.into_func().is_some() {
                Some(ret)
            } else {
                None
            }
        }))
        .into_shared()
    }

    /// Call WASM function
    #[export]
    fn call_wasm(&mut self, _owner: &Object, name: String, args: VariantArray) -> Variant {
        let WasmObjectData {
            ref mut inst,
            ref mut store,
            ..
        } = self.get_data_mut();
        let func = match inst.get_func(&mut *store, &name) {
            Some(f) => f,
            None => {
                godot_error!("WASM Function {} does not exist!", name);
                return Variant::new();
            }
        };

        let params: Vec<Val>;
        let mut results: Vec<Val>;

        {
            let ty = func.ty(&mut *store);
            params = ty
                .params()
                .zip(args.iter())
                .enumerate()
                .map(|(i, (t, a))| match t {
                    ValType::I32 => Val::I32(unwrap_ext! {
                        i32::from_variant(&a);
                        panic!("Argument {} type mismatch (expected i32)!", i)
                    }),
                    ValType::I64 => Val::I64(unwrap_ext! {
                        i64::from_variant(&a);
                        panic!("Argument {} type mismatch (expected i64)!", i)
                    }),
                    ValType::F32 => Val::F32(
                        unwrap_ext! {
                            f32::from_variant(&a);
                            panic!("Argument {} type mismatch (expected f32)!", i)
                        }
                        .to_bits(),
                    ),
                    ValType::F64 => Val::F64(
                        unwrap_ext! {
                            f64::from_variant(&a);
                            panic!("Argument {} type mismatch (expected f64)!", i)
                        }
                        .to_bits(),
                    ),
                    _ => panic!("Unconvertible WASM argument type!"),
                })
                .collect();
            if params.len() != ty.params().len() {
                godot_error!("Too few arguments!");
                return Variant::new();
            }
            results = ty
                .results()
                .map(|t| match t {
                    ValType::I32 => Val::I32(0),
                    ValType::I64 => Val::I64(0),
                    ValType::F32 => Val::F32(0.0f32.to_bits()),
                    ValType::F64 => Val::F64(0.0f64.to_bits()),
                    _ => panic!("Unconvertible WASM argument type!"),
                })
                .collect();
        }

        unwrap_ext! {
            func.call(&mut *store, &params, &mut results);
            {
                godot_error!("Function invocation error!");
                return Variant::new();
            }
        };

        VariantArray::from_iter(results.into_iter().map(|v| match v {
            Val::I32(v) => v.to_variant(),
            Val::I64(v) => v.to_variant(),
            Val::F32(v) => f32::from_bits(v).to_variant(),
            Val::F64(v) => f64::from_bits(v).to_variant(),
            _ => panic!("Unconvertible WASM argument type!"),
        }))
        .into_shared()
        .to_variant()
    }
}
