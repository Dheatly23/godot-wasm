# WasmHelper

_Defined in: [WasmHelper.gd](../out/addons/godot_wasm/WasmHelper.gd)_

The script contains helper constants and functions for using the addon.

## Constants

```gdscript
const TYPE_I32 = 1
const TYPE_I64 = 2
const TYPE_F32 = 3
const TYPE_F64 = 4
const TYPE_VARIANT = 6
```

## Static Functions

### `WasmModule load_wasm(String name, Variant data, Dictionary imports = {})`

Loads a new Webassembly module. Returns `null` if it fails.

### `WasmModule load_wasm_file(String name, String path, Dictionary imports = {})`

Loads a new Webassembly module from file.
Path can be a Godot-specific path or global path. Returns `null` if it fails.
