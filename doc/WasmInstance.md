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
* `"callable"` : Callable to call. Replaces object-method pair.

Config is too complex to be put here, read at [WasmConfig](./WasmConfig.md).

### `Array|null call_wasm(StringName name, Array args)`

Calls WASM exported function with given arguments. Returns null if it errors.

### `Callable bind_wasm(StringName name)`

Creates a callable that calls WASM exported function.

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

### `void memory_set_name(String name)`

Sets a custom exported memory name.
Useful if instance memory is non-standard (eg. not named `"memory"`).

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

### `void put_array(int ptr, Variant array)`

Writes array of values to memory.

### `Variant get_array(int ptr, int n, VariantType type)`

Reads array of values from memory.

### `Array read_struct(String format, int ptr)`

Reads a formatted data from memory.

### `int write_struct(String format, int ptr, Array data)`

Writes a formatted data into memory.

## Addendum 1: Struct Format String

The format string used for `read_struct()` and `write_struct()`
is defined as a list of items.
Each item contains a type, optionally preceded by a repetition count.
The valid types are as follows:

| String | Godot Type | Byte Length | Description |
|:------:|:----------:|:-----------:|:------------|
| `x` | | 1 | Padding byte, will not be read/written. Padding bytes are not automatically added. |
| `b` | `int` | 1 | Signed 8-bit number |
| `B` | `int` | 1 | Unsigned 8-bit number |
| `h` | `int` | 2 | Signed 16-bit number |
| `H` | `int` | 2 | Unsigned 16-bit number |
| `i` | `int` | 4 | Signed 32-bit number |
| `I` | `int` | 4 | Unsigned 32-bit number |
| `l` | `int` | 8 | Signed 64-bit number |
| `L` | `int` | 8 | Unsigned 64-bit number |
| `f` | `float` | 4 | 32-bit floating-point number |
| `d` | `float` | 8 | 64-bit floating-point number |
| `v2f` | `Vector2` | 8 | 2D vector as 2 32-bit floating-point number |
| `v2d` | `Vector2` | 16 | 2D vector as 2 64-bit floating-point number |
| `v2i` | `Vector2i` | 8 | 2D vector as 2 32-bit signed integer number |
| `v2l` | `Vector2i` | 16 | 2D vector as 2 64-bit signed integer number |
| `v3f` | `Vector3` | 12 | 3D vector as a 3 32-bit floating-point number |
| `v3d` | `Vector3` | 24 | 3D vector as a 3 64-bit floating-point number |
| `v3i` | `Vector3i` | 12 | 3D vector as a 3 32-bit signed integer number |
| `v3l` | `Vector3i` | 24 | 3D vector as a 3 64-bit signed integer number |
| `v4f` | `Vector4` | 12 | 4D vector as a 4 32-bit floating-point number |
| `v4d` | `Vector4` | 24 | 4D vector as a 4 64-bit floating-point number |
| `v4i` | `Vector4i` | 12 | 4D vector as a 4 32-bit signed integer number |
| `v4l` | `Vector4i` | 24 | 4D vector as a 4 64-bit signed integer number |
| `pf` | `Plane` | 16 | Plane represented as abcd 32-bit floating-point number |
| `pd` | `Plane` | 32 | Plane represented as abcd 64-bit floating-point number |
| `qf` | `Quat` | 16 | Quaternion represented as xyzw 32-bit floating-point number |
| `qd` | `Quat` | 32 | Quaternion represented as xyzw 64-bit floating-point number |
| `Cf` | `Color` | 16 | Color represented as rgba 32-bit floating-point number |
| `Cd` | `Color` | 32 | Color represented as rgba 64-bit floating-point number |
| `Cb` | `Color` | 4 | Color represented as rgba 8-bit integer |
| `rf` | `Rect2` | 16 | Rect2 represented as 4 32-bit floating-point number |
| `rd` | `Rect2` | 32 | Rect2 represented as 4 64-bit floating-point number |
| `ri` | `Rect2i` | 16 | Rect2i represented as 4 32-bit signed integer number |
| `rl` | `Rect2i` | 32 | Rect2i represented as 4 64-bit signed integer number |
| `af` | `Aabb` | 24 | Aabb represented as 6 32-bit floating-point number |
| `ad` | `Aabb` | 48 | Aabb represented as 6 64-bit floating-point number |
| `mf` | `Basis` | 36 | Basis represented as 9 row-major 32-bit floating-point number |
| `md` | `Basis` | 72 | Basis represented as 9 row-major 64-bit floating-point number |
| `Mf` | `Projection` | 64 | Projection represented as 16 column-major 32-bit floating-point number |
| `Md` | `Projection` | 128 | Projection represented as 16 column-major 64-bit floating-point number |
| `tf` | `Transform2D` | 24 | 2D transform represented as 6 32-bit floating-point number |
| `td` | `Transform2D` | 48 | 2D transform represented as 6 64-bit floating-point number |
| `Tf` | `Transform2D` | 48 | 3D transform represented as 12 32-bit floating-point number |
| `Td` | `Transform2D` | 96 | 3D transform represented as 12 64-bit floating-point number |
