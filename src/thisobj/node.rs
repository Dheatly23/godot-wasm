use std::marker::PhantomData;
use std::mem::transmute;

use anyhow::Result;
use gdnative::prelude::*;
use wasmtime::{Caller, Linker, Store};

use crate::thisobj::object::ObjectRegistry;
use crate::thisobj::{FuncRegistry, InstanceData};
use crate::wasm_engine::{WasmEngine, WasmModule};
use crate::wasm_externref_godot::{externref_to_object, variant_to_externref};
use crate::wasm_store::call_func;

pub const THISOBJ_NODE: &str = "this/node";

pub struct NodeRegistry<T, F>(F, PhantomData<T>);

impl<T, F> NodeRegistry<T, F>
where
    for<'r> F: Fn(&'r T) -> TRef<'r, Node> + Send + Sync + Copy + 'static,
{
    pub fn new(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<T, F> FuncRegistry<T> for NodeRegistry<T, F>
where
    for<'r> F: Fn(&'r T) -> TRef<'r, Node> + Send + Sync + Copy + 'static,
{
    fn register_linker(&self, store: &mut Store<T>, linker: &mut Linker<T>) -> Result<()> {
        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "queue_free", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            o.queue_free();
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_name", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(variant_to_externref(o.name().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "set_name", move |ctx: Caller<T>, n| {
            let o = f(ctx.data());
            let n: GodotString = externref_to_object(n)?;
            o.set_name(n);
            Ok(())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_owner", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(variant_to_externref(o.owner().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "set_owner", move |ctx: Caller<T>, w| {
            let o = f(ctx.data());
            let w: Ref<Node, Shared> = externref_to_object(w)?;
            o.set_owner(w);
            Ok(())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_tree", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(variant_to_externref(o.get_tree().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_parent", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(variant_to_externref(o.get_parent().to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_child_count", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(o.get_child_count())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_child", move |ctx: Caller<T>, i| {
            let o = f(ctx.data());
            Ok(variant_to_externref(o.get_child(i).to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_index", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            Ok(o.get_index())
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_node", move |ctx: Caller<T>, n| {
            let o = f(ctx.data());
            let n: GodotString = externref_to_object(n)?;
            Ok(variant_to_externref(o.get_node_or_null(n).to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(THISOBJ_NODE, "get_path", move |ctx: Caller<T>| {
            let o = f(ctx.data());
            let p: GodotString = o.get_path().into();
            Ok(variant_to_externref(p.to_variant()))
        })?;

        let f = self.0;
        linker.func_wrap(
            THISOBJ_NODE,
            "add_child",
            move |ctx: Caller<T>, c, b: i32| {
                let o = f(ctx.data());
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
    data: Option<InstanceData<NodeData>>,
}

unsafe impl Send for WasmNode {}
unsafe impl Sync for WasmNode {}

type NodeData = (Instance<WasmEngine, Shared>, Option<TRef<'static, Node>>);

impl WasmNode {
    fn new(_owner: &Node) -> Self {
        Self { data: None }
    }

    fn _guard_section<R>(
        data: &mut InstanceData<NodeData>,
        owner: TRef<Node>,
        f: impl FnOnce(&mut InstanceData<NodeData>) -> R,
    ) -> R {
        data.store.data_mut().1 =
            Some(unsafe { transmute::<TRef<Node>, TRef<'static, Node>>(owner) });
        let ret = f(&mut *data);
        data.store.data_mut().1 = None;
        ret
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
        module: Instance<WasmModule, Shared>,
        #[opt] host_bindings: Option<Dictionary>,
    ) -> Variant {
        self.data = match InstanceData::initialize(
            module.clone(),
            host_bindings,
            (
                unsafe {
                    module
                        .assume_safe()
                        .map(|v, _| v.data.as_ref().expect("Uninitialized!").engine.clone())
                        .unwrap()
                },
                Some(unsafe { transmute::<TRef<Node>, TRef<'static, Node>>(owner) }),
            ),
            |store, linker| {
                NodeRegistry::new(|(_, v): &(_, Option<TRef<Node>>)| {
                    *v.as_ref().expect("No this supplied")
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
            .is_function_exists(&name)
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
            .get_signature(&name)
    }

    /// Call WASM function
    #[export]
    fn call_wasm(&mut self, owner: TRef<Node>, name: String, args: VariantArray) -> Variant {
        let data = self.data.as_mut().expect("Object uninitialized!");
        Self::_guard_section(data, owner, |data| data.call(&name, args))
    }

    #[export]
    fn _ready(&mut self, owner: TRef<Node>) {
        let data = self.data.as_mut().expect("Object uninitialized!");
        if data.is_function_exists("_ready") {
            Self::_guard_section(data, owner, |data| {
                call_func(&mut data.store, &data.inst, "_ready", std::iter::empty())
            });
        }
    }

    #[export]
    fn _process(&mut self, owner: TRef<Node>, v: Variant) {
        let data = self.data.as_mut().expect("Object uninitialized!");
        if data.is_function_exists("_process") {
            Self::_guard_section(data, owner, |data| {
                call_func(
                    &mut data.store,
                    &data.inst,
                    "_process",
                    (&[v]).iter().cloned(),
                )
            });
        }
    }

    #[export]
    fn _physics_process(&mut self, owner: TRef<Node>, v: Variant) {
        let data = self.data.as_mut().expect("Object uninitialized!");
        if data.is_function_exists("_physics_process") {
            Self::_guard_section(data, owner, |data| {
                call_func(
                    &mut data.store,
                    &data.inst,
                    "_physics_process",
                    (&[v]).iter().cloned(),
                )
            });
        }
    }

    #[export]
    fn _enter_tree(&mut self, owner: TRef<Node>) {
        let data = self.data.as_mut().expect("Object uninitialized!");
        if data.is_function_exists("_enter_tree") {
            data.store.data_mut().1 = Some(unsafe { transmute(owner) });
            call_func(
                &mut data.store,
                &data.inst,
                "_enter_tree",
                std::iter::empty(),
            );
            data.store.data_mut().1 = None;
        }
    }

    #[export]
    fn _exit_tree(&mut self, owner: TRef<Node>) {
        let data = self.data.as_mut().expect("Object uninitialized!");
        if data.is_function_exists("_exit_tree") {
            data.store.data_mut().1 = Some(unsafe { transmute(owner) });
            call_func(
                &mut data.store,
                &data.inst,
                "_exit_tree",
                std::iter::empty(),
            );
            data.store.data_mut().1 = None;
        }
    }
}
