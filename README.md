# godot-wasm
WebAssembly binding for Godot

# Introduction
Hello there! Thanks for checking out this library 🙏. I published this as my future portfolio on my coding adventure.
This is my hobby project, developed over 6 months now. It has gone through a _lot_ of changes.
And with Godot 4 on horizon, i _might_ port it to the next version (if and when rust bindings is finalized).

# Building
Unfortunately, there is no automatic compilation to full-packaged addon. So you have to copy the compiled dlls to appropiate place.
There is a `cargo-make` script embedded, if you can contribute please do!

# Using the Library
After adding it to your Godot project, there is 2 classes added by the library:
* `WasmModule` : Contains the compiled Webassembly module.
* `WasmInstance` : Contains the instantiated module.

Due to limitation of godot-rust, you must call `initialize` after creating new object.
Here are a snippet of example code:
```gdscript
# Change resource path appropiately
const WasmModule = preload("res://godot_wasm/WasmModule.gdns")
const WasmInstance = preload("res://godot_wasm/WasmInstance.gdns")

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
