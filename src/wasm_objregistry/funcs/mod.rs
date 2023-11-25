mod array;
mod dict;
mod other;
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

register![array, dict, other, pool_array, primitive, string, typeis];
