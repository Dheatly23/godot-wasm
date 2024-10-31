extends Camera2D

const SPEED := 32.0

func _process(delta: float):
	delta *= SPEED

	if Input.is_action_pressed("camera_sprint"):
		delta *= 32
	position += delta * Vector2(
		Input.get_action_strength("camera_right") - Input.get_action_strength("camera_left"),
		Input.get_action_strength("camera_down") - Input.get_action_strength("camera_up"),
	)
