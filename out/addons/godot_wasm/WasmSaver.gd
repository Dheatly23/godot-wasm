@tool
extends ResourceFormatSaver
class_name WasmSaver

func _recognize(resource: Resource) -> bool:
	return resource is WasmModule

func _get_recognized_extensions(resource: Resource) -> PackedStringArray:
	if not _recognize(resource):
		return PackedStringArray()
	return PackedStringArray([
		"cwasm",
	])

func _save(resource: Resource, path: String, flags: int) -> Error:
	var module := resource as WasmModule
	if module == null:
		return FAILED

	var data := module.serialize()
	if not (data is PackedByteArray):
		return FAILED
	var file := FileAccess.open(path, FileAccess.WRITE)
	if file == null:
		return FileAccess.get_open_error()
	file.store_buffer(data)

	return OK
