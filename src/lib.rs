mod wasm_config;
mod wasm_engine;
#[cfg(feature = "object-registry-extern")]
mod wasm_externref;
mod wasm_instance;
#[cfg(feature = "object-registry-compat")]
mod wasm_objregistry;
mod wasm_util;

use godot::prelude::*;

// This is just a type tag without any functionality
struct GodotWasm;

#[gdextension]
unsafe impl ExtensionLibrary for GodotWasm {}
