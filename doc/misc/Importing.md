# Importing WASM File into Godot

The addon will automatically registers `.wasm` and `.wat` files as WASM module.
Simply enables the addon and it'll recognize those files.

## Internals

There are 3 classes that helps with
registering, loading, saving, and importing WASM file.
`WasmLoader.gd` is tasked to deserialize WASM module.
`WasmSaver.gd` is tasked to serialize WASM module.
`WasmImporter.gd` is tasked to compile and load WASM file.

## Import Types

There are 2 different import type, as compiled file or as original file.
Both will produce `WasmModule`, so there is no difference in that.

Compiled format will produce serialized module `.cwasm` file.
Loading it is very fast and efficient.
The resulting file is bound to specific version and
must be reimported if you update godot-wasm.
Also it is unsafe to use with untrusted input,
as it simply deserialize the data.
It's maybe not compatible across architecture, OS, etc.

To ensure maximum safety and cross-compatibility, import as original file.
To reduce compilation overhead,
it is recommended to cache the resulting resource.
