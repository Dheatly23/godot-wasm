# WasmModule

_Defined in: [src/wasm_engine.rs](../src/wasm_engine.rs)_

This class defines a Webassembly module that can be later instantiated.
You should cache this to speed up further instantiation.

**⚠ WARNING: CALL initialize() ASAP, DO NOT USE UNINITIALIZED OBJECT!**

## Enums

### WasmType

* `TYPE_I32 = 1`
* `TYPE_I64 = 2`
* `TYPE_F32 = 3`
* `TYPE_F64 = 4`
* `TYPE_VARIANT = 6`

  _Feature gate:_ `object-registry-extern`

## Properties

### `String name`

(Optional) name of the module. Is currently ignored by the code.

## Methods

### `WasmModule initialize(String name, Variant data, Dictionary imports)`

Compiles the Webassembly data. Data can be one of the following:
* `PackedByteArray` of WASM/WAT file content.
* `String` of WAT file.
* `File` object that will be read.

Imports is a dictionary with keys is the import module name and value is
an instance of `WasmModule`.

NOTE: Imports is validated and failure will results in compilation error.
Eceptions are made for these modules:
* `host` : Host-defined functions.
* `godot_object_v1` : Legacy index-based Godot API
  (only enabled with feature `object-registry-compat`).
* `godot_object_v2` : New extern-based Godot API
  (only enabled with feature `object-registry-extern`).
* `wasi_unstable`, `wasi_snapshot_preview1` : WASI-related modules
  (only enabled with feature `wasi`).

Returns itself if succeed and `null` if failed. All errors is emitted
to the console directly and is not visible from GDScript.

### `Array get_imported_modules()`

Returns all the modules it imports.

### `Dictionary get_exports()`

Returns all exported functions signature. The keys are names of
the functions and it's values are a dictionary with two keys,
`params` and `results`, which contains an array of `WasmType` values.

### `Dictionary get_host_imports()`

Returns all host function imports. It's return value format is similiar
to `get_exports()`.

### `bool has_function(String name)`

Returns `true` if it has an exported function with that name.

### `Dictionary get_signature(String name)`

Returns the signature of exported function with that name.
Returns `null` if function is not found.
