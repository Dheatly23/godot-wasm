extends Node

signal message_emitted(msg)

@export_file("*.wasm", "*.wat") var wasm_file := ""

var instance: WasmInstance = null

# Instance threadpool version
#func _ready():
#	var f: WasmFile = load(wasm_file)
#
#	var module := f.get_module()
#	if module == null:
#		__log("Cannot compile module " + wasm_file)
#		return
#
#	instance = InstanceHandle.new()
#	instance.instantiate(module, {}, {}, Callable(self, "__log"))
#
#	for i in range(1, 4):
#		for j in range(1, 4):
#			instance.call_queue(
#				"add", [i, j],
#				Callable(self, "__on_result").bind(i, j),
#				Callable(self, "__log"),
#			)
#
#func __log(msg: String) -> void:
#	emit_signal("message_emitted", msg)
#
#func __on_result(ret: Array, i: int, j: int) -> void:
#	__log("%s + %s = %s" % [i, j, ret[0]])

# Non threadpool version
func _ready():
	var f: WasmFile = load(wasm_file)

	instance = f.instantiate()

	call_deferred("__cb")

func __cb():
	if instance == null:
		return

	for i in range(1, 4):
		for j in range(1, 4):
			var ret: Array = instance.call_wasm("add", [i, j])
			message_emitted.emit("%s + %s = %s" % [i, j, ret[0]])
