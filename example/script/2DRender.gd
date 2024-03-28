extends Node2D

signal message_emitted(msg)

export(String, FILE, "*.wasm,*.wat") var wasm_file := ""

onready var wasi_ctx: WasiContext = WasiContext.new()
onready var crypto := Crypto.new()

onready var _tex := ImageTexture.new()
onready var _img := Image.new()
onready var _lbl: Label = $UI/Root/Panel/VBox/Label

var module: WasmModule = null
var instance: WasmInstance = null

func __selected(index):
	instance = WasmInstance.new()
	instance.connect("error_happened", self, "__emit_log")
	instance = instance.initialize(
		module,
		{
			"log": {
				params = [WasmHelper.TYPE_I32, WasmHelper.TYPE_I32],
				results = [],
				object = self,
				method = "__log",
			},
			"rand": {
				params = [WasmHelper.TYPE_I32, WasmHelper.TYPE_I32],
				results = [],
				object = self,
				method = "__rand",
			},
		},
		{
			"epoch.enable": true,
			"epoch.timeout": 1.0,
			"wasi.enable": true,
			"wasi.context": wasi_ctx,
		}
	)

	if instance == null:
		emit_signal("message_emitted", "Failed to instantiate module")
		return
	if instance.call_wasm("init", [index]) == null:
		emit_signal("message_emitted", "Failed to call init")

func _ready():
	$Sprite.texture = _tex

	$UI/Root/Panel/VBox/TypeLst.select(1)

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

	__selected(1)

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
	_lbl.text = "WASM Time: %.3f ms" % ((end - start) / 1e3)

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

func _input(event: InputEvent):
	if instance == null:
		return

	if (event is InputEventMouseButton) and (not event.is_pressed()):
		var p := get_global_mouse_position()
		p -= $Sprite.get_rect().position
		instance.call_wasm("click", [p.x, p.y, event.button_index - 1])

func __emit_log(msg):
	emit_signal("message_emitted", msg.strip_edges())

func __log(p: int, n: int):
	var s = instance.memory_read(p, n).get_string_from_utf8()
	print(s)
	emit_signal("message_emitted", s)

func __rand(p: int, n: int):
	var b := crypto.generate_random_bytes(n)
	instance.memory_write(p, b)
