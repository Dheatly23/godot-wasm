pub mod wasm_engine;
pub mod wasm_object;

use gdnative::prelude::*;

pub use crate::wasm_engine::*;
pub use crate::wasm_object::*;

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;

// Function that registers all exposed classes to Godot
fn init(handle: InitHandle) {
    handle.add_class::<WasmEngine>();
    handle.add_class::<WasmObject>();
}
// Macro that creates the entry-points of the dynamic library.
godot_init!(init);
