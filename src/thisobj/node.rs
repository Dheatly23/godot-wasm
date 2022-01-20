use gdnative::prelude::*;

use crate::thisobj::object::ObjectRegistry;
use crate::wasm_engine::WasmEngine;
use crate::wasm_externref_godot::{externref_to_object, variant_to_externref};
use crate::wasm_store::call_func;
use crate::{make_funcdef, make_nativeclass};

pub const THISOBJ_NODE: &str = "this/node";

make_funcdef! {
    impl NodeRegistry<Node> [THISOBJ_NODE] {
        fn queue_free(o) {
            o.queue_free();
        }

        fn get_name(o) {
            Ok(variant_to_externref(o.name().to_variant()))
        }

        fn set_name(o, n) {
            let n: GodotString = externref_to_object(n)?;
            o.set_name(n);
            Ok(())
        }

        fn get_owner(o) {
            Ok(variant_to_externref(o.owner().to_variant()))
        }

        fn set_owner(o, w) {
            let w: Ref<Node, Shared> = externref_to_object(w)?;
            o.set_owner(w);
            Ok(())
        }

        fn get_tree(o) {
            Ok(variant_to_externref(o.get_tree().to_variant()))
        }

        fn get_parent(o) {
            Ok(variant_to_externref(o.get_parent().to_variant()))
        }

        fn get_child_count(o) {
            Ok(o.get_child_count())
        }

        fn get_child(o, i) {
            Ok(variant_to_externref(o.get_child(i).to_variant()))
        }

        fn get_index(o) {
            Ok(o.get_index())
        }

        fn get_node(o, n) {
            let n: GodotString = externref_to_object(n)?;
            Ok(variant_to_externref(o.get_node_or_null(n).to_variant()))
        }

        fn get_path(o) {
            let p: GodotString = o.get_path().into();
            Ok(variant_to_externref(p.to_variant()))
        }

        fn add_child(o, c, b: i32) {
            let c: Ref<Node, Shared> = externref_to_object(c)?;
            o.add_child(c, b != 0);
            Ok(())
        }

        <ObjectRegistry>
    }
}

make_nativeclass! {
    impl WasmNode<NodeRegistry, Node> {
        #[export]
        fn _ready(&mut self, owner: TRef<Node>) {
            self._maybe_call_func(owner, "_ready", std::iter::empty());
        }

        #[export]
        fn _process(&mut self, owner: TRef<Node>, v: Variant) {
            self._maybe_call_func(owner, "_process", (&[v]).iter().cloned());
        }

        #[export]
        fn _physics_process(&mut self, owner: TRef<Node>, v: Variant) {
            self._maybe_call_func(owner, "_physics_process", (&[v]).iter().cloned());
        }

        #[export]
        fn _enter_tree(&mut self, owner: TRef<Node>) {
            self._maybe_call_func(owner, "_enter_tree", std::iter::empty());
        }

        #[export]
        fn _exit_tree(&mut self, owner: TRef<Node>) {
            self._maybe_call_func(owner, "_enter_tree", std::iter::empty());
        }
    }
}

impl WasmNode {
    #[inline(always)]
    fn _maybe_call_func<I>(&mut self, owner: TRef<Node>, fname: &str, i: I) -> Variant
    where
        I: Iterator<Item = Variant>,
    {
        let data = self._get_data();
        if data.is_function_exists(fname) {
            Self::_guard_section(data, owner, move |data| {
                call_func(&mut data.store, &data.inst, fname, i)
            })
        } else {
            Variant::new()
        }
    }
}
