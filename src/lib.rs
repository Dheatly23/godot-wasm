#[cfg(feature = "godot-component")]
mod godot_component;
mod godot_util;
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

#[cfg(feature = "tracing")]
use std::env::var_os;
#[cfg(feature = "tracing")]
use std::path::PathBuf;

use godot::prelude::*;
#[cfg(feature = "tracing")]
use log4rs::init_file;

// This is just a type tag without any functionality
struct GodotWasm;

#[gdextension]
unsafe impl ExtensionLibrary for GodotWasm {
    fn min_level() -> InitLevel {
        InitLevel::Servers
    }

    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Servers {
            #[cfg(feature = "tracing")]
            if let Some(v) = var_os("GODOT_WASM_LOG_CONFIG_FILE") {
                init_file(PathBuf::from(v), Default::default()).unwrap();
            }
            wasm_engine::init_engine();
        }
    }

    fn on_level_deinit(level: InitLevel) {
        if level == InitLevel::Servers {
            wasm_engine::deinit_engine();
        }
    }
}
