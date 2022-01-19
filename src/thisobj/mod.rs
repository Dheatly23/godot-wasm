pub mod node;
pub mod object;

use std::iter::FromIterator;
use std::mem::transmute;

use anyhow::{bail, Result};
use gdnative::prelude::*;
use wasmtime::{Engine, Linker, Store};

use crate::wasm_engine::{ModuleData, WasmModule};
use crate::wasm_externref_godot::register_godot_externref;
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

pub struct InstanceData<T: 'static> {
    pub(crate) store: Store<T>,
    pub(crate) inst: wasmtime::Instance,
}

impl<T: 'static> InstanceData<T> {
    /// Initialize instance data
    pub fn initialize<Fr>(
        module: Instance<WasmModule, Shared>,
        host_bindings: Option<Dictionary>,
        t: T,
        register: Fr,
    ) -> Result<Self>
    where
        Fr: FnOnce(&mut Store<T>, &mut Linker<T>) -> Result<()>,
    {
        match unsafe { module.assume_safe() }.map(move |v, _| -> Result<_> {
            let ModuleData {
                engine,
                module,
                deps,
            } = match v.data.as_ref() {
                Some(v) => v,
                None => bail!("Uninitialized!"),
            };

            // SAFETY: This reference lifetime is smaller than engine object lifetime
            let engine: &Engine = unsafe {
                match engine
                    .assume_safe()
                    .map(|e, _| transmute::<&Engine, &Engine>(&e.engine))
                {
                    Ok(v) => v,
                    Err(_) => unreachable!(),
                }
            };

            let mut store = Store::new(&*engine, t);
            let mut linker = Linker::new(&*engine);

            register_godot_externref(&mut linker)?;
            register(&mut store, &mut linker)?;
            if let Some(host_bindings) = host_bindings {
                create_hostmap(host_bindings)?.register_linker(&mut store, &mut linker)?;
            }
            DepLoader { deps: &deps }.register_linker(&mut store, &mut linker)?;

            let inst = linker.instantiate(&mut store, module)?;

            Ok(Self { store, inst })
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
