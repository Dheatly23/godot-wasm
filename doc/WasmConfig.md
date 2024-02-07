# WasmInstance

_Defined in: [src/wasm_config.rs](../src/wasm_config.rs)_

**THIS IS NOT A CLASS!**

It documents configuration options for instantiation.
Configuration value is a dictionary with key is configuration name.

## Configs

### epoch.enable

* Feature gate: `epoch-timeout`
* Type: `bool`
* Default: `false`

Enables epoch-based timeout.

### epoch.timeout

* Feature gate: `epoch-timeout`
* Type: `null|int|float`
* Default: `null`

Sets how many second the instance can run.
If not set or `null`, it defaults to 5 seconds.

### epoch.useAutoreset

* Feature gate: `epoch-timeout`
* Type: `bool`
* Default: `false`

If enabled, automatically resets epoch timer whenever it returns from host.

### memory.maxGrowBytes

* Feature gate: `memory-limiter`
* Type: `int`

If set, it limits the amount of **extra** bytes all Webassembly memories can allocate.

### table.maxGrowEntries

* Feature gate: `memory-limiter`
* Type: `int`

If set, it limits the amount of **extra** entries all Webassembly tables can allocate.

### wasi.enable

* Feature gate: `wasi`
* Type: `bool`
* Default: `false`

Enables usage of WASI.

### wasi.context

* Feature gate: `wasi`
* Type: `WasiContext`

Sets which WASI context object it can use.

### wasi.args

* Feature gate: `wasi`
* Type: `Array`

Sets arguments of the instance.
NOTE: First argument is the "executable name".

### wasi.envs

* Feature gate: `wasi`
* Type: `Dictionary`

Sets additional environment variables for the instance.

### wasi.fsReadonly

* Feature gate: `wasi`
* Type: `bool`
* Default: `false`

If enabled, it prevents Webassembly from writing to filesystem.
Only useful with context set, as by default it can't access anything.

### wasi.stdin.bindMode

* Feature gate: `wasi`
* Type: `String`

Must be one of these value:
* `"context"` (default) : Connect standard input to context object.
* `"unbound"` : Do not connect standard input.
* `"instance"` : Connect standard input to instance object.

### wasi.stdin.inputData

* Feature gate: `wasi`
* Type: `PackedByteArray`

Prefill standard input with data.

### wasi.stdin.inputFile

* Feature gate: `wasi`
* Type: `string`

Prefill standard input with in-memory file.
Useful only with context set.

### wasi.stdout.bindMode

* Feature gate: `wasi`
* Type: `String`

Must be one of these value:
* `"context"` (default) : Connect standard output to context object.
* `"unbound"` : Do not connect standard output.
* `"instance"` : Connect standard output to instance object.

### wasi.stdout.bufferMode

* Feature gate: `wasi`
* Type: `String`

Must be one of these value:
* `"line"` (default) : Buffers by line. Emits as string.
* `"block"` : Buffers by block. Emits as PackedByteArray.
* `"unbuffered"` : Disable buffering. Emits as PackedByteArray.

### wasi.stderr.bindMode

* Feature gate: `wasi`
* Type: `String`

Must be one of these value:
* `"context"` (default) : Connect standard error to context object.
* `"unbound"` : Do not connect standard error.
* `"instance"` : Connect standard error to instance object.

### wasi.stderr.bufferMode

* Feature gate: `wasi`
* Type: `String`

Must be one of these value:
* `"line"` (default) : Buffers by line. Emits as string.
* `"block"` : Buffers by block. Emits as PackedByteArray.
* `"unbuffered"` : Disable buffering. Emits as PackedByteArray.

### extern.bindMode

* Type: `String`

Must be one of these value:
* `"none"` or `"no_binding"` (default) : Do not expose Godot API.
* `"compat"` or `"registry"` : Use legacy index-based Godot API.
* `"extern"` or `"native"` : Use new extern-based Godot API.
