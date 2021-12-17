use std::iter::FromIterator;
use std::mem::take;

use anyhow::{bail, Result};
use gdnative::prelude::*;
use hashbrown::HashMap;
use wasmtime::{FuncType, Linker, Store, Trap, Val, ValRaw, ValType};

use crate::wasm_engine::*;
use crate::{TYPE_F32, TYPE_F64, TYPE_I32, TYPE_I64};

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

type HostMap = HashMap<GodotString, (Ref<Object, Shared>, GodotString, FuncType)>;
type StoreData = (Instance<WasmEngine, Shared>, HostMap);

#[derive(NativeClass)]
#[inherit(Object)]
#[user_data(gdnative::nativescript::user_data::MutexData<WasmObject>)]
pub struct WasmObject {
    data: Option<WasmObjectData<StoreData>>,
    host: HostMap,
}

pub struct WasmObjectData<T> {
    pub(crate) store: Store<T>,
    pub(crate) inst: wasmtime::Instance,
}

impl WasmObject {
    /// Create new WasmEngine
    fn new(_owner: &Object) -> Self {
        Self {
            data: None,
            host: HostMap::default(),
        }
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
            .with_getter(|this, _| this.get_data().store.data().0.clone())
            .done();
    }

    /// Register new function handle. MUST be called before initialize()
    #[export]
    fn register_host_handle(
        &mut self,
        _owner: &Object,
        name: GodotString,
        signature: Dictionary,
        object: Ref<Object, Shared>,
        method: GodotString,
    ) {
        if self.data.is_some() {
            godot_warn!("WASM object already initialized!");
            return;
        }

        fn to_valtypes(sig: Variant) -> Vec<ValType> {
            let f = |v| match v {
                TYPE_I32 => ValType::I32,
                TYPE_I64 => ValType::I64,
                TYPE_F32 => ValType::F32,
                TYPE_F64 => ValType::F64,
                _ => panic!("Cannot convert signature!"),
            };
            if let Some(x) = sig.try_to_byte_array() {
                x.read()
                    .as_slice()
                    .iter()
                    .map(|&v| v as u32)
                    .map(f)
                    .collect()
            } else if let Some(x) = sig.try_to_int32_array() {
                x.read()
                    .as_slice()
                    .iter()
                    .map(|&v| v as u32)
                    .map(f)
                    .collect()
            } else if let Ok(x) = VariantArray::from_variant(&sig) {
                x.iter()
                    .map(|v| u32::from_variant(&v).expect("Cannot convert signature!"))
                    .map(f)
                    .collect()
            } else {
                panic!("Cannot convert signature!")
            }
        }

        let params = to_valtypes(signature.get("params"));
        let results = to_valtypes(signature.get("results"));

        let ft = FuncType::new(params, results);

        self.host.insert(name, (object, method, ft));
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
        let host = take(&mut self.host);

        let (store, inst) = match unsafe { engine.assume_safe() }.map(move |v, _| -> Result<_> {
            let WasmEngine { engine, modules } = v;
            let modules = modules.read();

            let ModuleData { module, deps } = match modules.get(&name) {
                Some(v) => v,
                None => bail!("No module named {}", name),
            };

            let mut store = Store::new(&engine, (eobj, host));
            let mut linker = Linker::new(&engine);

            unsafe fn set_raw(
                v: *mut ValRaw,
                t: ValType,
                var: Variant,
            ) -> Result<(), FromVariantError> {
                match t {
                    ValType::I32 => (*v).i32 = i32::from_variant(&var)?,
                    ValType::I64 => (*v).i64 = i64::from_variant(&var)?,
                    ValType::F32 => (*v).f32 = f32::from_variant(&var)?.to_bits(),
                    ValType::F64 => (*v).f64 = f64::from_variant(&var)?.to_bits(),
                    _ => unreachable!("Unsupported type"),
                };
                Ok(())
            }

            for (name, (_, _, ty)) in store.data().1.iter() {
                let name = name.clone();
                unsafe {
                    linker.func_new_unchecked(
                        "host",
                        &name.to_string(),
                        ty.clone(),
                        move |caller, raw| {
                            let data: &StoreData = caller.data();
                            let (object, method, ty) = data.1.get(&name).unwrap();

                            let params = ty.params();
                            let mut input = Vec::with_capacity(params.len());
                            for (i, p) in params.enumerate() {
                                let v = raw.add(i);
                                input.push(match p {
                                    ValType::I32 => (*v).i32.to_variant(),
                                    ValType::I64 => (*v).i64.to_variant(),
                                    ValType::F32 => f32::from_bits((*v).f32).to_variant(),
                                    ValType::F64 => f64::from_bits((*v).f64).to_variant(),
                                    _ => unreachable!("Unsupported type"),
                                });
                            }

                            let object = match object.assume_safe_if_sane() {
                                Some(v) => v,
                                None => return Err(Trap::new("Object has been deleted")),
                            };
                            let output = object.call(method.clone(), &input);

                            let ef = |v| Trap::from(Box::new(v) as Box<_>);

                            let mut results = ty.results();
                            if results.len() == 0 {
                                return Ok(());
                            } else if (results.len() == 1)
                                && VariantArray::from_variant(&output).is_err()
                            {
                                return set_raw(raw, results.next().unwrap(), output).map_err(ef);
                            }
                            let output = VariantArray::from_variant(&output).map_err(ef)?;
                            if (output.len() as usize) < results.len() {
                                return Err(Trap::new("Array too short"));
                            }
                            for (i, (t, v)) in results.zip(output.iter()).enumerate() {
                                set_raw(raw.add(i), t, v).map_err(ef)?;
                            }
                            Ok(())
                        },
                    )
                }?;
            }

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
                godot_error!("Error! {}", e);
                return Variant::new();
            }
        };
        self.data = Some(WasmObjectData { store, inst });

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
            e => {
                godot_error!("Function invocation error: {}", e);
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
