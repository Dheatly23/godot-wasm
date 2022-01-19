use std::marker::PhantomData;
use std::mem::transmute;

use anyhow::{Error, Result};
use gdnative::prelude::*;
use wasmtime::{Caller, Linker, Store, Trap};

use crate::thisobj::{FuncRegistry, InstanceData};
use crate::wasm_engine::{WasmEngine, WasmModule};
use crate::wasm_externref_godot::{externref_to_object, variant_to_externref};

pub const THISOBJ_OBJECT: &str = "this/object";

pub struct ObjectRegistry<T, F>(F, PhantomData<T>);

impl<T, F> ObjectRegistry<T, F>
where
    for<'r> F: Fn(&'r T) -> TRef<'r, Object> + Send + Sync + Copy + 'static,
{
    pub fn new(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<T, F> FuncRegistry<T> for ObjectRegistry<T, F>
where
    for<'r> F: Fn(&'r T) -> TRef<'r, Object> + Send + Sync + Copy + 'static,
{
    fn register_linker(&self, _store: &mut Store<T>, linker: &mut Linker<T>) -> Result<()> {
        let f = self.0;
        linker.func_wrap(THISOBJ_OBJECT, "callv", move |ctx: Caller<T>, a, n| {
            let o = f(ctx.data());
            let n: GodotString = externref_to_object(n)?;
            Ok(variant_to_externref(unsafe {
                o.callv(n, externref_to_object(a)?)
            }))
        })?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_OBJECT,
            "callv_deferred",
            move |ctx: Caller<T>, a, n| {
                let o = f(ctx.data());
                let n: GodotString = externref_to_object(n)?;
                let a: Vec<_> = externref_to_object::<VariantArray>(a)?.iter().collect();
                unsafe { o.call_deferred(n, &a) };
                Ok(())
            },
        )?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_OBJECT,
            "add_user_signal",
            move |ctx: Caller<T>, n, a| {
                let o = f(ctx.data());
                let n: GodotString = externref_to_object(n)?;
                o.add_user_signal(n, externref_to_object(a)?);
                Ok(())
            },
        )?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_OBJECT,
            "connect",
            move |ctx: Caller<T>, n, t, m, b, f_| {
                let o = f(ctx.data());
                let n: GodotString = externref_to_object(n)?;
                let t: Ref<Object, Shared> = externref_to_object(t)?;
                let m: GodotString = externref_to_object(m)?;
                o.connect(n, t, m, externref_to_object(b)?, f_)
                    .map_err(|e| Trap::from(Error::new(e)))
            },
        )?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_OBJECT,
            "disconnect",
            move |ctx: Caller<T>, n, t, m| {
                let o = f(ctx.data());
                let n: GodotString = externref_to_object(n)?;
                let t: Ref<Object, Shared> = externref_to_object(t)?;
                let m: GodotString = externref_to_object(m)?;
                o.disconnect(n, t, m);
                Ok(())
            },
        )?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_OBJECT,
            "is_connected",
            move |ctx: Caller<T>, n, t, m| {
                let o = f(ctx.data());
                let n: GodotString = externref_to_object(n)?;
                let t: Ref<Object, Shared> = externref_to_object(t)?;
                let m: GodotString = externref_to_object(m)?;
                Ok(o.is_connected(n, t, m) as u32)
            },
        )?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_OBJECT,
            "emit_signal",
            move |ctx: Caller<T>, s, a| {
                let o = f(ctx.data());
                let s: GodotString = externref_to_object(s)?;
                let a: Vec<_> = externref_to_object::<VariantArray>(a)?.iter().collect();
                o.emit_signal(s, &a);
                Ok(())
            },
        )?;

        let f = self.0;
        linker.func_wrap(THISOBJ_OBJECT, "get_instance_id", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(o.get_instance_id())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_OBJECT, "get_class", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(variant_to_externref(o.get_class().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_OBJECT,
            "get_incoming_connections",
            move |ctx: Caller<T>| {
                let o = f(ctx.data());
                Ok(variant_to_externref(
                    o.get_incoming_connections().to_variant(),
                ))
            },
        )?;

        Ok(())
    }
}

#[derive(NativeClass)]
#[inherit(Reference)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::nativescript::user_data::MutexData<WasmReference>)]
pub struct WasmReference {
    data: Option<
        InstanceData<(
            Instance<WasmEngine, Shared>,
            Option<TRef<'static, Reference>>,
        )>,
    >,
}

unsafe impl Send for WasmReference {}
unsafe impl Sync for WasmReference {}

impl WasmReference {
    fn new(_owner: &Reference) -> Self {
        Self { data: None }
    }
}

#[methods]
impl WasmReference {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .add_property::<Instance<WasmEngine, Shared>>("engine")
            .with_getter(|this, _| {
                this.data
                    .as_ref()
                    .expect("Uninitialized!")
                    .store
                    .data()
                    .0
                    .clone()
            })
            .done();
    }

    #[export]
    fn initialize(
        &mut self,
        owner: TRef<Reference>,
        module: Instance<WasmModule, Shared>,
        #[opt] host_bindings: Option<Dictionary>,
    ) -> Variant {
        self.data = match InstanceData::initialize(
            module.clone(),
            host_bindings,
            (
                unsafe {
                    match module
                        .assume_safe()
                        .map(|v, _| v.data.as_ref().expect("Uninitialized!").engine.clone())
                    {
                        Ok(x) => x,
                        Err(e) => {
                            godot_error!("{}", e);
                            return Variant::new();
                        }
                    }
                },
                Some(unsafe { transmute::<TRef<Reference>, TRef<'static, Reference>>(owner) }),
            ),
            |store, linker| {
                ObjectRegistry::new(|(_, v): &(_, Option<TRef<Reference>>)| {
                    v.as_ref().expect("No this supplied").upcast()
                })
                .register_linker(store, linker)
            },
        ) {
            Ok(mut v) => {
                v.store.data_mut().1 = None;
                Some(v)
            }
            Err(e) => {
                godot_error!("{}", e);
                return Variant::new();
            }
        };

        owner.to_variant()
    }

    /// Check if function exists
    #[export]
    fn is_function_exists(&mut self, _owner: &Reference, name: String) -> bool {
        self.data
            .as_mut()
            .expect("Object uninitialized!")
            .is_function_exists(&name)
    }

    /// Gets exported functions
    #[export]
    fn get_exports(&mut self, _owner: &Reference) -> VariantArray {
        self.data
            .as_mut()
            .expect("Object uninitialized!")
            .get_exports()
    }

    /// Gets function signature
    #[export]
    fn get_signature(&mut self, _owner: &Reference, name: String) -> Variant {
        self.data
            .as_mut()
            .expect("Object uninitialized!")
            .get_signature(&name)
    }

    /// Call WASM function
    #[export]
    fn call_wasm(&mut self, owner: TRef<Reference>, name: String, args: VariantArray) -> Variant {
        let data = self.data.as_mut().expect("Object uninitialized!");
        data.store.data_mut().1 =
            Some(unsafe { transmute::<TRef<Reference>, TRef<'static, Reference>>(owner) });
        let ret = data.call(&name, args);
        data.store.data_mut().1 = None;
        ret
    }
}
