# godot-wasm
WebAssembly binding for Godot, with rich feature set.

## Introduction
Hello there! Thanks for checking out this library 🙏.
I published this as my future portfolio on my coding adventure.
This is my hobby project, developed over 6 months
(+1 year porting to Godot 4).
It has gone through a _lot_ of changes.

For the Godot 3 version, check out branch `gdnative`.

## ⚠ WARNING! Very Beta! ⚠
**This repository is changing rapidly.** And since i don't really like
semver, things might break unexpectedly. Just follow the latest commit
and you're probably safe.

Documentation is in [doc](doc/README.md) folder. But it may be not up-to-date.

## Features

**NOTE:** Many features are not yet available or (partially) broken.

* Easily run any WASM module.
* Supports WAT compilation.
* Imports any (custom) Godot methods into WASM.
* Easy access to linear memory, per-element basis or bulk array operation.
* Catch and throw runtime error/traps with signal.
* Epoch-based limiter to stop bad-behaving module.
* Memory limiter to prevent exhaustion.
* Experimental API for direct Godot object manipulation. Available in 3 flavors:
  * Legacy `compat` API using indices. Easier to implement.
  * Newer `extern` API using externrefs, with interop with legacy API.
    Some programming language might not support it at all.
  * Component-based API. Requires component model.
* WASI common API ~~with in-memory filesystem~~.

  **NOTE:** In-memory filesystem is currently disabled due to dependency problems.
  It will be enabled in the future if possible.

* Native resource support to ease import/export.
* Optional support for component model and WASI 0.2

## Building
To build the addon:
1. Clone this repository
2. Install [`just`](https://github.com/casey/just) and [`nushell`](https://github.com/nushell/nushell).
3. Run `just profile=release deploy-addon`
4. Copy addon in `out/addons/godot_wasm` to your project

Extra build arguments can be added with
environment variable `BUILD_EXTRA_ARGS`, separated by spaces.

### Cross-Compilation
Cross-compilation is not always possible.
You might be able to do it with WSL (Windows)
or [cross](https://crates.io/crates/cross) (Linux), but result may vary.
It's recommended to **not** use `just`,
instead directly run `cargo build` with the correct arguments.

### Platform Support
Officially, it should work for Windows and Linux on x86 (32 and 64-bit) architecture.
Other platforms aren't tested and might not work.

Platforms that are listed in `.gdextension` file is as follow:
* Windows: x86_32, x86_64, ARM64.
* Linux: x86_32, x86_64, ARM64, RV64.
* MacOS: x86_64, ARM64.

Web platform is and will not be supported.
Even if it is, it's API will likely be different,
and many features (like limiter) will not work.

## Using the Library
After adding it to your Godot project, there are many classes added by the library:
* `WasmModule` : Contains the compiled WebAssembly module.
* `WasmInstance` : Contains the instantiated module.
* `WasiContext` : Context for WASI, including stdout and filesystem.
* `WasiCommand` : Optional class to run WASI 0.2 runnable component.

Due to limitation of godot-rust,
you must call `initialize` after creating new object.
Here is a snippet of example code:
```gdscript
const WAT = """
(module
  (func $add (export "add") (param i64 i64) (result i64)
    local.get 0
    local.get 1
    i64.add
  )
)
"""

func _ready():
  # initialize() returns itself if succeed and null otherwise
  # WARNING! DO NOT USE UNINITIALIZED/FAILED MODULE OBJECTS
  var module = WasmModule.new().initialize(
    WAT, # Module data (accepts PackedByteArray, String, FileAccess, or WasmModule)
    {} # Imports to other module
  )

  # Create instance from module
  var instance = WasmInstance.new().initialize(
    module, # Module object
    {}, # Host imports
    {} # Configuration
  )
  # Convenience method
  # var instance = module.instantiate({})

  # Call to WASM
  print(instance.call_wasm("add", [1, 2]))

  # There are many more methods of WasmInstance, including:
  # - Trapping (signal_error/signal_error_cancel)
  # - Epoch (reset_epoch)
  # - Memory (too many to list here)
  # See it's source code (src/wasm_instance.rs) for list.
```

With the addon, there are many more helper scripts too:
* `WasmHelper` : Autoload that contains many helper functions to load
  WebAssembly code.
* `WasmLoader`/`WasmSaver` : Registers WASM files as native resource.

## Potential Uses

There are many uses of running WebAssembly code in Godot. If you are looking
for inspiration or just confused about the purpose of this package,
here are some prompts:

### Language-independent* programming game

  Many programming language now supports compiling to WebAssembly. And with
  many programming type game out there, it would be awesome to transfer your
  skill at your favourite programming language into the game. Bonus, if
  somehow your program has bugs, it won't corrupt or crash the game.

  _*Right now, very few programming language can emit standalone WASM.
  Although WASI expands the number of language supported, it may require
  some custom host API shim layer._

### Competitive robot/AI game

  Isn't that obvious enough? Tied to previous one, a really great use is some
  sort of competitive multiplayer AI vs AI game. With sandboxing of
  WebAssembly, no code can do any harm to participant/judge.

### Custom userscript

  Instead of making your own scripting language to integrate into your game,
  why not consider sandboxing it within WebAssembly?

### Modding framework

  WebAssembly can replace DLL/SO as a way to mod your game. Using it as easy
  as exposing your API as imports. Plus, sandboxing makes *any* mod
  automatically be safe from doing malicious things.

### Server-sent Mod

  With mods there will *always* problem with multiplayer. Imagine having to
  install random code just to join your favorite server. And don't forget
  to juggle mods for different servers. Well no more, the server could just
  send you the mods and assets necessary to join. It's automatic, painless,
  and of course, safe. Think of browsers, where you load untrusted website
  code safely.
