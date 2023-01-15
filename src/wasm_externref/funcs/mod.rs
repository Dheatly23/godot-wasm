mod array;
#[cfg(feature = "object-registry-compat")]
mod compat;
mod dict;
mod object;
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

register![array, compat, dict, pool_array, primitive, string, typeis, object];

#[cfg(not(feature = "object-registry-compat"))]
mod compat {
    use crate::wasm_instance::StoreData;
    use wasmtime::Linker;
    #[inline]
    pub fn register_functions(_: &mut Linker<StoreData>) {}
}
