tool
class_name WasmHelper

const TYPE_I32 = 1
const TYPE_I64 = 2
const TYPE_F32 = 3
const TYPE_F64 = 4
const TYPE_VARIANT = 6

const WASM_HEADER = PoolByteArray([
	0x00, 0x61, 0x73, 0x6d,
	0x01, 0x00, 0x00, 0x00,
])

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
	var file: File = File.new()
	file.open(path, File.READ)
	var buf = file.get_buffer(file.get_len())
	file.close()
	return load_wasm(name, buf, imports)

static func __leb128_u64(buf: PoolByteArray, start: int) -> Dictionary:
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

static func get_custom_sections(data: PoolByteArray) -> Dictionary:
	var ret := {}

	if data.subarray(0, 7) != WASM_HEADER:
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

				var name := data.subarray(i, i + name_len - 1).get_string_from_utf8()
				i += name_len
				var section_data := data.subarray(i, end - 1)
				i = end

				if name in ret:
					ret[name].append(section_data)
				else:
					ret[name] = [section_data]

			_:
				# Skip section
				i += section_len

	return ret
