tool
extends EditorPlugin

var wasm_file_import_plugin: EditorImportPlugin = null

func _enter_tree():
	wasm_file_import_plugin = preload("WasmFileLoader.gd").new()
	add_import_plugin(wasm_file_import_plugin)


func _exit_tree():
	remove_import_plugin(wasm_file_import_plugin)
	wasm_file_import_plugin = null
