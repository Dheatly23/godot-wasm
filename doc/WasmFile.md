# WasmFile

_Defined in: [WasmFile.gd](../out/addons/godot_wasm/WasmFile.gd)_

_Inherits: [PackedDataContainer](https://docs.godotengine.org/en/stable/classes/class_packeddatacontainer.html)_

This class (along with WasmFileHelper) helps importing WASM and WAT file into Godot.

## Properties

### `String name`

The name of the module. Is exported and editable.

### `Dictionary imports`

Imports for this module. Values are also `WasmFile` objects.

## Methods

### `WasmModule get_module()`

Gets the compiled module.
Module object is cached, so calling this twice will yield the same object.

### `WasmInstance instantiate(Dictionary host = {}, Dictionary config = {})`

Creates a new instance from the module.
