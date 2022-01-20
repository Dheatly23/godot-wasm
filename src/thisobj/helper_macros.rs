#[macro_export]
macro_rules! make_funcdef {
    (impl $rname:ident <$obj:ty> [$modname:expr] {$(fn $fname:ident($($(mut)? ctx,)? $o:ident $(, $arg:ident $(: $t:ty)?)* $(,)?) $( -> $ret:ty)? $code:block)* $(<$($parent:ident),+>)?}) => {
        pub struct $rname<T, F>(F, core::marker::PhantomData<T>);

        impl<T, F> $rname<T, F>
        where
            for<'r> F: Fn(&'r T) -> gdnative::TRef<'r, $obj> + Send + Sync + Copy + 'static,
        {
            pub fn new(f: F) -> Self {
                Self(f, core::marker::PhantomData)
            }
        }

        impl<T, F> $crate::thisobj::FuncRegistry<T> for $rname<T, F>
        where
            for<'r> F: Fn(&'r T) -> gdnative::TRef<'r, $obj> + Send + Sync + Copy + 'static,
        {
            fn register_linker(&self, _store: &mut wasmtime::Store<T>, linker: &mut wasmtime::Linker<T>) -> anyhow::Result<()> {
                $({
                    let $o = self.0;
                    linker.func_wrap(
                        $modname,
                        stringify!($fname),
                        move |ctx: wasmtime::Caller<T> $(, $arg $(: $t)?)*| $( -> $ret)? {
                            let $o = $o(ctx.data());
                            $code
                        }
                    )?;
                })*

                $($({
                    let f = self.0;
                    $parent::new(move |v| f(v).upcast()).register_linker(_store, linker)?;
                })+)?

                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! make_nativeclass {
    (impl $name:ident <$registry:ident, $owner:ty> {$($rest:tt)*}) => {
        impl $name {
            #[inline(always)]
            fn _postinit(&mut self) {
            }
        }

        $crate::make_nativeclass!{
            #[members()]
            impl $name <$registry, $owner> {$($rest)*}
        }
    };
    (#[initialize($extra:ty, $init:expr)] $(#[$($attr:tt)*])* impl $name:ident <$registry:ident, $owner:ty> {$($rest:tt)*}) => {
        $crate::make_nativeclass!{
            $(#[$($attr)*])*
            #[members($extra, $init)]
            impl $name <$registry, $owner> {$($rest)*}
        }
    };
    (#[no_postinit] $(#[$($attr:tt)*])* impl $name:ident <$registry:ident, $owner:ty> {$($rest:tt)*}) => {
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
    (#[members($($extra:ty, $init:expr)?)] impl $name:ident <$registry:ident, $owner:ty> {$($rest:tt)*}) => {
        #[derive(gdnative::NativeClass)]
        #[inherit($owner)]
        #[register_with(Self::register_properties)]
        #[user_data(gdnative::nativescript::user_data::MutexData<$name>)]
        pub struct $name {
            data: Option<
                $crate::thisobj::InstanceData<(
                    gdnative::nativescript::Instance<$crate::wasm_engine::WasmEngine, gdnative::thread_access::Shared>,
                    Option<gdnative::TRef<'static, $owner>>,
                    $($extra,)?
                )>,
            >,
        }

        unsafe impl Send for $name {}
        unsafe impl Sync for $name {}

        impl $name {
            fn new(_owner: &$owner) -> Self {
                Self {
                    data: None,
                }
            }

            #[inline(always)]
            fn _get_data(&mut self) -> &mut $crate::thisobj::InstanceData<(
                gdnative::nativescript::Instance<$crate::wasm_engine::WasmEngine, gdnative::thread_access::Shared>,
                Option<gdnative::TRef<'static, $owner>>,
                $($extra)?
            )> {
                self.data.as_mut().expect("Object uninitialized!")
            }

            #[inline(always)]
            fn _guard_section<R>(
                data: &mut $crate::thisobj::InstanceData<(
                    gdnative::nativescript::Instance<$crate::wasm_engine::WasmEngine, gdnative::thread_access::Shared>,
                    Option<gdnative::TRef<'static, $owner>>,
                    $($extra)?
                )>,
                owner: gdnative::TRef<$owner>,
                f: impl FnOnce(&mut $crate::thisobj::InstanceData<(
                    gdnative::nativescript::Instance<$crate::wasm_engine::WasmEngine, gdnative::thread_access::Shared>,
                    Option<gdnative::TRef<'static, $owner>>,
                    $($extra)?
                )>) -> R,
            ) -> R {
                data.store.data_mut().1 =
                    Some(unsafe { core::mem::transmute::<gdnative::TRef<$owner>, gdnative::TRef<'static, $owner>>(owner) });
                let ret = f(&mut *data);
                data.store.data_mut().1 = None;
                ret
            }

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
        }

        #[gdnative::methods]
        impl $name {
            #[export]
            fn initialize(
                &mut self,
                owner: gdnative::TRef<$owner>,
                module: gdnative::nativescript::Instance<$crate::wasm_engine::WasmModule, gdnative::thread_access::Shared>,
                #[opt] host_bindings: Option<gdnative::core_types::Dictionary>,
            ) -> gdnative::core_types::Variant {
                self.data = match $crate::thisobj::InstanceData::initialize(
                    module.clone(),
                    host_bindings,
                    (
                        unsafe {
                            module
                                .assume_safe()
                                .map(|v, _| v.data.as_ref().expect("Uninitialized!").engine.clone())
                                .unwrap()
                        },
                        Some(unsafe { core::mem::transmute::<gdnative::TRef<$owner>, gdnative::TRef<'static, $owner>>(owner) }),
                        $($init)?
                    ),
                    |store, linker| {
                        (&$registry::new(|v: &(_, Option<gdnative::TRef<$owner>> $(, $extra)?)| {
                            *v.1.as_ref().expect("No this supplied")
                        }) as &dyn $crate::thisobj::FuncRegistry<_>).register_linker(store, linker)
                    },
                ) {
                    Ok(mut v) => {
                        v.store.data_mut().1 = None;
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
            fn call_wasm(&mut self, owner: gdnative::TRef<$owner>, name: String, args: gdnative::core_types::VariantArray) -> gdnative::core_types::Variant {
                let data = self._get_data();
                Self::_guard_section(data, owner, |data| data.call(&name, args))
            }

            $($rest)*
        }
    };
}
