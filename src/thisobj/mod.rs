pub mod node;
pub mod object;

use std::iter::FromIterator;

use anyhow::{bail, Result};
use gdnative::prelude::*;
use wasmtime::{Linker, Store};

use crate::wasm_engine::{ModuleData, WasmEngine};
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
    pub engine: &'a WasmEngine,
    pub name: &'a str,
}

impl<'a, T: 'static> FuncRegistry<T> for DepLoader<'a> {
    fn register_linker(&self, store: &mut Store<T>, linker: &mut Linker<T>) -> Result<()> {
        let WasmEngine { modules, .. } = self.engine;
        let modules = modules.read();
        let deps = match modules.get(self.name) {
            Some(ModuleData { deps, .. }) => deps,
            None => bail!("No module named {}", self.name),
        };
        let mut it = modules.iter();
        let mut prev = 0;
        for &i in deps.iter() {
            match it.nth(i - prev) {
                Some((k, x)) => linker.module(&mut *store, k, &x.module)?,
                None => unreachable!("Iterator overrun"),
            };
            prev = i;
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
        engine: Instance<WasmEngine, Shared>,
        name: &str,
        host_bindings: Option<Dictionary>,
        t: T,
        register: Fr,
    ) -> Result<Self>
    where
        Fr: FnOnce(&mut Store<T>, &mut Linker<T>) -> Result<()>,
    {
        match unsafe { engine.assume_safe() }.map(move |v, _| -> Result<_> {
            let WasmEngine { engine, modules } = &*v;

            let mut store = Store::new(&engine, t);
            let mut linker = Linker::new(&engine);

            register_godot_externref(&mut linker)?;
            register(&mut store, &mut linker)?;
            if let Some(host_bindings) = host_bindings {
                create_hostmap(host_bindings)?.register_linker(&mut store, &mut linker)?;
            }
            DepLoader {
                engine: v,
                name: &*name,
            }
            .register_linker(&mut store, &mut linker)?;

            let modules = modules.read();

            let ModuleData { module, .. } = match modules.get(name) {
                Some(v) => v,
                None => bail!("No module named {}", name),
            };

            let inst = linker.instantiate(&mut store, module)?;

            Ok(Self { store, inst })
        }) {
            Ok(Ok(v)) => Ok(v),
            Err(e) => bail!("{}", e),
            Ok(Err(e)) => Err(e),
        }
    }

    /// Check if function exists
    pub fn is_function_exists(&mut self, name: String) -> bool {
        self.inst.get_func(&mut self.store, &name).is_some()
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
    fn get_signature(&mut self, name: String) -> Variant {
        let f = match self.inst.get_func(&mut self.store, &name) {
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
    pub fn call(&mut self, name: String, args: VariantArray) -> Variant {
        call_func(&mut self.store, &self.inst, name, args.iter())
    }
}
