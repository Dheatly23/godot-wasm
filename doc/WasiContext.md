# WasiContext

_Defined in: [src/wasi_ctx/mod.rs](../src/wasi_ctx/mod.rs)_

This class defines a WASI execution context.

## Signals

### `stdout_emit(Variant message)`

_Feature gate:_ `wasi`

Used to handle standard output.
Depending on the config, data can be a `String` or `PackedByteArray`.

### `stderr_emit(Variant message)`

_Feature gate:_ `wasi`

Used to handle standard error.
Depending on the config, data can be a `String` or `PackedByteArray`.

## Properties

### `bool fs_readonly`

If set to `true`, prevents any instance from writing to filesystem.

### `bool bypass_stdio`

If set to `true`, pass standard output and standard error to Godot
instead of emitting signal.

## Methods

### `void add_env_variable(String key, String value)`

Sets environment variable.

### `null|String get_env_variable(String key)`

Gets environment variable.

### `null|String delete_env_variable(String key)`

Deletes environment variable.

### `void mount_physical_dir(String host_path, [String guest_path])`

Mounts path to Webassembly.
Host and guest path must be global path, not Godot specific paths.
If guest path is not set, it is set the same as host path.

### `Dictionary get_mounts()`

Gets all mount points and their source directory.

### `bool unmount_physical_dir(String guest_path)`

Unmounts path.

### `int file_is_exist(String path, [bool follow_symlink])`

Checks if file exists. Possible return values:
* 0 : File does not exist
* 1 : Is a file
* 2 : Is a directory
* 3 : Is a symlink

### `bool file_make_file(String path, String name, [bool follow_symlink])`

Create new file. Returns `true` if succeed.

### `bool file_make_dir(String path, String name, [bool follow_symlink])`

Create new directory. Returns `true` if succeed.

### `bool file_make_link(String path, String name, String target, [bool follow_symlink])`

Create new symlink. Returns `true` if succeed.

### `bool file_delete_file(String path, String name, [bool follow_symlink])`

Delete file. Returns `true` if succeed.

### `null|PoolStringArray file_dir_list(String path, [bool follow_symlink])`

Returns all filenames in directory.

### `null|Dictionary file_stat(String path, [bool follow_symlink])`

Returns file stats, like `size`, `ctime`, `mtime`, and `atime`.

### `bool file_set_time(String path, Dictionary time, [bool follow_symlink])`

Set file `mtime` and `atime` with the value in dictionary.
You can omit values and it'll be filled with current time.

### `null|String file_link_target(String path, [bool follow_symlink])`

Reads symlink target.

### `null|PoolByteArray file_read(String path, int length, [int offset, bool follow_symlink])`

Reads file content. Set length to 0 to read the entire file.

### `bool file_write(String path, Variant data, [int offset, bool truncate, bool follow_symlink])`

Writes file content.
Accepts `String` and all `Pool*` data type, except `PoolStringArray`.
File will be appended with zeros if needed.
Returns `true` if succeed.
