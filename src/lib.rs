#[cfg(feature = "wasi-preview2")]
mod preview2;
mod rw_struct;
#[cfg(feature = "wasi")]
mod wasi_ctx;
mod wasm_config;
mod wasm_engine;
#[cfg(feature = "object-registry-extern")]
mod wasm_externref;
mod wasm_instance;
#[cfg(feature = "object-registry-compat")]
mod wasm_objregistry;
mod wasm_util;
#[cfg(feature = "godot-component")]
mod godot_component;

use godot::prelude::*;

// This is just a type tag without any functionality
struct GodotWasm;

#[gdextension]
unsafe impl ExtensionLibrary for GodotWasm {}
