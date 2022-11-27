use std::collections::HashMap;
use std::mem::{swap, transmute};
use std::ptr;

use anyhow::{bail, Error};
use gdnative::export::user_data::Map;
use gdnative::prelude::*;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, Once, OnceState, RawMutex};
use scopeguard::guard;
use wasmtime::{
    AsContextMut, Extern, Instance as InstanceWasm, Memory, Store, StoreContextMut, ValRaw, ValType,
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
    store: Mutex<Store<StoreData>>,
    instance: InstanceWasm,
    module: Instance<WasmModule, Shared>,
}

pub struct StoreData {
    mutex_raw: *const RawMutex,
}

// SAFETY: Store data is safely contained within instance data?
unsafe impl Send for StoreData {}
unsafe impl Sync for StoreData {}

impl InstanceData {
    pub fn instantiate(
        mut store: Store<StoreData>,
        module: Instance<WasmModule, Shared>,
        host: Option<Dictionary>,
    ) -> Result<Self, Error> {
        type InstMap = HashMap<Ref<Reference, Shared>, InstanceWasm>;

        fn f<T>(
            store: &mut Store<T>,
            module: &ModuleData,
            insts: &mut InstMap,
            host: &Option<HashMap<String, Extern>>,
        ) -> Result<InstanceWasm, Error> {
            let it = module.module.imports();
            let mut imports = Vec::with_capacity(it.len());

            for i in it {
                if i.module() == HOST_MODULE {
                    if let Some(host) = host.as_ref() {
                        if let Some(v) = host.get(i.name()) {
                            imports.push(v.clone());
                            continue;
                        }
                    }
                }

                if let Some(v) = module.imports.get(i.module()) {
                    let v = loop {
                        match insts.get(v.base()) {
                            Some(v) => break v,
                            None => {
                                let t = v
                                    .script()
                                    .map(|m| f(&mut *store, m.get_data()?, &mut *insts, host))
                                    .unwrap()?;
                                insts.insert(v.base().clone(), t);
                            }
                        }
                    };

                    if let Some(v) = v.get_export(&mut *store, i.name()) {
                        imports.push(v.clone());
                        continue;
                    }
                }

                bail!("Unknown import {:?}.{:?}", i.module(), i.name());
            }

            Ok(InstanceWasm::new(store, &module.module, &imports)?)
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
            store: Mutex::new(store),
        })
    }

    fn acquire_store<F, R>(&self, f: F) -> R
    where
        for<'a> F: FnOnce(&Self, StoreContextMut<'a, StoreData>) -> R,
    {
        let mut guard_ = self.store.lock();

        let _scope;
        // SAFETY: Context should be destroyed after function call
        unsafe {
            let p = &mut guard_.data_mut().mutex_raw as *mut _;
            let mut v = self.store.raw() as *const _;
            swap(&mut *p, &mut v);
            _scope = guard(p, move |p| {
                *p = v;
            });
        }

        f(self, guard_.as_context_mut())
    }
}

impl StoreData {
    pub(crate) fn release_store<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard;
        if !self.mutex_raw.is_null() {
            // SAFETY: Pointer is valid and locked mutex
            unsafe {
                _guard = guard(&*self.mutex_raw, |v| v.lock());
                _guard.unlock();
            }
        }

        f()
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
                Store::new(
                    &*ENGINE,
                    StoreData {
                        mutex_raw: ptr::null(),
                    },
                ),
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

    fn get_memory<F, R>(&self, f: F) -> Result<R, Error>
    where
        for<'a> F: FnOnce(StoreContextMut<'a, StoreData>, Memory) -> Result<R, Error>,
    {
        self.get_data()?.acquire_store(move |m, mut store| {
            match m.instance.get_memory(&mut store, MEMORY_EXPORT) {
                Some(mem) => Ok(f(store, mem)?),
                None => bail!("No memory exported"),
            }
        })
    }

    fn read_memory<F, R>(&self, i: usize, n: usize, f: F) -> Result<R, Error>
    where
        F: FnOnce(&[u8]) -> Result<R, Error>,
    {
        self.get_memory(|store, mem| {
            let data = mem.data(&store);
            match data.get(i..i + n) {
                Some(s) => Ok(f(s)?),
                None => bail!("Index out of bound {}-{}", i, i + n),
            }
        })
    }

    fn write_memory<F, R>(&self, i: usize, n: usize, f: F) -> Result<R, Error>
    where
        for<'a> F: FnOnce(&'a mut [u8]) -> Result<R, Error>,
    {
        self.get_memory(|mut store, mem| {
            let data = mem.data_mut(&mut store);
            match data.get_mut(i..i + n) {
                Some(s) => Ok(f(s)?),
                None => bail!("Index out of bound {}-{}", i, i + n),
            }
        })
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
        match self.get_data().and_then(move |m| {
            m.acquire_store(move |m, mut store| {
                let f = match m.instance.get_export(&mut store, &name) {
                    Some(f) => match f {
                        Extern::Func(f) => f,
                        _ => bail!("Export {} is not a function", &name),
                    },
                    None => bail!("Export {} does not exists", &name),
                };

                let ty = f.ty(&store);
                let pi = ty.params();
                let ri = ty.results();
                let mut arr = vec![ValRaw::i32(0); pi.len().max(ri.len())];

                for (ix, t) in pi.enumerate() {
                    let v = args.get(ix as _);
                    arr[ix] = match t {
                        ValType::I32 => ValRaw::i32(i32::from_variant(&v)?),
                        ValType::I64 => ValRaw::i64(i64::from_variant(&v)?),
                        ValType::F32 => ValRaw::f32(f32::from_variant(&v)?.to_bits()),
                        ValType::F64 => ValRaw::f64(f64::from_variant(&v)?.to_bits()),
                        _ => bail!("Unsupported WASM type conversion {}", t),
                    };
                }

                for t in ty.results() {
                    match t {
                        ValType::I32 | ValType::I64 | ValType::F32 | ValType::F64 => (),
                        _ => bail!("Unsupported WASM type conversion {}", t),
                    }
                }

                // SAFETY: Array length is maximum of params and returns and initialized
                unsafe {
                    f.call_unchecked(&mut store, arr.as_mut_ptr())?;
                }

                let ret = VariantArray::new();
                for (ix, t) in ri.enumerate() {
                    let v = arr[ix];
                    ret.push(match t {
                        ValType::I32 => v.get_i32().to_variant(),
                        ValType::I64 => v.get_i64().to_variant(),
                        ValType::F32 => f32::from_bits(v.get_f32()).to_variant(),
                        ValType::F64 => f64::from_bits(v.get_f64()).to_variant(),
                        _ => bail!("Unsupported WASM type conversion {}", t),
                    });
                }

                Ok(ret.into_shared())
            })
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
        match self.get_data().and_then(|m| {
            m.acquire_store(|m, mut store| {
                Ok(match m.instance.get_export(&mut store, MEMORY_EXPORT) {
                    Some(Extern::Memory(_)) => true,
                    _ => false,
                })
            })
        }) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                false
            }
        }
    }

    #[method]
    fn memory_size(&self) -> usize {
        match self.get_memory(|store, mem| Ok(mem.data_size(&store))) {
            Ok(v) => v,
            Err(e) => {
                godot_error!("{}", e);
                0
            }
        }
    }

    #[method]
    fn memory_read(&self, i: usize, n: usize) -> Option<ByteArray> {
        match self.read_memory(i, n, |s| Ok(ByteArray::from_slice(s))) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn memory_write(&self, i: usize, a: ByteArray) -> bool {
        let a = &a.read();
        match self.write_memory(i, i + a.len(), |s| {
            s.copy_from_slice(a);
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
        match self.read_memory(i, i + 1, |s| Ok(s[0])) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_8(&self, i: usize, v: u8) -> bool {
        match self.write_memory(i, i + 1, |s| {
            s[0] = v;
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
        match self.read_memory(i, i + 2, |s| Ok(u16::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_16(&self, i: usize, v: u16) -> bool {
        match self.write_memory(i, i + 2, |s| {
            s.copy_from_slice(&v.to_le_bytes());
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
        match self.read_memory(i, i + 4, |s| Ok(u32::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_32(&self, i: usize, v: u32) -> bool {
        match self.write_memory(i, i + 4, |s| {
            s.copy_from_slice(&v.to_le_bytes());
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
        match self.read_memory(i, i + 8, |s| Ok(i64::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_64(&self, i: usize, v: i64) -> bool {
        match self.write_memory(i, i + 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
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
        match self.read_memory(i, i + 4, |s| Ok(f32::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_float(&self, i: usize, v: f32) -> bool {
        match self.write_memory(i, i + 4, |s| {
            s.copy_from_slice(&v.to_le_bytes());
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
        match self.read_memory(i, i + 8, |s| Ok(f64::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                godot_error!("{}", e);
                None
            }
        }
    }

    #[method]
    fn store_double(&self, i: usize, v: f64) -> bool {
        match self.write_memory(i, i + 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
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
