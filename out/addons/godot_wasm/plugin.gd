tool
extends EditorPlugin


func _enter_tree():
	add_custom_type(
		"WasmModule",
		"Reference",
		preload("WasmModule.gdns"),
		preload("placeholder.bmp")
	)
	add_custom_type(
		"WasmInstance",
		"Reference",
		preload("WasmInstance.gdns"),
		preload("placeholder.bmp")
	)
	add_autoload_singleton("WasmHelper", "res://addons/godot_wasm/WasmHelper.gd")


func _exit_tree():
	remove_custom_type("WasmModule")
	remove_custom_type("WasmInstance")
	remove_autoload_singleton("WasmHelper")
