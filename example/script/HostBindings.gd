extends Node

signal message_emitted(msg)

export(String, FILE, "*.wasm,*.wat") var wasm_file := ""

var instance: Object = null

# Instance threadpool version
func _ready():
	var f: WasmFile = load(wasm_file)

	var module := f.get_module()
	if module == null:
		__log("Cannot compile module " + wasm_file)
		return

	instance = InstanceHandle.new()
	instance.instantiate(
		module,
		{
			"write": {
				params = [
					WasmHelper.TYPE_I32,
					WasmHelper.TYPE_I32,
				],
				results = [],
				object = self,
				method = "__write",
			},
		},
		{
			"epoch.enable": true,
		},
		self, "__log"
	)

	instance.call_queue("main", [], null, "", self, "_log")

func __write(ptr: int, sz: int) -> void:
	var buf: PoolByteArray = instance.inst.memory_read(ptr, sz)
	InstanceThreadpoolAutoload.queue_call_main(
		self,
		"__log",
		[buf.get_string_from_utf8()]
	)

func __log(msg: String) -> void:
	emit_signal("message_emitted", msg)

# Non threadpool version
#func _ready():
#	var f: WasmFile = load(wasm_file)
#
#	instance = f.instantiate({
#		"write": {
#			params = [
#				WasmHelper.TYPE_I32,
#				WasmHelper.TYPE_I32,
#			],
#			results = [],
#			object = self,
#			method = "__write",
#		},
#	})
#
#	call_deferred("__cb")
#
#func __cb():
#	if instance == null:
#		return
#
#	instance.call_wasm("main", [])
#
#func __write(ptr: int, sz: int) -> void:
#	var buf: PoolByteArray = instance.memory_read(ptr, sz)
#	emit_signal("message_emitted", buf.get_string_from_utf8())
