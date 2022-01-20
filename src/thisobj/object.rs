use std::mem::transmute;

use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::Trap;

use crate::thisobj::{FuncRegistry, InstanceData};
use crate::wasm_engine::{WasmEngine, WasmModule};
use crate::wasm_externref_godot::{externref_to_object, variant_to_externref};
use crate::{make_funcdef, make_nativeclass};

pub const THISOBJ_OBJECT: &str = "this/object";

make_funcdef! {
    impl ObjectRegistry<Object> [THISOBJ_OBJECT] {
        fn callv(o, a, n) {
            let n: GodotString = externref_to_object(n)?;
            Ok(variant_to_externref(unsafe {
                o.callv(n, externref_to_object(a)?)
            }))
        }

        fn callv_deferred(o, a, n) {
            let n: GodotString = externref_to_object(n)?;
            let a: Vec<_> = externref_to_object::<VariantArray>(a)?.iter().collect();
            unsafe { o.call_deferred(n, &a) };
            Ok(())
        }

        fn add_user_signal(o, n, a) {
            let n: GodotString = externref_to_object(n)?;
            o.add_user_signal(n, externref_to_object(a)?);
            Ok(())
        }

        fn connect(o, n, t, m, b, f) {
            let n: GodotString = externref_to_object(n)?;
            let t: Ref<Object, Shared> = externref_to_object(t)?;
            let m: GodotString = externref_to_object(m)?;
            match o.connect(n, t, m, externref_to_object(b)?, f) {
                Ok(r) => Ok(r),
                Err(e) => Err(Trap::from(Error::new(e))),
            }
        }

        fn disconnect(o, n, t, m) {
            let n: GodotString = externref_to_object(n)?;
            let t: Ref<Object, Shared> = externref_to_object(t)?;
            let m: GodotString = externref_to_object(m)?;
            o.disconnect(n, t, m);
            Ok(())
        }

        fn is_connected(o, n, t, m) {
            let n: GodotString = externref_to_object(n)?;
            let t: Ref<Object, Shared> = externref_to_object(t)?;
            let m: GodotString = externref_to_object(m)?;
            Ok(o.is_connected(n, t, m) as u32)
        }

        fn emit_signal(o, s, a) {
            let s: GodotString = externref_to_object(s)?;
            let a: Vec<_> = externref_to_object::<VariantArray>(a)?.iter().collect();
            o.emit_signal(s, &a);
            Ok(())
        }

        fn get_instance_id(o) {
            Ok(o.get_instance_id())
        }

        fn get_class(o) {
            Ok(variant_to_externref(o.get_class().to_variant()))
        }

        fn get_incoming_connections(o) {
            Ok(variant_to_externref(o.get_incoming_connections().to_variant()))
        }
    }
}

make_nativeclass! {
    impl WasmObject<ObjectRegistry, Object> {}
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
