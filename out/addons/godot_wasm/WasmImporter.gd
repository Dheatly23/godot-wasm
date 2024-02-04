@tool
extends EditorImportPlugin

enum Presets {
	DEFAULT
}

const SAVE_EXT := "cwasm"

func _get_importer_name() -> String:
	return "godot_wasm.wasm"

func _get_visible_name() -> String:
	return "WASM Importer"

func _get_recognized_extensions() -> PackedStringArray:
	return PackedStringArray(["wasm", "wat"])

func _get_save_extension() -> String:
	return SAVE_EXT

func _get_resource_type() -> String:
	return "WasmModule"

func _get_import_order() -> int:
	return 0

func _get_priority():
	return 1

func _get_preset_count() -> int:
	return Presets.size()

func _get_preset_name(preset: int) -> String:
	match preset:
		Presets.DEFAULT:
			return "Default"
		_:
			return "Unknown"

func _get_import_options(path: String, preset: int) -> Array[Dictionary]:
	return []

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

	var r: WasmModule = WasmModule.new().initialize("", data, {})
	if r == null:
		return FAILED

	return ResourceSaver.save(r, "%s.%s" % [save_path, SAVE_EXT])
