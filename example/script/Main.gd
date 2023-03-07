extends Control

@export var anim_seconds: float = 1

@export var scenes := [] # (Array, PackedScene)
@export var names := [] # (Array, String)

@onready var tween := create_tween()
@onready var sidebar := $SidebarMenu
@onready var panel := $SidebarMenu/Panel
@onready var detector := $Detect
@onready var view := $SubViewportContainer/SubViewport
@onready var logger := $LogContainer

var child_scene: Node = null

func _show_menu():
	print("Show")
	tween.kill()
	tween = create_tween()
	tween.tween_property(
		sidebar,
		"offset",
		0.0,
		anim_seconds
	).set_trans(Tween.TRANS_CUBIC).set_ease(Tween.EASE_OUT)
	tween.play()

func _hide_menu():
	print(Rect2(detector.position, detector.size))
	print(get_local_mouse_position())
	if Rect2(
		detector.position,
		detector.size
	).has_point(get_local_mouse_position()):
		return

	print("Hide")
	tween.kill()
	tween = create_tween()
	tween.tween_property(
		sidebar,
		"offset",
		1.0,
		anim_seconds
	).set_trans(Tween.TRANS_CUBIC).set_ease(Tween.EASE_OUT)
	tween.play()

func _ready():
	detector.offset_left -= panel.size.x

	var box := $SidebarMenu/Panel/Scroller/VBox

	for i in range(len(names)):
		var button := Button.new()

		button.text = names[i]
		button.alignment = HORIZONTAL_ALIGNMENT_LEFT
		button.connect("pressed", Callable(self,"__load_scene").bind(names[i], scenes[i]))

		box.add_child(button)

func __load_scene(name: String, scene: PackedScene) -> void:
	if child_scene != null:
		view.remove_child(child_scene)
		child_scene.queue_free()

	logger.clear_log()
	logger.add_text("Loading example: %s" % [name])

	child_scene = scene.instantiate(PackedScene.GEN_EDIT_STATE_DISABLED)
	child_scene.connect("message_emitted",Callable(logger,"add_text"))
	view.add_child(child_scene)

func _on_Viewport_gui_input(event):
	print(event)
