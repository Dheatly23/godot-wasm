tool
extends EditorPlugin

var wasm_file_import_plugin: EditorImportPlugin = null

func _enter_tree():
	add_autoload_singleton("WasmHelper", "res://addons/godot_wasm/WasmHelper.gd")

	add_custom_type(
		"WasmModule",
		"Reference",
		WasmModule,
		preload("placeholder.bmp")
	)
	add_custom_type(
		"WasmInstance",
		"Reference",
		WasmInstance,
		preload("placeholder.bmp")
	)
	add_custom_type(
		"WasmFile",
		"PackedDataContainer",
		WasmFile,
		preload("placeholder.bmp")
	)

	wasm_file_import_plugin = preload("WasmFileLoader.gd").new()
	add_import_plugin(wasm_file_import_plugin)


func _exit_tree():
	remove_import_plugin(wasm_file_import_plugin)
	wasm_file_import_plugin = null

	remove_custom_type("WasmModule")
	remove_custom_type("WasmInstance")
	remove_custom_type("WasmFile")

	remove_autoload_singleton("WasmHelper")
