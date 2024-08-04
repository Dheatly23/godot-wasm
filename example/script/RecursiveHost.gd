extends Node

signal message_emitted(msg)

@export var wasm_file: WasmModule

var instance: WasmInstance = null

func _ready():
	instance = wasm_file.instantiate({
		"host": {
			"recurse": {
				params = [
					WasmHelper.TYPE_I64,
					WasmHelper.TYPE_I64,
				],
				results = [
					WasmHelper.TYPE_I64,
				],
				callable = __recurse,
			},
		},
	}, {})

	__cb.call_deferred()

func __cb():
	if instance == null:
		return

	instance.error_happened.connect(__log, CONNECT_ONE_SHOT)
	for i in range(1, 65):
		var ret = instance.call_wasm("recurse", [i, 0])
		if ret == null:
			__log("Error at input: %s" % [i])
			return
		__log("Input: %s Value: %s" % [i, ret[0]])

func __recurse(n: int, a: int):
	__log("Called host with n: %s a: %s" % [n, a])
	if n <= 0:
		return a
	var r = instance.call_wasm("recurse", [n - 1, n + a])
	if r == null:
		instance.signal_error("Error")
	else:
		r = r[0]
	return r

func __log(msg: String) -> void:
	message_emitted.emit(msg)
