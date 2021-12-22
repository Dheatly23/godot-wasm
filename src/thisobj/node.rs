use std::marker::PhantomData;

use anyhow::Result;
use gdnative::prelude::*;
use wasmtime::{Caller, Linker, Store};

use crate::thisobj::object::ObjectRegistry;
use crate::thisobj::{FuncRegistry, InstanceData};
use crate::wasm_engine::WasmEngine;
use crate::wasm_externref_godot::{externref_to_object, variant_to_externref};

pub const THISOBJ_NODE: &str = "this/node";

pub struct NodeRegistry<T, F>(F, PhantomData<T>);

impl<T, F> NodeRegistry<T, F>
where
    for<'r> F: Fn(&'r mut T) -> TRef<'r, Node, Unique> + Send + Sync + Copy + 'static,
{
    pub fn new(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<T, F> FuncRegistry<T> for NodeRegistry<T, F>
where
    for<'r> F: Fn(&'r mut T) -> TRef<'r, Node, Unique> + Send + Sync + Copy + 'static,
{
    fn register_linker(&self, store: &mut Store<T>, linker: &mut Linker<T>) -> Result<()> {
        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_name", move |mut ctx: Caller<T>| {
            let o = f(ctx.data_mut());
            Ok(variant_to_externref(o.name().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "set_name", move |mut ctx: Caller<T>, n| {
            let o = f(ctx.data_mut());
            let n: GodotString = externref_to_object(n)?;
            o.set_name(n);
            Ok(())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_owner", move |mut ctx: Caller<T>| {
            let o = f(ctx.data_mut());
            Ok(variant_to_externref(o.owner().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "set_owner", move |mut ctx: Caller<T>, w| {
            let o = f(ctx.data_mut());
            let w: Ref<Node, Shared> = externref_to_object(w)?;
            o.set_owner(w);
            Ok(())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_tree", move |mut ctx: Caller<T>| {
            let o = f(ctx.data_mut());
            Ok(variant_to_externref(o.get_tree().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_parent", move |mut ctx: Caller<T>| {
            let o = f(ctx.data_mut());
            Ok(variant_to_externref(o.get_parent().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_NODE,
            "get_child_count",
            move |mut ctx: Caller<T>| {
                let o = f(ctx.data_mut());
                Ok(o.get_child_count())
            },
        )?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_child", move |mut ctx: Caller<T>, i| {
            let o = f(ctx.data_mut());
            Ok(variant_to_externref(o.get_child(i).to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_index", move |mut ctx: Caller<T>| {
            let o = f(ctx.data_mut());
            Ok(o.get_index())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_node", move |mut ctx: Caller<T>, n| {
            let o = f(ctx.data_mut());
            let n: GodotString = externref_to_object(n)?;
            Ok(variant_to_externref(o.get_node_or_null(n).to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_path", move |mut ctx: Caller<T>| {
            let o = f(ctx.data_mut());
            let p: GodotString = o.get_path().into();
            Ok(variant_to_externref(p.to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_NODE,
            "add_child",
            move |mut ctx: Caller<T>, c, b: i32| {
                let o = f(ctx.data_mut());
                let c: Ref<Node, Shared> = externref_to_object(c)?;
                o.add_child(c, b != 0);
                Ok(())
            },
        )?;

        let f = self.0;
        ObjectRegistry::new(move |v| f(v).upcast()).register_linker(store, linker)
    }
}

#[derive(NativeClass)]
#[inherit(Node)]
#[register_with(Self::register_properties)]
#[user_data(gdnative::nativescript::user_data::MutexData<WasmNode>)]
pub struct WasmNode {
    data: Option<InstanceData<(Instance<WasmEngine, Shared>, Option<Ref<Node, Unique>>)>>,
}

impl WasmNode {
    fn new(_owner: &Node) -> Self {
        Self { data: None }
    }
}

#[methods]
impl WasmNode {
    /// Register properties
    fn register_properties(builder: &ClassBuilder<Self>) {
        builder
            .add_property::<Instance<WasmEngine, Shared>>("engine")
            .with_getter(|this, _| {
                this.data
                    .as_ref()
                    .expect("Object uninitialized!")
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
        owner: TRef<Node>,
        engine: Instance<WasmEngine, Shared>,
        name: String,
        #[opt] host_bindings: Option<Dictionary>,
    ) -> Variant {
        self.data = match InstanceData::initialize(
            engine.clone(),
            &name,
            host_bindings,
            (engine, Some(unsafe { owner.claim().assume_unique() })),
            |store, linker| {
                NodeRegistry::new(|(_, v): &mut (_, Option<Ref<Node, Unique>>)| {
                    v.as_ref().expect("No this supplied").as_ref()
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
    fn is_function_exists(&mut self, _owner: &Node, name: String) -> bool {
        self.data
            .as_mut()
            .expect("Object uninitialized!")
            .is_function_exists(name)
    }

    /// Gets exported functions
    #[export]
    fn get_exports(&mut self, _owner: &Node) -> VariantArray {
        self.data
            .as_mut()
            .expect("Object uninitialized!")
            .get_exports()
    }

    /// Gets function signature
    #[export]
    fn get_signature(&mut self, _owner: &Node, name: String) -> Variant {
        self.data
            .as_mut()
            .expect("Object uninitialized!")
            .get_signature(name)
    }

    /// Call WASM function
    #[export]
    fn call_wasm(&mut self, owner: TRef<Node>, name: String, args: VariantArray) -> Variant {
        let data = self.data.as_mut().expect("Object uninitialized!");
        data.store.data_mut().1 = Some(unsafe { owner.claim().assume_unique() });
        let ret = data.call(name, args);
        data.store.data_mut().1 = None;
        ret
    }
}
