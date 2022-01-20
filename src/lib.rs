pub mod thisobj;
pub mod wasm_engine;
pub mod wasm_externref_godot;
pub mod wasm_store;

use gdnative::prelude::*;

use crate::thisobj::{node::WasmNode, node2d::WasmNode2D, object::*};
use crate::wasm_engine::{WasmEngine, WasmModule};

pub const TYPE_I32: u32 = 1;
pub const TYPE_I64: u32 = 2;
pub const TYPE_F32: u32 = 3;
pub const TYPE_F64: u32 = 4;
pub const TYPE_VARIANT: u32 = 6;

// Function that registers all exposed classes to Godot
fn init(handle: InitHandle) {
    handle.add_class::<WasmEngine>();
    handle.add_class::<WasmModule>();
    handle.add_class::<WasmObject>();
    handle.add_class::<WasmReference>();
    handle.add_class::<WasmNode>();
    handle.add_class::<WasmNode2D>();
}
// Macro that creates the entry-points of the dynamic library.
godot_init!(init);
