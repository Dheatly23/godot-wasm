extends Node

signal message_emitted(msg)

@export var wasm_file: WasmModule

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
#func __on_result(ret: Array, i: int, j: int) -> void:
#	__log("%s + %s = %s" % [i, j, ret[0]])

# Non threadpool version
func _ready():
	instance = wasm_file.instantiate({}, {})

	call_deferred("__cb")

func __cb():
	if instance == null:
		return

	instance.error_happened.connect(__log)
	for i in range(1, 4):
		for j in range(1, 4):
			var ret: Array = instance.call_wasm("add", [i, j])
			message_emitted.emit("%s + %s = %s" % [i, j, ret[0]])

	# This will always error
	#instance.call_wasm("test", [3, 4, 1])

func __log(msg: String) -> void:
	emit_signal("message_emitted", msg)
