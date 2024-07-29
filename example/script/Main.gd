extends Control

@export var anim_seconds: float = 1

@export var scenes: Array[PackedScene] = []
@export var names: Array[String] = []

@onready var tween: Tween = null
@onready var sidebar := $SidebarMenu
@onready var panel := $SidebarMenu/Panel
@onready var detector := $Detect
@onready var view := $SubViewportContainer/SubViewport
@onready var logger := $LogContainer

var child_scene: Node = null
var is_shown := false

func _show_menu():
	if tween:
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
	if tween:
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
	var box := $SidebarMenu/Panel/Scroller/VBox
	var button: Button

	for i in range(len(names)):
		button = Button.new()

		button.text = names[i]
		button.alignment = HORIZONTAL_ALIGNMENT_LEFT
		button.pressed.connect(__load_scene.bind(names[i], scenes[i]))

		box.add_child(button)

	button = Button.new()
	button.text = "Exit"
	button.alignment = HORIZONTAL_ALIGNMENT_LEFT
	button.pressed.connect(__quit)
	box.add_child(button)

func _process(_delta):
	var shown: bool = detector \
		.get_rect() \
		.merge(panel.get_rect()) \
		.intersection(sidebar.get_rect()) \
		.has_point(get_local_mouse_position())
	if shown:
		if not is_shown:
			_show_menu()
	elif is_shown:
		_hide_menu()
	is_shown = shown

func __load_scene(scene_name: String, scene: PackedScene):
	if child_scene != null:
		view.remove_child(child_scene)
		child_scene.queue_free()

	logger.clear_log()
	logger.add_text("Loading example: %s" % [scene_name])

	child_scene = scene.instantiate(PackedScene.GEN_EDIT_STATE_DISABLED)
	if child_scene.has_signal("message_emitted"):
		child_scene.message_emitted.connect(logger.add_text)
	view.add_child(child_scene)

func __quit():
	var tree := get_tree()
	tree.root.propagate_notification(NOTIFICATION_WM_CLOSE_REQUEST)
	tree.quit()
