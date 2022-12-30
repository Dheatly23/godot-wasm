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
	match preset:
		Presets.DEFAULT:
			return [{
				name = "name",
				default_value = "",
				property_hint = PROPERTY_HINT_NONE,
				usage = PROPERTY_USAGE_DEFAULT,
				hint_string = "String",
			}, {
				name = "imports",
				default_value = {},
				property_hint = PROPERTY_HINT_NONE,
				usage = PROPERTY_USAGE_DEFAULT,
				hint_string = "Dictionary",
			}]
		_:
			return []

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
	r.imports = options["imports"]

	var err: int = r.__initialize(source_file)
	if err != OK:
		return err
	return ResourceSaver.save("%s.%s" % [save_path, get_save_extension()], r)
