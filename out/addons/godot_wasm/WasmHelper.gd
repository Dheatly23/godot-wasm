@tool
class_name WasmHelper

const TYPE_I32 = 1
const TYPE_I64 = 2
const TYPE_F32 = 3
const TYPE_F64 = 4
const TYPE_VARIANT = 6

static func load_wasm(
	name: String,
	data,
	imports: Dictionary = {}
) -> WasmModule:
	return WasmModule.new().initialize(
		name,
		data,
		imports
	)

static func load_wasm_file(
	name: String,
	path: String,
	imports: Dictionary = {}
) -> WasmModule:
	var file := FileAccess.open(path, FileAccess.READ)
	var buf = file.get_buffer(file.get_length())
	file.close()
	return load_wasm(name, buf, imports)

static func __leb128_u64(buf: PackedByteArray, start: int) -> Dictionary:
	var ret := 0
	var v := 0
	for i in range(0, 64, 7):
		v = buf[start]
		start += 1
		var v_ := (v & 127) << i

		if (v_ >> i) != (v & 127):
			printerr("Value overflow!")
			return {error = true}
		ret |= v_
		if v & 128 == 0:
			break

	if v & 128 != 0:
		printerr("Value overflow!")
		return {error = true}

	return {
		value = ret,
		cursor = start,
		error = false,
	}

static func get_custom_sections(data: PackedByteArray) -> Dictionary:
	var ret := {}

	var wasm_header := PackedByteArray([
		0x00, 0x61, 0x73, 0x6d,
		0x01, 0x00, 0x00, 0x00,
	])

	if data.slice(0, 7) != wasm_header:
		printerr("Header error!")
		return {}

	var i := 8
	while i != len(data):
		var id := data[i]

		var temp := __leb128_u64(data, i)
		if temp["error"]:
			return {}

		var section_len: int = temp["value"]
		i = temp["cursor"]

		match id:
			0:
				var end := i + section_len
				temp = __leb128_u64(data, i)
				if temp["error"]:
					return {}

				var name_len: int = temp["value"]
				i = temp["cursor"]

				var name := data.slice(i, i + name_len - 1).get_string_from_utf8()
				i += name_len
				var section_data := data.slice(i, end - 1)
				i = end

				if name in ret:
					ret[name].append(section_data)
				else:
					ret[name] = [section_data]

			_:
				# Skip section
				i += section_len

	return ret
