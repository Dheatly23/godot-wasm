extends Node

@warning_ignore("unused_signal")
signal message_emitted(msg: String)

@export var wasm_file: WasmModule

var instance: WasmInstance = null
var mutex := Mutex.new()
var tasks: Array[int] = []

func _ready():
	mutex.lock()
	tasks.append(WorkerThreadPool.add_task(__start))
	mutex.unlock()

func __start():
	instance = wasm_file.instantiate({}, {})
	if instance == null:
		return
	instance.error_happened.connect(__log)

	mutex.lock()
	for i in range(1, 4):
		for j in range(1, 4):
			var c := func ():
				var ret: Array = instance.call_wasm(&"add", [i, j])
				__log("%s + %s = %s" % [i, j, ret[0]])
			tasks.append(WorkerThreadPool.add_task(c))

	# This will always error
	#instance.call_wasm("test", [3, 4, 1])

	mutex.unlock()

func _exit_tree() -> void:
	mutex.lock()
	while not tasks.is_empty():
		var ts := tasks
		tasks = []
		mutex.unlock()
		for t in ts:
			WorkerThreadPool.wait_for_task_completion(t)
		mutex.lock()
	mutex.unlock()

func __log(msg: String) -> void:
	call_thread_safe(&"emit_signal", &"message_emitted", msg)
