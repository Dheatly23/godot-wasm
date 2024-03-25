@tool
extends EditorPlugin

var import_plugins: Array[EditorImportPlugin] = []

func _enter_tree():
	var v := preload("WasmImporter.gd").new()
	v.priority = 2.0
	import_plugins.push_back(v)

	v = preload("WasmImporter.gd").new()
	v.as_orig = true
	v.priority = 1.0
	import_plugins.push_back(v)

	for i in import_plugins:
		add_import_plugin(i)

func _exit_tree():
	for v in import_plugins:
		remove_import_plugin(v)

	import_plugins.clear()
