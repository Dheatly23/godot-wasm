extends Control

signal message_emitted(msg)

const PROGRAM := """
print('Hello from RustPython!')

print('First 10 fibonacci numbers:')

a, b = 1, 1
for i in range(10):
	print(a)
	a, b = b, a + b
"""

export(String, FILE, "*.wasm,*.wat") var wasm_file := ""

onready var file: WasmFile = load(wasm_file)
onready var wasi_ctx: WasiContext = WasiContext.new()

# Instance threadpool version
func _ready():
	var module := file.get_module()
	if module == null:
		__log("Cannot compile module " + wasm_file)
		return

	wasi_ctx.connect("stdout_emit", self, "__bin_log")
	wasi_ctx.connect("stderr_emit", self, "__bin_log")
	wasi_ctx.bypass_stdio = false
	wasi_ctx.write_memory_file("/test.py", PROGRAM.to_utf8())

func __log(msg: String) -> void:
	emit_signal("message_emitted", msg)

func __bin_log(msg: PoolByteArray) -> void:
	emit_signal("message_emitted", msg.get_string_from_utf8())

func __run():
	var module := file.get_module()
	if module == null:
		__log("Cannot compile module " + wasm_file)
		return

	var instance := InstanceHandle.new()
	instance.instantiate(
		module,
		{},
		{
			"engine.use_epoch": true,
			"engine.use_wasi": true,
			"wasi.wasi_context": wasi_ctx,
			"wasi.args": ["rustpython.wasm", "test.py"],
		}
	)
	instance.call_queue("_start", [])

# Non threadpool version
#func __run():
#	var module := file.get_module()
#	if module == null:
#		__log("Cannot compile module " + wasm_file)
#		return
#
#	var instance = WasmInstance.new().initialize(
#		module,
#		{},
#		{
#			"engine.use_epoch": true,
#			"engine.use_wasi": true,
#			"wasi.wasi_context": wasi_ctx,
#			"wasi.args": ["rustpython.wasm", "test.py"],
#		}
#	)
#	instance.call_wasm("__main_void", [])
