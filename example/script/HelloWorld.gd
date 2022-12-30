extends Node

signal message_emitted(msg)

export(PackedDataContainer) var wasm_file = null

var instance: Object = null

# Instance threadpool version
func _ready():
	var w: WasmFile = wasm_file
	if w == null:
		return

	var module = w.get_module()
	if module == null:
		return

	instance = InstanceHandle.new()
	instance.instantiate(module, {}, {}, self, "__log")

	for i in range(1, 4):
		for j in range(1, 4):
			instance.call_queue(
				"add", [i, j],
				self, "__on_result",
				self, "__log",
				[i, j]
			)

func __log(msg: String) -> void:
	emit_signal("message_emitted", msg)

func __on_result(ret: Array, i: int, j: int) -> void:
	__log("%s + %s = %s" % [i, j, ret[0]])

# Non threadpool version
#func _ready():
#	var w: WasmFile = wasm_file
#	if w == null:
#		return
#
#	instance = w.instantiate()
#
#	call_deferred("__cb")
#
#func __cb():
#	if instance == null:
#		return
#
#	for i in range(1, 4):
#		for j in range(1, 4):
#			var ret: Array = instance.call_wasm("add", [i, j])
#			emit_signal("message_emitted", "%s + %s = %s" % [i, j, ret[0]])
