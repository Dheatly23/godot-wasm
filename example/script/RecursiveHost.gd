extends Node

@warning_ignore("unused_signal")
signal message_emitted(msg: String)

@export var wasm_file: WasmModule

var instance: WasmInstance = null
var task_id = null

func _ready():
	task_id = WorkerThreadPool.add_task(__start)

func _exit_tree() -> void:
	if task_id != null:
		WorkerThreadPool.wait_for_task_completion(task_id)
		task_id = null

func __start():
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
	call_thread_safe(&"emit_signal", &"message_emitted", msg)
