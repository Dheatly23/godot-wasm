# WasmInstance

_Defined in: [src/wasm_instance.rs](../src/wasm_instance.rs)_

This class creates a Webassembly instance.

**⚠ WARNING: CALL initialize() ASAP, DO NOT USE UNINITIALIZED OBJECT!**

## Signals

### `error_happened(String message)`

Emitted when an error has happened.
It can be one of the following (non-exhaustive):
* Trap instruction
* Trap from host function
* Epoch timeout reached
* Instantiation errors

### `stdout_emit(Variant message)`

_Feature gate:_ `wasi`

Used to handle standard output.
Depending on the config, data can be a `String` or `PackedByteArray`.

### `stderr_emit(Variant message)`

_Feature gate:_ `wasi`

Used to handle standard error.
Depending on the config, data can be a `String` or `PackedByteArray`.

### `stdin_request()`

_Feature gate:_ `wasi`

Used to handle standard input request.

## Properties

### `WasmModule module`

The module used to instantiate.

## Methods

### `WasmInstance initialize(WasmModule module, Dictionary host = {}, Dictionary config = {})`

Instantiate module. Host is a dictionary with key of function name and
value is a dictionary with content defined as follows:
* `"params"` : Array of type enum values.
* `"results"` : Array of type enum values.
* `"object"` : The object which to bind call.
* `"mefhod"` : The method name to call.

Config is too complex to be put here, read at [WasmConfig](./WasmConfig.md).

### `Array|null call_wasm(String name, Array args)`

Calls WASM exported function with given arguments. Returns null if it errors.

### `String signal_error(String message)`

Used from host calls to signal error upon returning to WASM.

### `String signal_error_cancel()`

Used from host calls to undo `signal_error`.

### `void reset_epoch()`

Used from host calls to manually reset epoch timer.

### `int register_object(Variant object)`

_Feature gate:_ `object-registry-compat`

Registers object to be used in WASM. Returns it's ID.
NOTE: ID is specific to the instance it's registered
and may not be used cross-instance.

### `Variant registry_get(int id)`

_Feature gate:_ `object-registry-compat`

Gets object by it's ID.

### `Variant registry_set(int id, Variant new_object)`

_Feature gate:_ `object-registry-compat`

Replace object in registry. Returns old object.

### `Variant unregister_object(int id)`

_Feature gate:_ `object-registry-compat`

Unregisters object from registry. Returns the object.

### `void stdin_add_line(String line)`

_Feature gate:_ `wasi`

Appends a new line to standard input.

### `void stdin_close()`

_Feature gate:_ `wasi`

Closes standard input.

### `bool has_memory()`

Returns true if memory is available
(an export of type memory and named `memory`).

### `int memory_size()`

Gets memory size.

### `PackedByteArray memory_read(int ptr, int n)`

Reads a chunk of memory. Return null if pointer range is invalid.

### `void memory_write(int ptr, PackedByteArray data)`

Writes a chunk of memory.

### `int get_8(int ptr)`

Gets a byte from memory.

### `void put_8(int ptr, int value)`

Puts a byte to memory.

### `int get_16(int ptr)`

Gets a 16-bit little-endian unsigned integer from memory.

### `void put_16(int ptr, int value)`

Puts a 16-bit little-endian unsigned integer to memory.

### `int get_32(int ptr)`

Gets a 32-bit little-endian unsigned integer from memory.

### `void put_32(int ptr, int value)`

Puts a 32-bit little-endian unsigned integer to memory.

### `int get_64(int ptr)`

Gets a 64-bit little-endian signed integer from memory.

### `void put_64(int ptr, int value)`

Puts a 64-bit little-endian signed integer to memory.

### `float get_float(int ptr)`

Gets a 32-bit little-endian floating-point number from memory.

### `void put_float(int ptr, float value)`

Puts a 32-bit little-endian floating-point number to memory.

### `float get_double(int ptr)`

Gets a 64-bit little-endian floating-point number from memory.

### `void put_double(int ptr, float value)`

Puts a 64-bit little-endian floating-point number to memory.

### `void put_array(int ptr, PackedByteArray|PackedIntArray|PackedFloatArray array)`

Writes array of values to memory.

### `PackedByteArray|PackedIntArray|PackedFloatArray get_array(int ptr, int n, VariantType type)`

Reads array of values from memory.
