mod wasm_engine;
mod wasm_object;

use gdnative::prelude::*;

pub use crate::wasm_engine::*;
pub use crate::wasm_object::*;

// Function that registers all exposed classes to Godot
fn init(handle: InitHandle) {
    handle.add_class::<WasmEngine>();
    handle.add_class::<WasmObject>();
}
// Macro that creates the entry-points of the dynamic library.
godot_init!(init);
