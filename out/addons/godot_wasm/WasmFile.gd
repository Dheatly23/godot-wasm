tool
extends PackedDataContainer
class_name WasmFile

export(String) var name := ""
export(Dictionary) var imports := {}

var __module: WasmModule = null

func instantiate(host: Dictionary = {}, config: Dictionary = {}) -> WasmInstance:
	var m := get_module()

	if m == null:
		return null
	return m.instantiate(host, config)

func get_module() -> WasmModule:
	if __module == null:
		var im := {}
		for k in imports:
			var v = imports[k]
			if v.get_script() == get_script():
				im[k] = v.__get_module()
		__module = WasmHelper.load_wasm(name, __data__, im)

	return __module

func __initialize(path: String, imports_: Array) -> int:
	for v in imports_:
		var f: WasmFile = v
		if f == null:
			return ERR_CANT_OPEN
		if f.name in imports:
			return ERR_DUPLICATE_SYMBOL
		imports[f.name] = f

	var file := File.new()
	var err := file.open(path, File.READ)
	if err != OK:
		return err

	__data__ = file.get_buffer(file.get_len())

	file.close()
	return OK
