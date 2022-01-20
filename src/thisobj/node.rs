use gdnative::prelude::*;
use wasmtime::TypedFunc;

use crate::thisobj::object::ObjectRegistry;
use crate::wasm_engine::WasmEngine;
use crate::wasm_externref_godot::{externref_to_object, variant_to_externref};
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

        fn get_viewport(o) {
            Ok(variant_to_externref(o.get_viewport().to_variant()))
        }

        fn get_parent(o) {
            Ok(variant_to_externref(o.get_parent().to_variant()))
        }

        fn is_parent_of(o, n) {
            let n: Ref<Node> = externref_to_object(n)?;
            Ok(o.is_a_parent_of(n) as i32)
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

        fn move_child(o, c, p: i64) {
            let c: Ref<Node, Shared> = externref_to_object(c)?;
            o.move_child(c, p);
            Ok(())
        }

        fn remove_child(o, c) {
            let c: Ref<Node, Shared> = externref_to_object(c)?;
            o.remove_child(c);
            Ok(())
        }

        fn get_groups(o) {
            variant_to_externref(o.get_groups().to_variant())
        }

        fn add_to_group(o, g, p: i32) {
            let g: GodotString = externref_to_object(g)?;
            o.add_to_group(g, p != 0);
            Ok(())
        }

        fn remove_from_group(o, g) {
            let g: GodotString = externref_to_object(g)?;
            o.remove_from_group(g);
            Ok(())
        }

        fn is_in_group(o, g) {
            let g: GodotString = externref_to_object(g)?;
            Ok(o.is_in_group(g) as i32)
        }

        fn can_process(o) {
            o.can_process() as i32
        }

        fn get_pause_mode(o) {
            o.pause_mode().0 as i32
        }

        fn set_pause_mode(o, p: i32) {
            o.set_pause_mode(p as i64);
        }

        fn is_processing(o) {
            o.is_processing() as i32
        }

        fn set_process(o, p: i32) {
            o.set_process(p != 0)
        }

        fn is_physics_processing(o) {
            o.is_physics_processing() as i32
        }

        fn set_physics_process(o, p: i32) {
            o.set_physics_process(p != 0)
        }

        fn is_processing_input(o) {
            o.is_processing_input() as i32
        }

        fn set_process_input(o, p: i32) {
            o.set_process_input(p != 0)
        }

        fn is_processing_unhandled_input(o) {
            o.is_processing_unhandled_input() as i32
        }

        fn set_process_unhandled_input(o, p: i32) {
            o.set_process_unhandled_input(p != 0)
        }

        fn is_processing_unhandled_key_input(o) {
            o.is_processing_unhandled_key_input() as i32
        }

        fn set_process_unhandled_key_input(o, p: i32) {
            o.set_process_unhandled_key_input(p != 0)
        }

        <ObjectRegistry>
    }
}

#[derive(Default)]
pub struct NodeExtra {
    _enter_tree: Option<TypedFunc<(), ()>>,
    _exit_tree: Option<TypedFunc<(), ()>>,
    _ready: Option<TypedFunc<(), ()>>,
    _process: Option<TypedFunc<(f64,), ()>>,
    _physics_process: Option<TypedFunc<(f64,), ()>>,
}

make_nativeclass! {
    #[initialize(NodeExtra, NodeExtra::default())]
    impl WasmNode<NodeRegistry, Node> {
        #[export]
        fn _ready(&mut self, owner: TRef<Node>) {
            let data = self._get_data();
            if let Some(f) = data.store.data_mut().2._ready {
                Self::_guard_section(
                    data,
                    owner,
                    |data| match f.call(&mut data.store, ()) {
                        Ok(()) => (),
                        Err(e) => godot_error!("{}", e),
                    }
                )
            }
        }

        #[export]
        fn _process(&mut self, owner: TRef<Node>, v: f64) {
            let data = self._get_data();
            if let Some(f) = data.store.data_mut().2._process {
                Self::_guard_section(
                    data,
                    owner,
                    |data| match f.call(&mut data.store, (v,)) {
                        Ok(()) => (),
                        Err(e) => godot_error!("{}", e),
                    }
                )
            }
        }

        #[export]
        fn _physics_process(&mut self, owner: TRef<Node>, v: f64) {
            let data = self._get_data();
            if let Some(f) = data.store.data_mut().2._physics_process {
                Self::_guard_section(
                    data,
                    owner,
                    |data| match f.call(&mut data.store, (v,)) {
                        Ok(()) => (),
                        Err(e) => godot_error!("{}", e),
                    }
                )
            }
        }

        #[export]
        fn _enter_tree(&mut self, owner: TRef<Node>) {
            let data = self._get_data();
            if let Some(f) = data.store.data_mut().2._enter_tree {
                Self::_guard_section(
                    data,
                    owner,
                    |data| match f.call(&mut data.store, ()) {
                        Ok(()) => (),
                        Err(e) => godot_error!("{}", e),
                    }
                )
            }
        }

        #[export]
        fn _exit_tree(&mut self, owner: TRef<Node>) {
            let data = self._get_data();
            if let Some(f) = data.store.data_mut().2._exit_tree {
                Self::_guard_section(
                    data,
                    owner,
                    |data| match f.call(&mut data.store, ()) {
                        Ok(()) => (),
                        Err(e) => godot_error!("{}", e),
                    }
                )
            }
        }
    }
}

impl WasmNode {
    #[inline(always)]
    fn _postinit(&mut self) {
        let data = self._get_data();
        data.store.data_mut().2 = NodeExtra {
            _enter_tree: data
                .inst
                .get_typed_func(&mut data.store, "_enter_tree")
                .ok(),
            _exit_tree: data.inst.get_typed_func(&mut data.store, "_exit_tree").ok(),
            _ready: data.inst.get_typed_func(&mut data.store, "_ready").ok(),
            _process: data.inst.get_typed_func(&mut data.store, "_process").ok(),
            _physics_process: data
                .inst
                .get_typed_func(&mut data.store, "_physics_process")
                .ok(),
        };
    }
}
