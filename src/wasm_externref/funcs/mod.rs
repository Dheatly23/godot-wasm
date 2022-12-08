mod array;
mod dict;
mod pool_array;
mod primitive;
mod string;
mod typeis;

use wasmtime::Linker;

use crate::wasm_instance::StoreData;

macro_rules! register{
    ($($m:ident),* $(,)?) => {
        #[inline]
        pub fn register_functions(linker: &mut Linker<StoreData>) {
            $($m::register_functions(&mut *linker);)*
        }
    };
}

register![array, dict, pool_array, primitive, string, typeis];
