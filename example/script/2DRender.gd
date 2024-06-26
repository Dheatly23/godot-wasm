extends Node2D

signal message_emitted(msg)

@export var wasm_file: WasmModule

@onready var wasi_ctx: WasiContext = WasiContext.new()
@onready var crypto := Crypto.new()

@onready var _tex := ImageTexture.new()
@onready var _img := Image.new()
@onready var _lbl: Label = $UI/Root/Panel/VBox/Label

var instance: WasmInstance = null

func __selected(index):
	instance = WasmInstance.new()
	instance.error_happened.connect(__emit_log)
	instance = instance.initialize(
		wasm_file,
		{
			"host": {
				"log": {
					params = [WasmHelper.TYPE_I32, WasmHelper.TYPE_I32],
					results = [],
					callable = __log,
				},
				"rand": {
					params = [WasmHelper.TYPE_I32, WasmHelper.TYPE_I32],
					results = [],
					object = self,
					method = "__rand",
				},
			},
		},
		{
			"epoch.enable": true,
			"epoch.timeout": 1.0,
			"wasi.enable": true,
			"wasi.context": wasi_ctx,
		},
	)

	if instance == null:
		message_emitted.emit("Failed to instantiate module")
		return

	if instance.call_wasm("init", [index]) == null:
		message_emitted.emit("Failed to call init")

func _ready():
	$Sprite.texture = _tex

	$UI/Root/Panel/VBox/TypeLst.select(1)

#	wasi_ctx.connect("stdout_emit", self, "__emit_log")
#	wasi_ctx.connect("stderr_emit", self, "__emit_log")

	__selected(1)

func _process(delta):
	if instance == null:
		return

	var start := Time.get_ticks_usec()
	var ret = instance.call_wasm("process", [delta])
	if ret == null:
		message_emitted.emit("Failed to call process")
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
		_img.set_data(width, height, false, Image.FORMAT_RGBA8, data)
		if b:
			_tex.update(_img)
		else:
			_tex.set_image(_img)

func _input(event: InputEvent):
	if instance == null:
		return

	if (event is InputEventMouseButton) and (not event.is_pressed()):
		var p := get_global_mouse_position()
		p -= $Sprite.get_rect().position
		instance.call_wasm("click", [p.x, p.y, event.button_index - 1])

func __emit_log(msg):
	message_emitted.emit(msg.strip_edges())

func __log(p: int, n: int):
	var s = instance.memory_read(p, n).get_string_from_utf8()
	print(s)
	message_emitted.emit(s)

func __rand(p: int, n: int):
	var b := crypto.generate_random_bytes(n)
	instance.memory_write(p, b)
