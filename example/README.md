# Examples

This directory contains example Godot projects.
Before you can run any of it, initialize it first.

## Initialization

Due to issues, the addons are NOT included in repository. You must build it
yourself, or use `cargo make deploy_example` to do it automatically.

## Content

There are many example:
* `hello-wasm`

  Hello world from WebAssembly.
  Here you can learn how to load module, instantiate,
  and bind with Godot methods.

* `epoch-interruption`

  Showcase using epoch interruption to limit WebAssembly execution and
  preventing infinite loop.

* `connect-4`

  Simple connect 4 game with robot as WASM module. Provided with a stub robot,
  which you can replace with your own robot.

## Licensing

Unless otherwise noted, all scripts are licensed under Apache-2.0.
