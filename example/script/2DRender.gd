extends Node2D

signal message_emitted(msg)

export(String, FILE, "*.wasm,*.wat") var wasm_file := ""

onready var wasi_ctx: WasiContext = WasiContext.new()

onready var _tex := ImageTexture.new()
onready var _img := Image.new()

var module: WasmModule = null
var instance: WasmInstance = null

func __selected(index):
	instance = WasmInstance.new().initialize(
		module,
		{},
		{
			"epoch.enable": true,
			"epoch.timeout": 1.0,
			"wasi.enable": true,
			"wasi.context": wasi_ctx,
		}
	)
	if instance == null:
		emit_signal("message_emitted", "Failed to instantiate module")
	if instance.call_wasm("init", [index]) == null:
		emit_signal("message_emitted", "Failed to call init")

func _ready():
	$Sprite.texture = _tex

	wasi_ctx.connect("stdout_emit", self, "__emit_log")
	wasi_ctx.connect("stderr_emit", self, "__emit_log")

	var file: WasmFile = load(wasm_file)
	if file == null:
		emit_signal("message_emitted", "Failed to load module")
		return
	module = file.get_module()
	if module == null:
		emit_signal("message_emitted", "Failed to load module")
		return

	instance = WasmInstance.new().initialize(
		module,
		{},
		{
			"epoch.enable": true,
			"epoch.timeout": 1.0,
			"wasi.enable": true,
			"wasi.context": wasi_ctx,
		}
	)
	if instance == null:
		emit_signal("message_emitted", "Failed to instantiate module")
	if instance.call_wasm("init", [0]) == null:
		emit_signal("message_emitted", "Failed to call init")

func _process(delta):
	if instance == null:
		return

	var start := Time.get_ticks_usec()
	var ret = instance.call_wasm("process", [delta])
	if ret == null:
		emit_signal("message_emitted", "Failed to call process")
		instance = null
		return
	var end := Time.get_ticks_usec()
	__emit_log("WASM Time: %.3f ms" % ((end - start) / 1e3))

	var p: int = ret[0]
	if p == 0:
		return
	var width: int = instance.get_32(p)
	var height: int = instance.get_32(p + 4)
	p = instance.get_32(p + 8)
	var data = instance.memory_read(p, width * height * 4)

	if len(data) != 0:
		var b := _img.get_width() == width and _img.get_height() == height
		_img.create_from_data(width, height, false, Image.FORMAT_RGBA8, data)
		if b:
			_tex.set_data(_img)
		else:
			_tex.create_from_image(_img, Texture.FLAG_FILTER | Texture.FLAG_REPEAT)

func __emit_log(msg):
	emit_signal("message_emitted", msg.strip_edges())
