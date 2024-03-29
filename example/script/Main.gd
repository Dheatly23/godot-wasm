extends Control

export(float) var anim_seconds: float = 1

export(Array, PackedScene) var scenes := []
export(Array, String) var names := []

onready var tween := $Tween
onready var sidebar := $SidebarMenu
onready var panel := $SidebarMenu/Panel
onready var detector := $Detect
onready var view := $ViewportContainer/Viewport
onready var logger := $LogContainer

var child_scene: Node = null
var is_shown := false

func _show_menu():
	tween.interpolate_property(
		sidebar,
		"offset",
		null,
		0,
		anim_seconds,
		Tween.TRANS_CUBIC,
		Tween.EASE_OUT
	)
	tween.start()

func _hide_menu():
	if Rect2(
		detector.rect_position,
		detector.rect_size
	).has_point(get_local_mouse_position()):
		return

	tween.interpolate_property(
		sidebar,
		"offset",
		null,
		1,
		anim_seconds,
		Tween.TRANS_CUBIC,
		Tween.EASE_OUT
	)
	tween.start()

func _ready():
	var box := $SidebarMenu/Panel/Scroller/VBox

	for i in range(len(names)):
		var button := Button.new()

		button.text = names[i]
		button.align = Button.ALIGN_LEFT
		button.connect("pressed", self, "__load_scene", [names[i], scenes[i]])

		box.add_child(button)

func _process(_delta):
	var show := Rect2(
		detector.rect_position,
		detector.rect_size
	).merge(Rect2(
		panel.rect_position,
		panel.rect_size
	)).has_point(get_local_mouse_position())
	if show:
		if not is_shown:
			_show_menu()
	elif is_shown:
		_hide_menu()
	is_shown = show

func __load_scene(name: String, scene: PackedScene) -> void:
	if child_scene != null:
		view.remove_child(child_scene)
		child_scene.queue_free()

	logger.clear_log()
	logger.add_text("Loading example: %s" % [name])

	child_scene = scene.instance(PackedScene.GEN_EDIT_STATE_DISABLED)
	child_scene.connect("message_emitted", logger, "add_text")
	view.add_child(child_scene)
