tool
extends Node

const TYPE_I32 = 1
const TYPE_I64 = 2
const TYPE_F32 = 3
const TYPE_F64 = 4
const TYPE_VARIANT = 6

static func load_wasm(
	name: String,
	data,
	imports: Dictionary = {}
):
	return preload("WasmModule.gdns").new().initialize(
		name,
		data,
		imports
	)

static func load_wasm_file(
	name: String,
	path: String,
	imports: Dictionary = {}
):
	var file: File = File.new()
	file.open(path, File.READ)
	var buf = file.get_buffer(file.get_len())
	file.close()
	return load_wasm(name, buf, imports)
