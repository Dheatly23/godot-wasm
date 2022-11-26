use std::collections::HashMap;
use std::mem::transmute;

use anyhow::{bail, Error};
use gdnative::export::user_data::Map;
use gdnative::prelude::*;
use parking_lot::{Once, OnceState, ReentrantMutex};
use wasmer::{
    AsStoreMut, Engine, Exports, Extern, Imports, Instance as InstanceWasm, Store, Type, Value,
    WasmRef,
};

use crate::wasm_engine::{ModuleData, WasmModule, ENGINE};
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
    store: ReentrantMutex<Store>,
    instance: InstanceWasm,
    module: Instance<WasmModule, Shared>,
}

impl InstanceData {
    pub fn instantiate(
        mut store: Store,
        module: Instance<WasmModule, Shared>,
        host: Option<Dictionary>,
    ) -> Result<Self, Error> {
        type InstMap = HashMap<Ref<Reference, Shared>, InstanceWasm>;

        fn g((k, v): (&String, &Extern)) -> (String, Extern) {
            (k.clone(), v.clone())
        }

        fn f(
            store: &mut Store,
            module: &ModuleData,
            insts: &mut InstMap,
            host: &Option<Exports>,
        ) -> Result<InstanceWasm, Error> {
            let mut imports = Imports::new();
            if let Some(host) = host.as_ref() {
                imports.register_namespace(HOST_MODULE, host.into_iter().map(g));
            }

            for (k, v) in module.imports.iter() {
                let inst = loop {
                    match insts.get(v.base()) {
                        Some(i) => break i,
                        None => {
                            let i = v
                                .script()
                                .map(|m| f(&mut *store, m.get_data()?, &mut *insts, host))
                                .unwrap()?;
                            insts.insert(Ref::clone(v.base()), i);
                        }
                    };
                };
                imports.register_namespace(k, (&inst.exports).into_iter().map(g));
            }

            InstanceWasm::new(store, &module.module, &imports).map_err(Error::from)
        }

        let host = host.map(|h| make_host_module(&mut store, h)).transpose()?;

        let sp = &mut store;
        let instance = module
            .script()
            .map(move |m| {
                let mut insts = HashMap::new();
                f(sp, m.get_data()?, &mut insts, &host)
            })
            .unwrap()?;

        Ok(Self {
            instance: instance,
            module: module,
            store: ReentrantMutex::new(store),
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
        host: Option<Dictionary>,
    ) -> bool {
        let mut r = true;
        let ret = &mut r;

        self.once.call_once(move || {
            match InstanceData::instantiate(
                Store::new(Engine::clone(ENGINE.engine())),
                module,
                host,
            ) {
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
    #[method]
    fn initialize(
        &self,
        #[base] owner: TRef<Reference>,
        module: Instance<WasmModule, Shared>,
        #[opt] host: Option<Dictionary>,
    ) -> Option<Ref<Reference>> {
        if self.initialize_(module, host) {
            Some(owner.claim())
        } else {
            None
        }
    }

    #[method]
    fn call_wasm(&self, name: String, args: VariantArray) -> Option<VariantArray> {
        match self.get_data().and_then(|m| {
            let store = m.store.lock();
            // SAFETY: Store should be safe to mutate within thread?
            #[allow(mutable_transmutes)]
            let mut store = unsafe { transmute::<&Store, &mut Store>(&store).as_store_mut() };
            let f = m.instance.exports.get_function(&name)?;

            let mut p;

            {
                let ty = f.ty(&store);
                let params = ty.params();
                p = Vec::with_capacity(params.len());

                for (ix, t) in params.iter().enumerate() {
                    let v = args.get(ix as _);
                    p.push(match t {
                        Type::I32 => i32::from_variant(&v)?.into(),
                        Type::I64 => i64::from_variant(&v)?.into(),
                        Type::F32 => f32::from_variant(&v)?.into(),
                        Type::F64 => f64::from_variant(&v)?.into(),
                        _ => bail!("Unsupported WASM type conversion {}", t),
                    });
                }

                for t in ty.results() {
                    match t {
                        Type::I32 | Type::I64 | Type::F32 | Type::F64 => (),
                        _ => bail!("Unsupported WASM type conversion {}", t),
                    }
                }
            }

            let r = f.call(&mut store, &p)?;

            let ret = VariantArray::new();
            for i in r.iter() {
                ret.push(match i {
                    Value::I32(v) => v.to_variant(),
                    Value::I64(v) => v.to_variant(),
                    Value::F32(v) => v.to_variant(),
                    Value::F64(v) => v.to_variant(),
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

    #[method]
    fn has_memory(&self) -> bool {
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

    #[method]
    fn memory_size(&self) -> usize {
        match self.get_data().and_then(|m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);
            Ok(view.size().bytes().0)
        }) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                0
            }
        }
    }

    #[method]
    fn memory_read(&self, i: usize, n: usize) -> Option<ByteArray> {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);
            let mut v = vec![0u8; n];

            view.read(i as _, &mut v)?;
            Ok(ByteArray::from_vec(v))
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn memory_write(&self, i: usize, a: ByteArray) -> bool {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            view.write(i as _, &a.read())?;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn get_8(&self, i: usize) -> Option<u8> {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            Ok(WasmRef::new(&view, i as _).read()?)
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_8(&self, i: usize, v: u8) -> bool {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            WasmRef::new(&view, i as _).write(v)?;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn get_16(&self, i: usize) -> Option<u16> {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            Ok(WasmRef::new(&view, i as _).read()?)
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_16(&self, i: usize, v: u16) -> bool {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            WasmRef::new(&view, i as _).write(v)?;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn get_32(&self, i: usize) -> Option<u32> {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            Ok(WasmRef::new(&view, i as _).read()?)
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_32(&self, i: usize, v: u32) -> bool {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            WasmRef::new(&view, i as _).write(v)?;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn get_64(&self, i: usize) -> Option<i64> {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            Ok(WasmRef::new(&view, i as _).read()?)
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_64(&self, i: usize, v: i64) -> bool {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            WasmRef::new(&view, i as _).write(v)?;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn get_float(&self, i: usize) -> Option<f32> {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            Ok(WasmRef::new(&view, i as _).read()?)
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_float(&self, i: usize, v: f32) -> bool {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            WasmRef::new(&view, i as _).write(v)?;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn get_double(&self, i: usize) -> Option<f64> {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            Ok(WasmRef::new(&view, i as _).read()?)
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_double(&self, i: usize, v: f64) -> bool {
        match self.get_data().and_then(move |m| {
            let store = m.store.lock();
            let mem = m.instance.exports.get_memory(MEMORY_EXPORT)?;
            let view = mem.view(&*store);

            WasmRef::new(&view, i as _).write(v)?;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }
}
