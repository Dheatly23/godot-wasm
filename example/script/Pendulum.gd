extends Node2D

@warning_ignore("unused_signal")
signal message_emitted(msg: String)

const SCALE := 64.0

@export var wasm_file: WasmModule

@export_range(0.001, 10) var mass1 := 1.0
@export_range(0.001, 10) var length1 := 1.0
@export_range(0.001, 10) var mass2 := 1.0
@export_range(0.001, 10) var length2 := 1.0
@export_range(0.001, 1) var timestep := 0.001

@export_range(-180, 180, 0.1) var angle1: float:
	get:
		return rad_to_deg(_angle1)
	set(v):
		_angle1 = deg_to_rad(v)
@export_range(0, 100, 0.01) var velocity1 := 0.0
@export_range(-180, 180, 0.1) var angle2: float:
	get:
		return rad_to_deg(_angle2)
	set(v):
		_angle2 = deg_to_rad(v)
@export_range(0, 100, 0.01) var velocity2 := 0.0

var instance: WasmInstance = null
var acc_delta := 0.0
var task_id = null

@onready var shaft1 := $Shaft
@onready var bulb1 := $Bulb
@onready var pendulum2 := $Pendulum2
@onready var shaft2 := $Pendulum2/Shaft
@onready var bulb2 := $Pendulum2/Bulb

var _angle1 := 0.0
var _angle2 := 0.0

func __set_pendulum(
	length: float,
	weight: float,
	angle: float,
	shaft: Node2D,
	bulb: Node2D,
	child: Node2D = null,
) -> void:
	var s := sin(angle)
	var c := cos(angle)
	var t := Transform2D(Vector2(c, -s), Vector2(s, c), Vector2.ZERO)

	shaft.transform = t * Transform2D(
		Vector2(min(weight, 1), 0),
		Vector2(0, length),
		Vector2.ZERO
	)

	bulb.transform = t * Transform2D(
		Vector2(weight, 0),
		Vector2(0, weight),
		Vector2(0, SCALE * length)
	)

	if child != null:
		child.position = t * Vector2(0, SCALE * length)

func __update_pendulum(a1: float, v1: float, a2: float, v2: float) -> void:
	_angle1 = a1
	velocity1 = v1
	_angle2 = a2
	velocity2 = v2

	__set_pendulum(length1, mass1, _angle1, shaft1, bulb1, pendulum2)
	__set_pendulum(length2, mass2, _angle2, shaft2, bulb2)

func _ready():
	task_id = WorkerThreadPool.add_task(__start)

func _process(delta: float):
	if instance == null:
		return

	acc_delta += delta
	if task_id == null or WorkerThreadPool.is_task_completed(task_id):
		WorkerThreadPool.wait_for_task_completion(task_id)
		task_id = WorkerThreadPool.add_task(__update.bind(acc_delta))
		acc_delta = 0.0

func _exit_tree() -> void:
	if task_id != null:
		WorkerThreadPool.wait_for_task_completion(task_id)
		task_id = null

func __start():
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
			},
		},
		{
			"epoch.enable": true,
			"epoch.timeout": 1.0,
		},
	)

	if instance == null:
		return

	var ret = instance.call_wasm("setup", [
		mass1,
		mass2,
		length1,
		length2,
		timestep,
		_angle1,
		velocity1,
		_angle2,
		velocity2,
	])
	if ret == null:
		instance = null

func __update(delta: float):
	var ret = instance.call_wasm("process", [delta])
	if ret == null:
		instance = null
		return

	var p = ret[0]
	call_thread_safe(
		&"__update_pendulum",
		instance.get_double(p),
		instance.get_double(p + 8),
		instance.get_double(p + 16),
		instance.get_double(p + 24),
	)

func __log(msg: String) -> void:
	call_thread_safe(&"emit_signal", &"message_emitted", msg)

func __wasm_log(p: int, n: int):
	var s = instance.memory_read(p, n).get_string_from_utf8()
	print(s)
	__log(s)
