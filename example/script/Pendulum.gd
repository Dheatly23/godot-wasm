extends Node2D

signal message_emitted(msg)

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

@onready var shaft1 := $Shaft
@onready var bulb1 := $Bulb
@onready var pendulum2 := $Pendulum2
@onready var shaft2 := $Pendulum2/Shaft
@onready var bulb2 := $Pendulum2/Bulb

var _angle1 := 0.0
var _angle2 := 0.0

func _set_pendulum(
	shaft: Node2D,
	bulb: Node2D,
	length: float,
	weight: float,
	angle: float
) -> Vector2:
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

	return t * Vector2(0, SCALE * length)

func _update_pendulum() -> void:
	var v := _set_pendulum(shaft1, bulb1, length1, mass1, _angle1)
	pendulum2.position = v
	v = _set_pendulum(shaft2, bulb2, length2, mass2, _angle2)

# Instance threadpool version
#func _ready():
#	var f: WasmFile = load(wasm_file)
#
#	var module = f.get_module()
#	if module == null:
#		__log("Cannot compile module " + wasm_file)
#		return
#
#	instance = InstanceHandle.new()
#	instance.instantiate(
#		module,
#		{},
#		{
#			"epoch.enable": true,
#			"epoch.timeout": 1,
#		},
#		self, "__log"
#	)
#
#	instance.call_queue(
#		"setup",
#		[
#			mass1,
#			mass2,
#			length1,
#			length2,
#			timestep,
#			_angle1,
#			velocity1,
#			_angle2,
#			velocity2,
#		],
#		null, "",
#		self, "__log"
#	)
#
#var queued := 0
#
#func _process(delta):
#	if instance == null:
#		return
#
#	if queued < 3:
#		queued += 1
#		instance.call_queue(
#			"process", [delta],
#			self, "__update",
#			self, "__log"
#		)
#	else:
#		printerr("WASM Call takes too long! Maybe a bug?")
#
#func __log(msg: String) -> void:
#	emit_signal("message_emitted", msg)
#
#func __update(ret: Array) -> void:
#	var p = ret[0]
#	angle1 = instance.inst.get_double(p)
#	velocity1 = instance.inst.get_double(p + 8)
#	angle2 = instance.inst.get_double(p + 16)
#	velocity2 = instance.inst.get_double(p + 24)
#
#	_update_pendulum()
#
#	queued -= 1

# Non threadpool version
func _ready():
	instance = wasm_file.instantiate({}, {})

	__setup.call_deferred()

func _process(delta):
	if instance == null:
		return

	var ret: Array = instance.call_wasm("process", [delta])
	var p = ret[0]
	_angle1 = instance.get_double(p)
	velocity1 = instance.get_double(p + 8)
	_angle2 = instance.get_double(p + 16)
	velocity2 = instance.get_double(p + 24)

	_update_pendulum()

func __setup():
	if instance == null:
		return

	instance.call_wasm("setup", [
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
