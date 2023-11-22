mod array;
#[cfg(feature = "object-registry-compat")]
mod compat;
mod dict;
mod object;
mod pool_array;
mod primitive;
mod string;
mod typeis;

use wasmtime::{Func, StoreContextMut};

use crate::wasm_instance::StoreData;

macro_rules! register{
    ($($m:ident),* $(,)?) => {
        #[derive(Default)]
        pub struct Funcs {
            $($m: $m::Funcs),*
        }

        impl Funcs {
            pub fn get_func<T>(&mut self, store: &mut StoreContextMut<'_, T>, name: &str) -> Option<Func>
            where
                T: AsRef<StoreData> + AsMut<StoreData>,
            {
                $(if let r @ Some(_) = self.$m.get_func(&mut *store, name) {
                    r
                } else)*
                {
                    None
                }
            }
        }
    };
}

register![array, compat, dict, pool_array, primitive, string, typeis, object];

#[cfg(not(feature = "object-registry-compat"))]
mod compat {
    use crate::wasm_instance::StoreData;
    use wasmtime::Linker;
    #[inline]
    pub fn register_functions(_: &mut Linker<StoreData>) {}
}
