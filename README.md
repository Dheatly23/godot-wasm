# godot-wasm
WebAssembly binding for Godot, with rich feature set.

## Introduction
Hello there! Thanks for checking out this library 🙏.
I published this as my future portfolio on my coding adventure.
This is my hobby project, developed over 6 months now.
It has gone through a _lot_ of changes.
And with Godot 4 on horizon, i _might_ port it to the next version
(if and when rust bindings is finalized).

## ⚠ WARNING! Very Beta! ⚠
**This repository is changing rapidly.** And since i don't really like
semver, things might break unexpectedly. Just follow the latest commit
and you're probably safe.

Documentation is the source code itself. If you want to document, great!
But i'm just **really lazy** and perhaps old-school.

## Features

* Easily run any WASM module.
* Supports WAT compilation.
* Epoch-based limiter to stop bad-behaving module.
* Experimental API for direct Godot object manipulation.

## Building
To build the addon:
1. Clone this repository
2. Install `cargo-make` (see installation guide [here](https://crates.io/crates/cargo-make))
3. Run `cargo make deploy`
4. Copy addon in `out/addons/godot_wasm` to your project

## Using the Library
After adding it to your Godot project, there is 2 classes added by the library:
* `WasmModule` : Contains the compiled WebAssembly module.
* `WasmInstance` : Contains the instantiated module.

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
    "test", # Name of module (not really used)
    WAT, # Module string (accepts PoolByteArray or String)
    {} # Imports to other module
  )

  # Create instance from module
  var instance = WasmInstance.new().initialize(
    module, # Module object
    {} # Host imports
    # Configuration (optional)
  )
  # Convenience method
  # var instance = module.instantiate({})

  # Call to WASM
  print(instance.call_wasm("add", [1, 2]))
```

In the addon, there is a helper autoload `WasmHelper` to help you load WASM
from file. See example for details.

## Potential Uses

There are many uses of running WebAssembly code in Godot. If you are looking
for inspiration or just confused about the purpose of this package,
here are some prompts:

### Language-independent* programming game

  Many programming language now supports compiling to WebAssembly. And with
  many programming type game out there, it would be awesome to transfer your
  skill at your favourite programming language into the game. Bonus, if
  somehow your program has bugs, it won't corrupt or crash the game.

  _*Right now, very few programming language can emit standalone WASM._

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
