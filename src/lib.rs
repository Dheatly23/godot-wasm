mod wasm_config;
mod wasm_engine;
#[cfg(feature = "object-registry-extern")]
mod wasm_externref;
mod wasm_instance;
#[cfg(feature = "object-registry-compat")]
mod wasm_objregistry;
mod wasm_util;

use gdnative::prelude::*;

use crate::wasm_engine::WasmModule;
use crate::wasm_instance::WasmInstance;

// Function that registers all exposed classes to Godot
fn init(handle: InitHandle) {
    handle.add_class::<WasmModule>();
    handle.add_class::<WasmInstance>();
}
// Macro that creates the entry-points of the dynamic library.
godot_init!(init);
