#[doc(hidden)]
#[macro_export]
macro_rules! make_funcdef {
    (impl $rname:ident <$obj:ty> [$modname:expr] {$(fn $fname:ident($($(mut)? ctx,)? $o:ident $(, $arg:ident $(: $t:ty)?)* $(,)?) $code:block)* $(<$($parent:ident),+>)?}) => {
        #[derive(Default)]
        pub struct $rname();

        impl $crate::thisobj::FuncRegistry<$crate::thisobj::StoreData> for $rname
        {
            fn register_linker(
                &self,
                _store: &mut wasmtime::Store<$crate::thisobj::StoreData>,
                linker: &mut wasmtime::Linker<$crate::thisobj::StoreData>
            ) -> anyhow::Result<()> {
                $(
                    linker.func_wrap(
                        $modname,
                        stringify!($fname),
                        move |ctx: wasmtime::Caller<$crate::thisobj::StoreData> $(, $arg $(: $t)?)*| {
                            let $o: gdnative::TRef<$obj> = ctx.data().try_downcast()?;
                            Ok($code)
                        }
                    )?;
                )*

                $($(
                    $parent::default().register_linker(_store, linker)?;
                )+)?

                Ok(())
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! make_nativeclass {
    (impl $name:ident <$registry:ident, $owner:ident> {$($rest:tt)*}) => {
        impl $name {
            #[inline(always)]
            fn _postinit(&mut self) {
            }
        }

        $crate::make_nativeclass!{
            #[members((), ())]
            impl $name <$registry, $owner> {$($rest)*}
        }
    };
    (#[initialize($extra:ty, $init:expr)] $(#[$($attr:tt)*])* impl $name:ident <$registry:ident, $owner:ident> {$($rest:tt)*}) => {
        $crate::make_nativeclass!{
            $(#[$($attr)*])*
            #[members($extra, $init)]
            impl $name <$registry, $owner> {$($rest)*}
        }
    };
    (#[no_postinit] $(#[$($attr:tt)*])* impl $name:ident <$registry:ident, $owner:ident> {$($rest:tt)*}) => {
        impl $name {
            #[inline(always)]
            fn _postinit(&mut self) {
            }
        }

        $crate::make_nativeclass!{
            $(#[$($attr)*])*
            impl $name <$registry, $owner> {$($rest)*}
        }
    };
    (#[members($extra:ty, $init:expr)] impl $name:ident <$registry:ident, $owner:ident> {$($rest:tt)*}) => {
        #[derive(gdnative::NativeClass)]
        #[inherit($owner)]
        #[register_with(Self::register_properties)]
        #[user_data(gdnative::nativescript::user_data::MutexData<$name>)]
        pub struct $name {
            data: Option<$crate::thisobj::InstanceData>,
        }

        impl $name {
            fn new(_owner: &$owner) -> Self {
                Self {
                    data: None,
                }
            }

            #[inline(always)]
            fn _get_data(&mut self) -> &mut $crate::thisobj::InstanceData {
                self.data.as_mut().expect("Object uninitialized!")
            }

            #[inline(always)]
            fn _guard_section<R>(
                data: &mut $crate::thisobj::InstanceData,
                owner: gdnative::TRef<$owner>,
                f: impl FnOnce(&mut $crate::thisobj::InstanceData) -> R,
            ) -> R {
                data.store.data_mut().set_tref(
                    unsafe { core::mem::transmute::<gdnative::TRef<$owner>, gdnative::TRef<'static, $owner>>(owner) }
                );
                let ret = f(&mut *data);
                data.store.data_mut().clear_tref();
                ret
            }

            /// Register properties
            fn register_properties(builder: &gdnative::nativescript::ClassBuilder<Self>) {
                builder
                    .add_property::<gdnative::nativescript::Instance<$crate::wasm_engine::WasmModule, gdnative::thread_access::Shared>>("module")
                    .with_getter(|this, _| {
                        this.data
                            .as_ref()
                            .expect("Uninitialized!")
                            .module
                            .clone()
                    })
                    .done();
            }
        }

        #[gdnative::methods]
        impl $name {
            #[export]
            #[gdnative::profiled]
            fn initialize(
                &mut self,
                owner: gdnative::TRef<$owner>,
                module: gdnative::nativescript::Instance<$crate::wasm_engine::WasmModule, gdnative::thread_access::Shared>,
                #[opt] host_bindings: Option<gdnative::core_types::Dictionary>,
            ) -> gdnative::core_types::Variant {
                self.data = match $crate::thisobj::InstanceData::initialize(
                    module,
                    $crate::wasm_engine::LinkerCacheIndex::$owner,
                    host_bindings,
                    $crate::thisobj::StoreData::new(
                        unsafe { core::mem::transmute::<gdnative::TRef<$owner>, gdnative::TRef<'static, $owner>>(owner) },
                        $init
                    ),
                    |store, linker| (&$registry::default() as &dyn $crate::thisobj::FuncRegistry<_>).register_linker(store, linker)
                ) {
                    Ok(mut v) => {
                        v.store.data_mut().clear_tref();
                        Some(v)
                    }
                    Err(e) => {
                        gdnative::godot_error!("{}", e);
                        return gdnative::core_types::Variant::new();
                    }
                };
                self._postinit();

                owner.to_variant()
            }

            /// Check if function exists
            #[export]
            fn is_function_exists(&mut self, _owner: &$owner, name: String) -> bool {
                self._get_data().is_function_exists(&name)
            }

            /// Gets exported functions
            #[export]
            fn get_exports(&mut self, _owner: &$owner) -> gdnative::core_types::VariantArray {
                self._get_data().get_exports()
            }

            /// Gets function signature
            #[export]
            fn get_signature(&mut self, _owner: &$owner, name: String) -> gdnative::core_types::Variant {
                self._get_data().get_signature(&name)
            }

            /// Call WASM function
            #[export]
            #[gdnative::profiled]
            fn call_wasm(&mut self, owner: gdnative::TRef<$owner>, name: String, args: gdnative::core_types::VariantArray) -> gdnative::core_types::Variant {
                let data = self._get_data();
                Self::_guard_section(data, owner, |data| data.call(&name, args))
            }

            $($rest)*
        }
    };
}
