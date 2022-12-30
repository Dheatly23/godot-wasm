extends Node2D

signal message_emitted(msg)

enum TileState {
	EMPTY = 0
	YELLOW = 1
	RED = 2
}

const WIDTH = 7
const HEIGHT = 5
const TILE_SIZE = 32

export(PackedDataContainer) var wasm_file = null

onready var tiles: TileMap = $Tiles
onready var selector: Node2D = $Tiles/Selector

var state: Array
var turn: int = TileState.YELLOW
var game_end: bool = false

var robot_instance: InstanceHandle = null

func init_game() -> void:
	state = []
	turn = TileState.YELLOW
	game_end = false

	for x in range(WIDTH):
		for y in range(HEIGHT):
			state.append(TileState.EMPTY)
			tiles.set_cell(x, y, TileState.EMPTY)

	var w: WasmFile = wasm_file
	if w == null:
		return

	var module = w.get_module()
	if module == null:
		return

	robot_instance = InstanceHandle.new()
	robot_instance.instantiate(
		module,
		{},
		{
			"engine.use_epoch": true,
			"engine.epoch_timeout": 60,
		},
		self, "__log"
	)
	robot_instance.call_queue(
		"init", [WIDTH, HEIGHT],
		null, "",
		self, "__log"
	)

func get_state(x: int, y: int) -> int:
	return state[x * HEIGHT + y]

func set_state(x: int, y: int, v: int) -> void:
	state[x * HEIGHT + y] = v
	tiles.set_cell(x, HEIGHT - 1 - y, v)

func do_move(x: int) -> void:
	if game_end:
		return

	var y: int = HEIGHT - 1
	while y >= 0 and get_state(x, y) == TileState.EMPTY:
		y -= 1
	y += 1

	if y >= HEIGHT:
		match turn:
			TileState.YELLOW:
				__log(
					"Player tried to make a move in row {0}".format([
						x + 1,
					])
				)
			TileState.RED:
				__log(
					"Robot tried to make a move in row {0}".format([
						x + 1,
					])
				)
		return

	set_state(x, y, turn)

	match turn:
		TileState.YELLOW:
			__log(
				"Player make a move in cell ({0} {1})".format([
					x + 1,
					y + 1,
				])
			)
		TileState.RED:
			__log(
				"Robot make a move in cell ({0} {1})".format([
					x + 1,
					y + 1,
				])
			)

	var found: bool = false
	if x >= 3:
		if y >= 3:
			found = true
			for i in range(1, 4):
				if get_state(x-i, y-i) != turn:
					found = false
					break
		if not found and y < HEIGHT - 3:
			found = true
			for i in range(1, 4):
				if get_state(x-i, y+i) != turn:
					found = false
					break
		if not found:
			found = true
			for i in range(1, 4):
				if get_state(x-i, y) != turn:
					found = false
					break
	if not found and x < WIDTH - 3:
		if y >= 3:
			found = true
			for i in range(1, 4):
				if get_state(x+i, y-i) != turn:
					found = false
					break
		if not found and y < HEIGHT - 3:
			found = true
			for i in range(1, 4):
				if get_state(x+i, y+i) != turn:
					found = false
					break
		if not found:
			found = true
			for i in range(1, 4):
				if get_state(x+i, y) != turn:
					found = false
					break
	if not found and y >= 3:
		found = true
		for i in range(1, 4):
			if get_state(x, y-i) != turn:
				found = false
				break
	if not found and y < HEIGHT - 3:
		found = true
		for i in range(1, 4):
			if get_state(x, y+i) != turn:
				found = false
				break

	if found:
		game_end = true
		match turn:
			TileState.YELLOW:
				__log("Player won!")
			TileState.RED:
				__log("Robot won!")
		return

	match turn:
		TileState.RED:
			turn = TileState.YELLOW
		TileState.YELLOW:
			turn  = TileState.RED

func robot_think(move: int):
	if game_end:
		return

	robot_instance.call_queue(
		"make_move", [move],
		self, "__robot_move",
		self, "__log"
	)
	__log("Robot is thinking")

func _ready():
	tiles.position = -Vector2(TILE_SIZE * WIDTH, TILE_SIZE * HEIGHT) / 2
	init_game()
	__log("Game started")

func _input(event):
	if event is InputEventMouseMotion:
		var pos: Vector2 = (get_viewport_transform().inverse() * event.position - tiles.position) / TILE_SIZE
		if Rect2(Vector2.ZERO, Vector2(WIDTH, HEIGHT)).has_point(pos):
			selector.visible = true
			selector.position = pos.floor() * TILE_SIZE
		else:
			selector.visible = false
	elif event is InputEventMouseButton and not event.pressed and event.button_index == BUTTON_LEFT:
		var pos: Vector2 = (get_viewport_transform().inverse() * event.position - tiles.position) / TILE_SIZE
		if Rect2(Vector2.ZERO, Vector2(WIDTH, HEIGHT)).has_point(pos):
			get_tree().set_input_as_handled()
			var x: int = int(pos.x)
			if not game_end and turn == TileState.YELLOW:
				do_move(x)
				robot_think(x)

func __log(msg: String) -> void:
	emit_signal("message_emitted", msg)

func __robot_move(res: Array) -> void:
	if turn == TileState.RED and len(res) >= 1:
		do_move(res[0])
