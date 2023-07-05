# WasiContext

_Defined in: [src/wasi_ctx/mod.rs](../src/wasi_ctx/mod.rs)_

This class defines a WASI execution context.

## Signals

* `stdout_emit(Variant message)`

  _Feature gate:_ `wasi`

  Used to handle standard output.
  Depending on the config, data can be a `String` or `PackedByteArray`.

* `stderr_emit(Variant message)`

  _Feature gate:_ `wasi`

  Used to handle standard error.
  Depending on the config, data can be a `String` or `PackedByteArray`.

## Properties

* `bool fs_readonly`

  If set to `true`, prevents any instance from writing to filesystem.

* `bool bypass_stdio`

  If set to `true`, pass standard output and standard error to Godot
  instead of emitting signal.

## Methods

* `void add_env_variable(String key, String value)`

  Sets environment variable.

* `String get_env_variable(String key)`

  Gets environment variable.

* `void mount_physical_dir(String host_path [String guest_path])`

  Mounts path to Webassembly.
  Host and guest path must be global path, not Godot specific paths.
  If guest path is not set, it is set the same as host path.

* `void write_memory_file(String path, String|PackedByteArray data, [int offset])`

  Writes data into in-memory file. Creates file if non-existent.

* `PackedByteArray read_memory_file(String path, int length, [int offset])`

  Reads in-memory file content.
