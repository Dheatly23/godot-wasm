extends Node3D

@warning_ignore("unused_signal")
signal message_emitted(msg: String)

@export var wasm_file: WasmModule

@onready var wasi_ctx: WasiContext = WasiContext.new()
@onready var crypto := Crypto.new()

@onready var _mesh := ArrayMesh.new()
@onready var _lbl: Label = $UI/Root/Panel/VBox/Label

var instance: WasmInstance = null
var acc_delta := 0.0
var task_id = null

func __instantiate() -> bool:
	instance = WasmInstance.new()
	instance.error_happened.connect(__log)
	instance = instance.initialize(
		wasm_file,
		{
			"host": {
				"log": {
					params = [WasmHelper.TYPE_I32, WasmHelper.TYPE_I32],
					results = [],
					callable = __wasm_log,
				},
				"rand": {
					params = [WasmHelper.TYPE_I32, WasmHelper.TYPE_I32],
					results = [],
					callable = __rand,
				},
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
		__log("Failed to instantiate module")
	return instance != null

func __selected(index) -> void:
	var c := func ():
		if !__instantiate():
			return

		if instance.call_wasm("init", [index]) == null:
			__log("Failed to call init")
			instance = null

	if task_id != null:
		WorkerThreadPool.wait_for_task_completion(task_id)
	task_id = WorkerThreadPool.add_task(c)

func _ready():
	wasi_ctx.stdout_emit.connect(__log)
	wasi_ctx.stderr_emit.connect(__log)
	task_id = WorkerThreadPool.add_task(__start)

func __start():
	if !__instantiate():
		return

	var ret = instance.call_wasm(&"config", [])
	if ret == null:
		__log("Failed to call config")
		instance = null
		return

	var items := []
	var p: int = ret[0]
	var cp := instance.get_32(p)
	var cl := instance.get_32(p + 4)
	for o in range(0, cl * 8, 8):
		var sp := cp + o
		var s := instance.memory_read(
			instance.get_32(sp),
			instance.get_32(sp + 4),
		).get_string_from_utf8()
		items.append(s)

	var c := func ():
		$Mesh.mesh = _mesh

		var item_list: ItemList = $UI/Root/Panel/VBox/TypeLst
		for s in items:
			item_list.add_item(s)

		item_list.select(0)

		__selected(0)
	c.call_deferred()

func _process(delta: float) -> void:
	if instance == null:
		return

	acc_delta += delta
	if task_id == null or WorkerThreadPool.is_task_completed(task_id):
		WorkerThreadPool.wait_for_task_completion(task_id)
		task_id = WorkerThreadPool.add_task(__update.bind(acc_delta))
		acc_delta = 0.0

func __update(delta: float) -> void:
	var start := Time.get_ticks_usec()
	var ret = instance.call_wasm(&"process", [delta])
	if ret == null:
		__log("Failed to call process")
		instance = null
		return
	var end := Time.get_ticks_usec()

	var p: int = ret[0]
	if p == 0:
		return
	var data := []
	data.resize(Mesh.ARRAY_MAX)
	data[Mesh.ARRAY_VERTEX] = instance.get_array(
		instance.get_32(p),
		instance.get_32(p + 4),
		TYPE_PACKED_VECTOR3_ARRAY
	)
	data[Mesh.ARRAY_NORMAL] = instance.get_array(
		instance.get_32(p + 8),
		instance.get_32(p + 12),
		TYPE_PACKED_VECTOR3_ARRAY
	)
	data[Mesh.ARRAY_TANGENT] = instance.get_array(
		instance.get_32(p + 16),
		instance.get_32(p + 20) * 4,
		TYPE_PACKED_FLOAT32_ARRAY
	)
	data[Mesh.ARRAY_TEX_UV] = instance.get_array(
		instance.get_32(p + 24),
		instance.get_32(p + 28),
		TYPE_PACKED_VECTOR2_ARRAY
	)
	data[Mesh.ARRAY_COLOR] = instance.get_array(
		instance.get_32(p + 32),
		instance.get_32(p + 36),
		TYPE_PACKED_COLOR_ARRAY
	)
	data[Mesh.ARRAY_INDEX] = instance.get_array(
		instance.get_32(p + 40),
		instance.get_32(p + 44),
		TYPE_PACKED_INT32_ARRAY
	)

	var c := func ():
		_lbl.text = "WASM Time: %.3f ms" % ((end - start) / 1e3)

		_mesh.clear_surfaces()
		if len(data[Mesh.ARRAY_INDEX]) != 0:
			_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, data)
	c.call_deferred()

func _exit_tree() -> void:
	if task_id != null:
		WorkerThreadPool.wait_for_task_completion(task_id)
		task_id = null

func __ui_input(event: InputEvent) -> void:
	if instance == null:
		return

	if (event is InputEventMouseButton) and (not event.is_pressed()):
		var p: Vector2 = event.position
		var cam: Camera3D = $Camera
		var orig := cam.project_ray_origin(p)
		var norm := cam.project_ray_normal(p)
		instance.call_wasm(
			&"click",
			[
				orig.x, orig.y, orig.z,
				norm.x, norm.y, norm.z,
				event.button_index - 1,
			],
		)

func __log(msg: String) -> void:
	call_thread_safe(&"emit_signal", &"message_emitted", msg)

func __wasm_log(p: int, n: int) -> void:
	var s = instance.memory_read(p, n).get_string_from_utf8()
	print(s)
	__log(s)

func __rand(p: int, n: int):
	var b := crypto.generate_random_bytes(n)
	instance.memory_write(p, b)
