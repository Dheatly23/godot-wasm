extends Node

signal message_emitted(msg)

export(String, FILE, "*.wasm,*.wat") var wasm_file := ""

# Instance threadpool version
func _ready():
	var f: WasmFile = load(wasm_file)

	var module := f.get_module()
	if module == null:
		__log("Cannot compile module " + wasm_file)
		return

	var wasi_ctx = WasiContext.new()
	wasi_ctx.connect("stdout_emit", self, "__bin_log")
	wasi_ctx.connect("stderr_emit", self, "__bin_log")
	wasi_ctx.bypass_stdio = false
	wasi_ctx.mount_physical_dir(".", ".")

	var instance := InstanceHandle.new()
	instance.instantiate(
		module,
		{},
		{
			"engine.use_epoch": true,
			"engine.use_wasi": true,
			"wasi.wasi_context": wasi_ctx,
			"wasi.args": ["rustpython.wasm", "--help"],
		}
	)
	instance.call_queue("__main_void", [])

func __log(msg: String) -> void:
	emit_signal("message_emitted", msg)

func __bin_log(msg: PoolByteArray) -> void:
	emit_signal("message_emitted", msg.get_string_from_utf8())

# Non threadpool version
#func _ready():
#	var f: WasmFile = load(wasm_file)
#
#	var module := f.get_module()
#	if module == null:
#		__log("Cannot compile module " + wasm_file)
#		return
#
#	wasi_ctx = WasiContext.new()
#	wasi_ctx.connect("stdout_emit", self, "__bin_log")
#	wasi_ctx.connect("stderr_emit", self, "__bin_log")
#	wasi_ctx.bypass_stdio = false
#	wasi_ctx.mount_physical_dir(".", ".")
#
#	var instance = WasmInstance.new().initialize(
#		module,
#		{},
#		{
#			"engine.use_epoch": true,
#			"engine.use_wasi": true,
#			"wasi.wasi_context": wasi_ctx,
#			"wasi.args": ["rustpython.wasm", "--help"],
#		}
#	)
#	instance.call_wasm("__main_void", [])
