@tool
class_name WasmLoader
extends ResourceFormatLoader

func _get_recognized_extensions() -> PackedStringArray:
	return PackedStringArray([
		"wasm",
		"wat",
		"cwasm",
	])

func _handles_type(type: StringName) -> bool:
	return type == &"WasmModule"

func _get_resource_type(path: String) -> String:
	if path.ends_with(".wasm") || path.ends_with(".wat") || path.ends_with(".cwasm"):
		return "WasmModule"
	return ""

func _load(path: String, original_path: String, use_sub_threads: bool, cache_mode: CacheMode):
	if not original_path.is_empty():
		path = original_path

	var module: WasmModule
	if path.ends_with(".cwasm"):
		module = WasmModule.new().deserialize_file("", path, {})
	else:
		var data := FileAccess.get_file_as_bytes(path)
		var err := FileAccess.get_open_error()
		if err != OK:
			return err
		module = WasmModule.new().initialize("", data, {})

	if module == null:
		return FAILED
	return module
