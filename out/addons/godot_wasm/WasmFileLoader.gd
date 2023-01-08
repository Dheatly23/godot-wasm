tool
extends EditorImportPlugin

enum Presets {
	DEFAULT
}

func get_importer_name() -> String:
	return "godot_wasm.wasm"

func get_visible_name() -> String:
	return "WASM Importer"

func get_recognized_extensions() -> Array:
	return ["wasm", "wat"]

func get_save_extension() -> String:
	return "res"

func get_resource_type() -> String:
	return "PackedDataContainer"

func get_preset_count() -> int:
	return Presets.size()

func get_preset_name(preset: int) -> String:
	match preset:
		Presets.DEFAULT:
			return "Default"
		_:
			return "Unknown"

func get_import_options(preset: int) -> Array:
	return [{
		name = "name",
		default_value = "",
		hint = PROPERTY_HINT_NONE,
		hint_string = "String",
	}, {
		name = "imports",
		default_value = [],
		property_hint = PROPERTY_HINT_RESOURCE_TYPE,
		hint_string = "17/19:PackedDataContainer",
	}]

func get_option_visibility(option, options) -> bool:
	return true

func import(
	source_file: String,
	save_path: String,
	options: Dictionary,
	platform_variants: Array,
	gen_files: Array
):
	var r = WasmFile.new()

	r.name = options["name"]

	var err: int = r.__initialize(source_file, options["imports"])
	if err != OK:
		return err
	return ResourceSaver.save("%s.%s" % [save_path, get_save_extension()], r)
