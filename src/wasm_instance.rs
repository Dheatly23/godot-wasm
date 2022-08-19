use std::collections::HashMap;
use std::mem::transmute;

use anyhow::{bail, Error};
use gdnative::export::user_data::Map;
use gdnative::prelude::*;
use parking_lot::{Once, OnceState};
use wasmer::{
    Export, Exportable, Exports, Instance as InstanceWasm, LikeNamespace, NamedResolver, Type, Val,
};

use crate::wasm_engine::{ModuleData, WasmModule};
use crate::wasm_util::{make_host_module, HOST_MODULE, MEMORY_EXPORT};

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::export::user_data::ArcData<WasmInstance>)]
pub struct WasmInstance {
    once: Once,
    data: Option<InstanceData>,
}

pub struct InstanceData {
    instance: InstanceWasm,
    module: Instance<WasmModule, Shared>,
}

impl InstanceData {
    pub fn instantiate(
        module: Instance<WasmModule, Shared>,
        host: &Option<Exports>,
    ) -> Result<Self, Error> {
        type InstMap = HashMap<Ref<Reference, Shared>, InstanceWasm>;

        struct Res<'a> {
            host: Option<&'a Exports>,
            imports: &'a HashMap<String, Instance<WasmModule, Shared>>,
            insts: &'a InstMap,
        }

        impl<'a> NamedResolver for Res<'a> {
            fn resolve_by_name(&self, module: &str, field: &str) -> Option<Export> {
                if let Some(host) = self.host.as_ref() {
                    if module == HOST_MODULE {
                        return host.get_extern(field).map(|v| v.to_export());
                    }
                }
                self.insts
                    .get(&self.imports.get(module)?.base())
                    .and_then(|inst| inst.exports.get_namespace_export(field))
            }
        }

        fn f(
            module: &ModuleData,
            insts: &mut InstMap,
            host: &Option<Exports>,
        ) -> Result<InstanceWasm, Error> {
            for v in module.imports.values() {
                let v_ = v.base();
                if insts.get(v_).is_some() {
                    continue;
                }
                let v = v
                    .script()
                    .map(|m| f(m.get_data()?, &mut *insts, host))
                    .unwrap()?;
                insts.insert(Ref::clone(v_), v);
            }
            Ok(InstanceWasm::new(
                &module.module,
                &Res {
                    host: host.as_ref(),
                    imports: &module.imports,
                    insts,
                },
            )?)
        }

        Ok(Self {
            instance: module
                .script()
                .map(move |m| {
                    let mut insts = HashMap::new();
                    f(m.get_data()?, &mut insts, host)
                })
                .unwrap()?,
            module: module,
        })
    }
}

impl WasmInstance {
    fn new(_owner: &Reference) -> Self {
        Self {
            once: Once::new(),
            data: None,
        }
    }

    pub fn get_data(&self) -> Result<&InstanceData, Error> {
        if let OnceState::Done = self.once.state() {
            Ok(self.data.as_ref().unwrap())
        } else {
            bail!("Uninitialized module")
        }
    }

    pub fn initialize_(
        &self,
        module: Instance<WasmModule, Shared>,
        host: &Option<Exports>,
    ) -> bool {
        let mut r = true;
        let ret = &mut r;

        self.once
            .call_once(move || match InstanceData::instantiate(module, host) {
                Ok(v) => {
                    // SAFETY: Should be called only once and nobody else can read module data
                    #[allow(mutable_transmutes)]
                    let data = unsafe {
                        transmute::<&Option<InstanceData>, &mut Option<InstanceData>>(&self.data)
                    };
                    *data = Some(v);
                }
                Err(e) => {
                    godot_error!("{}", e);
                    *ret = false;
                }
            });

        r
    }
}

#[methods]
impl WasmInstance {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .property::<Option<Instance<WasmModule, Shared>>>("module")
            .with_getter(|v, _| match v.get_data() {
                Ok(m) => Some(Instance::clone(&m.module)),
                Err(e) => {
                    godot_error!("{}", e);
                    None
                }
            })
            .done();
    }

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[export]
    fn initialize(
        &self,
        owner: TRef<Reference>,
        module: Instance<WasmModule, Shared>,
        #[opt] host: Option<Dictionary>,
    ) -> Option<Ref<Reference>> {
        let host = match host.map(|h| make_host_module(h)).transpose() {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                return None;
            }
        };
        if self.initialize_(module, &host) {
            Some(owner.claim())
        } else {
            None
        }
    }

    #[export]
    fn call_wasm(
        &self,
        _owner: &Reference,
        name: String,
        args: VariantArray,
    ) -> Option<VariantArray> {
        match self.get_data().and_then(|m| {
            let f = m.instance.exports.get_function(&name)?;

            let params = f.ty().params();
            let mut p = Vec::with_capacity(params.len());

            for (ix, t) in params.iter().enumerate() {
                let v = args.get(ix as _);
                p.push(match t {
                    Type::I32 => Val::I32(i32::from_variant(&v)?),
                    Type::I64 => Val::I64(i64::from_variant(&v)?),
                    Type::F32 => Val::F32(f32::from_variant(&v)?),
                    Type::F64 => Val::F64(f64::from_variant(&v)?),
                    _ => bail!("Unsupported WASM type conversion {}", t),
                });
            }

            for t in f.ty().results() {
                match t {
                    Type::I32 | Type::I64 | Type::F32 | Type::F64 => (),
                    _ => bail!("Unsupported WASM type conversion {}", t),
                }
            }

            let r = f.call(&p)?;

            let ret = VariantArray::new();
            for i in r.iter() {
                ret.push(match i {
                    Val::I32(v) => v.to_variant(),
                    Val::I64(v) => v.to_variant(),
                    Val::F32(v) => v.to_variant(),
                    Val::F64(v) => v.to_variant(),
                    _ => bail!("Unsupported WASM type conversion"),
                });
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

    #[export]
    fn has_memory(&self, _owner: &Reference) -> bool {
        match self
            .get_data()
            .and_then(|m| Ok(m.instance.exports.get_memory(MEMORY_EXPORT).is_ok()))
        {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[export]
    fn memory_size(&self, _owner: &Reference) -> u64 {
        match self.get_data().and_then(|m| {
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            Ok(mem.data_size())
        }) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                0
            }
        }
    }

    #[export]
    fn memory_read(&self, _owner: &Reference, i: usize, n: usize) -> Option<ByteArray> {
        match self.get_data().and_then(|m| {
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;

            // SAFETY: It's up to the user to not access this object concurrently
            // (See Godot's policy on concurrency)
            unsafe {
                let s = match mem.data_unchecked().get(i..i + n) {
                    Some(v) => v,
                    None => bail!("Out of bounds!"),
                };
                Ok(ByteArray::from_slice(s))
            }
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[export]
    fn memory_write(&self, _owner: &Reference, i: usize, a: ByteArray) -> bool {
        match self.get_data().and_then(|m| {
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;

            // SAFETY: It's up to the user to not access this object concurrently
            // (See Godot's policy on concurrency)
            unsafe {
                let s_ = &*a.read();
                let s = match mem.data_unchecked_mut().get_mut(i..i + s_.len()) {
                    Some(v) => v,
                    None => bail!("Out of bounds!"),
                };
                s.copy_from_slice(s_);
                Ok(())
            }
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }
}
