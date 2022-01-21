use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::Trap;

use crate::wasm_externref_godot::{externref_to_object, variant_to_externref};
use crate::{make_funcdef, make_nativeclass};

pub const THISOBJ_OBJECT: &str = "this/object";

make_funcdef! {
    impl ObjectRegistry<Object> [THISOBJ_OBJECT] {
        fn callv(o, a, n) {
            let n: GodotString = externref_to_object(n)?;
            variant_to_externref(unsafe {
                o.callv(n, externref_to_object(a)?)
            })
        }

        fn callv_deferred(o, a, n) {
            let n: GodotString = externref_to_object(n)?;
            let a: Vec<_> = externref_to_object::<VariantArray>(a)?.iter().collect();
            unsafe { o.call_deferred(n, &a) };
        }

        fn add_user_signal(o, n, a) {
            let n: GodotString = externref_to_object(n)?;
            o.add_user_signal(n, externref_to_object(a)?);
        }

        fn connect(o, n, t, m, b, f) {
            let n: GodotString = externref_to_object(n)?;
            let t: Ref<Object, Shared> = externref_to_object(t)?;
            let m: GodotString = externref_to_object(m)?;
            match o.connect(n, t, m, externref_to_object(b)?, f) {
                Ok(r) => r,
                Err(e) => return Err(Trap::from(Error::new(e))),
            }
        }

        fn disconnect(o, n, t, m) {
            let n: GodotString = externref_to_object(n)?;
            let t: Ref<Object, Shared> = externref_to_object(t)?;
            let m: GodotString = externref_to_object(m)?;
            o.disconnect(n, t, m);
        }

        fn is_connected(o, n, t, m) {
            let n: GodotString = externref_to_object(n)?;
            let t: Ref<Object, Shared> = externref_to_object(t)?;
            let m: GodotString = externref_to_object(m)?;
            o.is_connected(n, t, m) as u32
        }

        fn emit_signal(o, s, a) {
            let s: GodotString = externref_to_object(s)?;
            let a: Vec<_> = externref_to_object::<VariantArray>(a)?.iter().collect();
            o.emit_signal(s, &a);
        }

        fn get_instance_id(o) {
            o.get_instance_id()
        }

        fn get_class(o) {
            variant_to_externref(o.get_class().to_variant())
        }

        fn get_incoming_connections(o) {
            variant_to_externref(o.get_incoming_connections().to_variant())
        }
    }
}

make_nativeclass! {
    impl WasmObject<ObjectRegistry, Object> {}
}

make_nativeclass! {
    impl WasmReference<ObjectRegistry, Reference> {}
}
