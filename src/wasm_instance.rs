use std::collections::HashMap;
use std::mem::{swap, transmute};
use std::ptr;

use anyhow::{bail, Error};
use gdnative::export::user_data::Map;
use gdnative::prelude::*;
use parking_lot::{lock_api::RawMutex as RawMutexTrait, Mutex, Once, OnceState, RawMutex};
use scopeguard::guard;
use wasmtime::{
    AsContextMut, Extern, Instance as InstanceWasm, Memory, Store, StoreContextMut, ValRaw,
};

use crate::wasm_config::Config;
use crate::wasm_engine::{EpochThreadHandle, ModuleData, WasmModule, ENGINE, EPOCH};
use crate::wasm_util::{
    from_raw, make_host_module, to_raw, EPOCH_DEADLINE, HOST_MODULE, MEMORY_EXPORT,
};

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
    config: Config,
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

        fn f(
            store: &mut Store<StoreData>,
            module: &ModuleData,
            insts: &mut InstMap,
            host: &Option<HashMap<String, Extern>>,
        ) -> Result<InstanceWasm, Error> {
            if store.data().config.with_epoch {
                store.epoch_deadline_trap();
                EpochThreadHandle::spawn_thread(&EPOCH, || ENGINE.increment_epoch());
            } else {
                store.epoch_deadline_callback(|_| Ok(EPOCH_DEADLINE));
            }

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

            store.set_epoch_deadline(EPOCH_DEADLINE);
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
        config: Option<Variant>,
    ) -> bool {
        let mut r = true;
        let ret = &mut r;

        self.once.call_once(move || {
            match InstanceData::instantiate(
                Store::new(
                    &*ENGINE,
                    StoreData {
                        mutex_raw: ptr::null(),
                        config: match config {
                            Some(v) => match Config::from_variant(&v) {
                                Ok(v) => v,
                                Err(e) => {
                                    godot_error!("{}", e);
                                    Config::default()
                                }
                            },
                            None => Config::default(),
                        },
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

        builder
            .signal("error_happened")
            .with_param("message", VariantType::GodotString)
            .done();
    }

    fn emit_error(&self, base: TRef<Reference>, err: Error) {
        let err = err.to_string();
        godot_error!("{}", err);
        base.emit_signal("error_happened", &[err.to_variant()]);
    }

    /// Initialize and loads module.
    /// MUST be called for the first time and only once.
    #[method]
    fn initialize(
        &self,
        #[base] owner: TRef<Reference>,
        module: Instance<WasmModule, Shared>,
        #[opt] host: Option<Dictionary>,
        #[opt] config: Option<Variant>,
    ) -> Option<Ref<Reference>> {
        if self.initialize_(module, host, config) {
            Some(owner.claim())
        } else {
            None
        }
    }

    #[method]
    fn call_wasm(
        &self,
        #[base] base: TRef<Reference>,
        name: String,
        args: VariantArray,
    ) -> Option<VariantArray> {
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
                    arr[ix] = unsafe { to_raw(t, args.get(ix as _))? };
                }

                store.set_epoch_deadline(EPOCH_DEADLINE);
                // SAFETY: Array length is maximum of params and returns and initialized
                unsafe {
                    f.call_unchecked(&mut store, arr.as_mut_ptr())?;
                }

                let ret = VariantArray::new();
                for (ix, t) in ri.enumerate() {
                    ret.push(unsafe { from_raw(t, arr[ix])? });
                }

                Ok(ret.into_shared())
            })
        }) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn has_memory(&self, #[base] base: TRef<Reference>) -> bool {
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
                self.emit_error(base, e);
                false
            }
        }
    }

    #[method]
    fn memory_size(&self, #[base] base: TRef<Reference>) -> usize {
        match self.get_memory(|store, mem| Ok(mem.data_size(&store))) {
            Ok(v) => v,
            Err(e) => {
                self.emit_error(base, e);
                0
            }
        }
    }

    #[method]
    fn memory_read(&self, #[base] base: TRef<Reference>, i: usize, n: usize) -> Option<ByteArray> {
        match self.read_memory(i, n, |s| Ok(ByteArray::from_slice(s))) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn memory_write(&self, #[base] base: TRef<Reference>, i: usize, a: ByteArray) -> bool {
        let a = &*a.read();
        match self.write_memory(i, a.len(), |s| {
            s.copy_from_slice(a);
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                self.emit_error(base, e);
                false
            }
        }
    }

    #[method]
    fn get_8(&self, #[base] base: TRef<Reference>, i: usize) -> Option<u8> {
        match self.read_memory(i, 1, |s| Ok(s[0])) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn put_8(&self, #[base] base: TRef<Reference>, i: usize, v: u8) -> bool {
        match self.write_memory(i, 1, |s| {
            s[0] = v;
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                self.emit_error(base, e);
                false
            }
        }
    }

    #[method]
    fn get_16(&self, #[base] base: TRef<Reference>, i: usize) -> Option<u16> {
        match self.read_memory(i, 2, |s| Ok(u16::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn put_16(&self, #[base] base: TRef<Reference>, i: usize, v: u16) -> bool {
        match self.write_memory(i, 2, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                self.emit_error(base, e);
                false
            }
        }
    }

    #[method]
    fn get_32(&self, #[base] base: TRef<Reference>, i: usize) -> Option<u32> {
        match self.read_memory(i, 4, |s| Ok(u32::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn put_32(&self, #[base] base: TRef<Reference>, i: usize, v: u32) -> bool {
        match self.write_memory(i, 4, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                self.emit_error(base, e);
                false
            }
        }
    }

    #[method]
    fn get_64(&self, #[base] base: TRef<Reference>, i: usize) -> Option<i64> {
        match self.read_memory(i, 8, |s| Ok(i64::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn put_64(&self, #[base] base: TRef<Reference>, i: usize, v: i64) -> bool {
        match self.write_memory(i, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                self.emit_error(base, e);
                false
            }
        }
    }

    #[method]
    fn get_float(&self, #[base] base: TRef<Reference>, i: usize) -> Option<f32> {
        match self.read_memory(i, 4, |s| Ok(f32::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn put_float(&self, #[base] base: TRef<Reference>, i: usize, v: f32) -> bool {
        match self.write_memory(i, 4, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                self.emit_error(base, e);
                false
            }
        }
    }

    #[method]
    fn get_double(&self, #[base] base: TRef<Reference>, i: usize) -> Option<f64> {
        match self.read_memory(i, 8, |s| Ok(f64::from_le_bytes(s.try_into().unwrap()))) {
            Ok(v) => Some(v),
            Err(e) => {
                self.emit_error(base, e);
                None
            }
        }
    }

    #[method]
    fn put_double(&self, #[base] base: TRef<Reference>, i: usize, v: f64) -> bool {
        match self.write_memory(i, 8, |s| {
            s.copy_from_slice(&v.to_le_bytes());
            Ok(())
        }) {
            Ok(()) => true,
            Err(e) => {
                self.emit_error(base, e);
                false
            }
        }
    }
}
