# Examples

This directory contains example Godot projects.
Before you can run any of it, initialize it first.

## Initialization

Due to issues, the addons are NOT included in repository. You must build it
yourself, or use `cargo make deploy_example` to do it automatically. It also
builds the webassembly modules.

## Content

Once you open the project, there is a sidebar on the right. Move mouse to
there to open it. There are many examples to choose from:

* Hello World
  Simple hello world module (`hello.wasm`).

* Host Bindings
  Example of using host functions to make a callback from WebAssembly.
  The host expose a write function to send a text to logger. You can
  modify the rust module (`host-bindings`) and recompile
  (`cargo make deploy_wasm`) to change it's output.

* Double Pendulum
  Showcasing the ability of WebAssembly to do complex calculation,
  this example simulates the double pendulum model.

* Connect 4
  This example shows how to integrate WebAssembly into a robot.
  The provided robot is a dummy one, so you can change it in
  it's corresponding rust module (`connect-4`). The robot is given 60 seconds
  to think, to prevent infinite loop. The robot is also ran under separate
  thread to prevent locking the main thread.

## Licensing

Unless otherwise noted, all script/code are licensed under Apache-2.0.
