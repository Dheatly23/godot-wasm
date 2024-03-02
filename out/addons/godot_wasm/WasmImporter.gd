@tool
extends EditorImportPlugin

enum Presets {
	DEFAULT
}

var as_orig := false
var priority := 1.0

func _get_importer_name() -> String:
	if as_orig:
		return "godot_wasm.orig.wasm"
	else:
		return "godot_wasm.wasm"

func _get_visible_name() -> String:
	if as_orig:
		return "WASM File"
	else:
		return "Compiled WASM File"

func _get_recognized_extensions() -> PackedStringArray:
	if as_orig:
		return PackedStringArray(["wasm"])
	else:
		return PackedStringArray(["wasm", "wat"])

func _get_save_extension() -> String:
	if as_orig:
		return "wasm"
	else:
		return "cwasm"

func _get_resource_type() -> String:
	return "WasmModule"

func _get_import_order() -> int:
	return 0

func _get_priority() -> float:
	return priority

func _get_preset_count() -> int:
	return Presets.size()

func _get_preset_name(preset: int) -> String:
	match preset:
		Presets.DEFAULT:
			return "Default"
		_:
			return "Unknown"

func _get_import_options(path: String, preset: int) -> Array[Dictionary]:
	return [
		{
			name = "include_original_file",
			default_value = false,
		},
	]

func _get_option_visibility(path: String, option_name: StringName, options: Dictionary) -> bool:
	return true

func _import(
	source_file: String,
	save_path: String,
	options: Dictionary,
	platform_variants: Array[String],
	gen_files: Array[String],
):
	var data := FileAccess.get_file_as_bytes(source_file)
	var err := FileAccess.get_open_error()
	if err != OK:
		return err

	var r: WasmModule = WasmModule.new().initialize(data, {})
	if r == null:
		return FAILED

	if options.include_original_file:
		var p := save_path + (".wasm" if source_file.ends_with(".wasm") else ".wat")
		var f := FileAccess.open(p, FileAccess.WRITE)
		err = FileAccess.get_open_error()
		if err != OK:
			return err

		f.store_buffer(data)
		gen_files.push_back(p)

	if as_orig:
		var f := FileAccess.open(save_path + ".wasm", FileAccess.WRITE)
		if f != null:
			f.store_buffer(data)
		return FileAccess.get_open_error()
	else:
		return ResourceSaver.save(r, save_path + ".cwasm")
