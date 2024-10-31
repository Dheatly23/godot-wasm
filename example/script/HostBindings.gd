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
			"write": {
				params = [
					WasmHelper.TYPE_I32,
					WasmHelper.TYPE_I32,
				],
				results = [],
				callable = __write,
			},
		},
	}, {})

	if instance == null:
		return

	instance.error_happened.connect(__log)
	instance.call_wasm(&"main", [])

func __write(ptr: int, sz: int) -> void:
	var buf: PackedByteArray = instance.memory_read(ptr, sz)
	__log(buf.get_string_from_utf8())

func __log(msg: String) -> void:
	call_thread_safe(&"emit_signal", &"message_emitted", msg)
