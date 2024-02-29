extends Camera2D

const SPEED := 32.0

func _process(delta: float):
	delta *= SPEED

	var up_pressed := Input.get_action_strength("camera_up") >= 0.1
	var down_pressed := Input.get_action_strength("camera_down") >= 0.1
	var left_pressed := Input.get_action_strength("camera_left") >= 0.1
	var right_pressed := Input.get_action_strength("camera_right") >= 0.1
	var sprint_pressed := Input.get_action_strength("camera_sprint") >= 0.1

	if sprint_pressed:
		delta *= 32
	if up_pressed and not down_pressed:
		position.y -= delta
	elif not up_pressed and down_pressed:
		position.y += delta
	if left_pressed and not right_pressed:
		position.x-= delta
	elif not left_pressed and right_pressed:
		position.x += delta
