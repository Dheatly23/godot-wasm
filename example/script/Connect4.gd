extends Node2D

@warning_ignore("unused_signal")
signal message_emitted(msg: String)

enum TileState {
	EMPTY = 0,
	YELLOW = 1,
	RED = 2,
}
const ATLAS_MAPPING: Dictionary = {
	TileState.EMPTY: Vector2i(0, 1),
	TileState.YELLOW: Vector2i(0, 0),
	TileState.RED: Vector2i(1, 0),
}

const WIDTH = 7
const HEIGHT = 5
const TILE_SIZE = 32

@export var wasm_file: WasmModule

@onready var tiles: TileMap = $Tiles
@onready var selector: Node2D = $Tiles/Selector

var state: Array
var turn: int = TileState.YELLOW
var game_end: bool = false

var robot_instance: WasmInstance = null
var task_id = null

func init_game() -> void:
	state = []
	turn = TileState.YELLOW
	game_end = false

	for x in range(WIDTH):
		for y in range(HEIGHT):
			state.append(TileState.EMPTY)
			tiles.set_cell(0, Vector2i(x, y), 0, ATLAS_MAPPING[TileState.EMPTY])

	task_id = WorkerThreadPool.add_task(__start)

func __start():
	robot_instance = WasmInstance.new().initialize(
		wasm_file,
		{},
		{
			"epoch.enable": true,
			"epoch.timeout": 60,
		},
	)
	robot_instance.error_happened.connect(__log)
	robot_instance.call_wasm("init", [WIDTH, HEIGHT])

func get_state(x: int, y: int) -> int:
	return state[x * HEIGHT + y]

func set_state(x: int, y: int, v: int) -> void:
	state[x * HEIGHT + y] = v
	tiles.set_cell(0, Vector2i(x, HEIGHT - 1 - y), 0, ATLAS_MAPPING[v])

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

#	robot_instance.call_queue(
#		"make_move", [move],
#		self, "__robot_move",
#		self, "__log"
#	)

	__log("Robot is thinking")
	if task_id != null:
		WorkerThreadPool.wait_for_task_completion(task_id)
	task_id = WorkerThreadPool.add_task(__robot_move.bind(move))

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
	elif event is InputEventMouseButton and not event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		var pos: Vector2 = (get_viewport_transform().inverse() * event.position - tiles.position) / TILE_SIZE
		if Rect2(Vector2.ZERO, Vector2(WIDTH, HEIGHT)).has_point(pos):
			get_viewport().set_input_as_handled()
			var x: int = int(pos.x)
			if not game_end and turn == TileState.YELLOW:
				do_move(x)
				robot_think(x)

func _exit_tree() -> void:
	if task_id != null:
		WorkerThreadPool.wait_for_task_completion(task_id)
		task_id = null

func __log(msg: String) -> void:
	call_thread_safe(&"emit_signal", &"message_emitted", msg)

func __robot_move(move: int) -> void:
	var ret = robot_instance.call_wasm("make_move", [move])
	if ret != null:
		do_move.call_deferred(ret[0])
