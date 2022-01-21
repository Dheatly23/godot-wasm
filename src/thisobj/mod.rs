mod helper_macros;
pub mod node;
pub mod node2d;
pub mod object;

use std::any::Any;
use std::iter::FromIterator;

use anyhow::{bail, Result};
use gdnative::prelude::*;
use wasmtime::{Linker, Store, Trap};

use crate::wasm_engine::{LinkerCacheIndex, ModuleData, WasmModule};
use crate::wasm_store::{call_func, create_hostmap, from_signature, register_hostmap, HostMap};

pub trait FuncRegistry<T> {
    fn register_linker(&self, store: &mut Store<T>, linker: &mut Linker<T>) -> Result<()>;
}

impl<T> FuncRegistry<T> for HostMap {
    fn register_linker(&self, _store: &mut Store<T>, linker: &mut Linker<T>) -> Result<()> {
        register_hostmap(linker, self)
    }
}

pub struct DepLoader<'a> {
    pub deps: &'a [Instance<WasmModule, Shared>],
}

impl<'a, T: 'static> FuncRegistry<T> for DepLoader<'a> {
    fn register_linker(&self, store: &mut Store<T>, linker: &mut Linker<T>) -> Result<()> {
        for i in self.deps {
            // SAFETY: Dependency is assumed to be valid
            let i = unsafe { i.assume_safe() };
            match i.map(|i, _| match i.data {
                Some(ModuleData { ref module, .. }) => {
                    linker.module(&mut *store, module.name().unwrap(), module)
                }
                None => bail!("Uninitialized!"),
            }) {
                Ok(v) => v,
                Err(_) => unreachable!(),
            }?;
        }
        Ok(())
    }
}

pub struct InstanceData {
    pub(crate) module: Instance<WasmModule, Shared>,
    pub(crate) store: Store<StoreData>,
    pub(crate) inst: wasmtime::Instance,
}

impl InstanceData {
    /// Initialize instance data
    pub(crate) fn initialize<Fr>(
        module: Instance<WasmModule, Shared>,
        linker_index: LinkerCacheIndex,
        host_bindings: Option<Dictionary>,
        t: StoreData,
        register: Fr,
    ) -> Result<Self>
    where
        Fr: FnOnce(&mut Store<StoreData>, &mut Linker<StoreData>) -> Result<()>,
    {
        let m = module.clone();
        match unsafe { module.assume_safe() }.map(move |v, _| -> Result<_> {
            let ModuleData {
                engine,
                module,
                deps,
            } = match v.data.as_ref() {
                Some(v) => v,
                None => bail!("Uninitialized!"),
            };

            let (mut store, mut linker) = match unsafe { engine.assume_safe() }.map(move |e, _| {
                let mut store = Store::new(&e.engine, t);
                let linker = e.get_linker_cache(linker_index, || {
                    let mut linker = e.get_default_linker_cache();
                    register(&mut store, &mut linker).unwrap();
                    linker
                });
                (store, linker)
            }) {
                Ok(v) => v,
                Err(e) => bail!("{}", e),
            };

            if let Some(host_bindings) = host_bindings {
                create_hostmap(host_bindings)?.register_linker(&mut store, &mut linker)?;
            }
            DepLoader { deps: &deps }.register_linker(&mut store, &mut linker)?;

            let inst = linker.instantiate(&mut store, module)?;

            Ok(Self {
                module: m,
                store,
                inst,
            })
        }) {
            Ok(Ok(v)) => Ok(v),
            Err(e) => bail!("{}", e),
            Ok(Err(e)) => Err(e),
        }
    }

    /// Check if function exists
    pub fn is_function_exists(&mut self, name: &str) -> bool {
        self.inst.get_func(&mut self.store, name).is_some()
    }

    /// Gets exported functions
    fn get_exports(&mut self) -> VariantArray {
        VariantArray::from_iter(self.inst.exports(&mut self.store).filter_map(|v| {
            let ret = GodotString::from(v.name()).to_variant();
            v.into_func().map(|_| ret)
        }))
        .into_shared()
    }

    /// Gets function signature
    fn get_signature(&mut self, name: &str) -> Variant {
        let f = match self.inst.get_func(&mut self.store, name) {
            Some(v) => v,
            None => {
                godot_error!("No function named {}", name);
                return Variant::new();
            }
        };

        match from_signature(f.ty(&mut self.store)) {
            Ok((p, r)) => {
                let d = Dictionary::new();
                d.insert(GodotString::from_str("params"), p);
                d.insert(GodotString::from_str("results"), r);
                d.owned_to_variant()
            }
            Err(e) => {
                godot_error!("{}", e);
                Variant::new()
            }
        }
    }

    #[inline(always)]
    pub fn call(&mut self, name: &str, args: VariantArray) -> Variant {
        call_func(&mut self.store, &self.inst, name, args.iter())
    }
}

pub struct StoreData {
    pub(crate) tref: Option<TRef<'static, Object>>,
    pub extra: Box<dyn Any + Send + Sync>,
}

unsafe impl Send for StoreData {}
unsafe impl Sync for StoreData {}

impl StoreData {
    pub fn new<T, E: Any + Send + Sync>(tref: TRef<'static, T>, extra: E) -> Self
    where
        T: GodotObject + SubClass<Object>,
    {
        Self {
            tref: Some(tref.upcast()),
            extra: Box::new(extra),
        }
    }

    pub fn set_tref<T>(&mut self, tref: TRef<'static, T>)
    where
        T: GodotObject + SubClass<Object>,
    {
        self.tref = Some(tref.upcast());
    }

    pub fn clear_tref(&mut self) {
        self.tref = None;
    }

    pub fn try_downcast<T>(&'_ self) -> std::result::Result<TRef<'_, T>, Trap>
    where
        T: GodotObject + SubClass<Object>,
    {
        match self.tref {
            Some(t) => match t.cast() {
                Some(t) => Ok(t),
                None => Err(Trap::new("Cannot cast this")),
            },
            None => Err(Trap::new("No this provided")),
        }
    }

    pub fn cast_extra_ref<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.extra.downcast_ref()
    }

    pub fn cast_extra_mut<T: Any + Send + Sync>(&mut self) -> Option<&mut T> {
        self.extra.downcast_mut()
    }
}
