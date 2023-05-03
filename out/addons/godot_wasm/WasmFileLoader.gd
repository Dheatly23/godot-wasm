@tool
extends EditorImportPlugin

enum Presets {
	DEFAULT
}

func _get_importer_name() -> String:
	return "godot_wasm.wasm"

func _get_visible_name() -> String:
	return "WASM Importer"

func _get_recognized_extensions() -> PackedStringArray:
	return PackedStringArray(["wasm", "wat"])

func _get_save_extension() -> String:
	return "res"

func _get_resource_type() -> String:
	return "PackedDataContainer"

func _get_import_order() -> int:
	return 0

func _get_preset_count() -> int:
	return Presets.size()

func _get_preset_name(preset: int) -> String:
	match preset:
		Presets.DEFAULT:
			return "Default"
		_:
			return "Unknown"

func _get_import_options(path: String, preset: int) -> Array[Dictionary]:
	return [{
		name = "name",
		default_value = "",
		hint = PROPERTY_HINT_NONE,
		hint_string = "String",
	}, {
		name = "imports",
		default_value = [],
		property_hint = PROPERTY_HINT_RESOURCE_TYPE,
		hint_string = "%s/%s:PackedDataContainer" % [TYPE_OBJECT, TYPE_OBJECT],
	}]

func _get_option_visibility(path: String, option_name: StringName, options: Dictionary) -> bool:
	return true

func _import(
	source_file: String,
	save_path: String,
	options: Dictionary,
	platform_variants: Array[String],
	gen_files: Array[String],
):
	var r = WasmFile.new()

	r.name = options["name"]

	var err: int = r.__initialize(source_file, options["imports"])
	if err != OK:
		return err
	return ResourceSaver.save(r, "%s.%s" % [save_path, _get_save_extension()], ResourceSaver.FLAG_CHANGE_PATH | ResourceSaver.FLAG_COMPRESS)
