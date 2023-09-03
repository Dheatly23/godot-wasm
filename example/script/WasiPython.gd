extends Control

signal message_emitted(msg)

export(String, FILE, "*.wasm,*.wat") var wasm_file := ""

onready var code_textbox: TextEdit = $Container/Panel/Margin/Tab/VBox/Code

onready var file: WasmFile = load(wasm_file)
onready var wasi_ctx: WasiContext = WasiContext.new()

func __check_file() -> bool:
	if file == null:
		printerr("Failed to import %s" % [wasm_file])
		return false
	return true

# Instance threadpool version
func _ready():
	if not __check_file():
		return

	var module := file.get_module()
	if module == null:
		__log("Cannot compile module " + wasm_file)
		return

	wasi_ctx.connect("stdout_emit", self, "__log")
	wasi_ctx.connect("stderr_emit", self, "__log")
	wasi_ctx.bypass_stdio = false
	wasi_ctx.write_memory_file("hello_world.py", "print('Hello from Python!')")
	wasi_ctx.write_memory_file("primes.py", """
def primes(n):
	r = [2]
	for i in range(3, n):
		if all(i % j for j in r):
			r.append(i)
	return r

print("First 1000 primes:")
p = primes(1000)
for i in range(0, len(p), 16):
	print(*(f"{x:3d}" for x in p[i:i+16]))
""")
	wasi_ctx.write_memory_file("data/text.txt", """
A text data read by read_file.py

I don't really feel like putting Lorem Ipsum here :)
""")
	wasi_ctx.write_memory_file("read_file.py", """
print("Reading data/text.txt")
with open("data/text.txt", "rt") as f:
	for l in f:
		print(l.strip())
""")
	wasi_ctx.write_memory_file("write_file.py", """
import json

print("Writing data/output.json")
with open("data/output.json", "wt") as f:
	json.dump({
		"first_name": "Jack",
		"last_name": "Coe",
		"alive": False,
		"profession": "preacher",
	}, f)
""")

func __log(msg: String) -> void:
	emit_signal("message_emitted", msg.strip_edges())

func __run(source: String, ret_method = ""):
	if not __check_file():
		return

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
			"wasi.args": ["rustpython.wasm", source],
		}
	)
	instance.call_queue(
		"_start",
		[],
		self if ret_method != "" else null,
		ret_method
	)

# Non threadpool version
#func __run(source: String):
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
#			"wasi.args": ["rustpython.wasm", source],
#		}
#	)
#	var r = instance.call_wasm("_start", [])
#	if ret_method != "":
#		self.call(ret_method, r)

func __cb_write_file(_v):
	var b = wasi_ctx.read_memory_file("data/output.json", 1000)
	if not (b is PoolByteArray):
		__log("Cannot read file data/output.json")
		return

	var r := JSON.parse(b.get_string_from_utf8())
	if r.error != OK:
		__log("Error processing JSON")
		__log(r.error_string)
		return

	__log("data/output.json : %s" % r.result)

func __run_custom():
	if not __check_file():
		return

	wasi_ctx.write_memory_file("test.py", code_textbox.text)
	__run("test.py")
