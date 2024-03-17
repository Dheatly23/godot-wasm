use wasmtime::component::bindgen;

bindgen!({
    path: "wit/imports/core",
    world: "godot:core/godot-core",
});
