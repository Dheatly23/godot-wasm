extends Camera3D

const PI_2 := PI / 2
const SPEED := PI / 16

@export var dist := 10.0

var rx := PI / 4
var ry := PI / -4

func _process(delta: float):
	delta *= SPEED

	if Input.is_action_pressed("camera_sprint"):
		delta *= 4
	ry = clamp(
		ry + delta * (Input.get_action_strength("camera_down") - Input.get_action_strength("camera_up")),
		-PI_2, PI_2,
	)
	rx = wrapf(
		rx + delta * (Input.get_action_strength("camera_right") - Input.get_action_strength("camera_left")),
		-PI, PI,
	)

	var q := Quaternion.from_euler(Vector3(ry, rx, 0))
	var v := q * (Vector3.BACK * dist)
	quaternion = q
	position = v
