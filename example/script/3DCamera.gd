extends Camera3D

const PI_2 := PI / 2
const SPEED := PI / 16

@export var dist := 10.0

var rx := PI / 4
var ry := PI / -4

func _process(delta: float):
	delta *= SPEED

	var up_pressed := Input.get_action_strength("camera_up") >= 0.1
	var down_pressed := Input.get_action_strength("camera_down") >= 0.1
	var left_pressed := Input.get_action_strength("camera_left") >= 0.1
	var right_pressed := Input.get_action_strength("camera_right") >= 0.1
	var sprint_pressed := Input.get_action_strength("camera_sprint") >= 0.1

	if sprint_pressed:
		delta *= 4
	if up_pressed and not down_pressed:
		ry = clamp(ry - delta, -PI_2, PI_2)
	elif not up_pressed and down_pressed:
		ry = clamp(ry + delta, -PI_2, PI_2)
	if left_pressed and not right_pressed:
		rx = wrapf(rx - delta, -PI, PI)
	elif not left_pressed and right_pressed:
		rx = wrapf(rx + delta, -PI, PI)

	var q := Quaternion.from_euler(Vector3(ry, rx, 0))
	var v := q * (Vector3.BACK * dist)
	quaternion = q
	position = v
