use gdnative::prelude::*;

use crate::thisobj::node::{NodeExtra, NodeRegistry};
use crate::thisobj::{InstanceData, StoreData};
use crate::{make_funcdef, make_nativeclass};

pub const THISOBJ_NODE2D: &str = "this/node2d";

make_funcdef! {
    impl Node2DRegistry<Node2D> [THISOBJ_NODE2D] {
        <NodeRegistry>
    }
}

make_nativeclass! {
    #[initialize(NodeExtra, NodeExtra::default())]
    impl WasmNode2D<Node2DRegistry, Node2D> {
        #[export]
        fn _ready(&mut self, owner: TRef<Node2D>) {
            let data = self._get_data();
            if let Some(f) = Self::_get_extra(data)._ready {
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
        fn _process(&mut self, owner: TRef<Node2D>, v: f64) {
            let data = self._get_data();
            if let Some(f) = Self::_get_extra(data)._process {
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
        fn _physics_process(&mut self, owner: TRef<Node2D>, v: f64) {
            let data = self._get_data();
            if let Some(f) = Self::_get_extra(data)._physics_process {
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
        fn _enter_tree(&mut self, owner: TRef<Node2D>) {
            let data = self._get_data();
            if let Some(f) = Self::_get_extra(data)._enter_tree {
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
        fn _exit_tree(&mut self, owner: TRef<Node2D>) {
            let data = self._get_data();
            if let Some(f) = Self::_get_extra(data)._exit_tree {
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

impl WasmNode2D {
    #[inline(always)]
    fn _get_extra(data: &InstanceData<StoreData>) -> &NodeExtra {
        data.store
            .data()
            .extra
            .downcast_ref()
            .expect("Data type mismatch")
    }

    #[inline(always)]
    fn _postinit(&mut self) {
        let data = self._get_data();
        *data
            .store
            .data_mut()
            .extra
            .downcast_mut()
            .expect("Data type mismatch") = NodeExtra {
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
