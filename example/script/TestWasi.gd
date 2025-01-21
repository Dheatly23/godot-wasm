extends Control

const FOLDER := "../wasm"

@warning_ignore("unused_signal")
signal message_emitted(msg: String)

@onready var wasi_ctx: WasiContext = WasiContext.new().initialize(null)

@onready var item_list: ItemList = $Center/Panel/VBox/HBox/VBox2/Items
@onready var file_tree: Tree = $Center/Panel/VBox/HBox/VBox/Tree
@onready var file_text: CodeEdit = $Center/Panel/VBox/HBox/Content

var modules := {}

var task_id = null

func _exit_tree() -> void:
	if task_id is int:
		WorkerThreadPool.wait_for_task_completion(task_id)

func _ready() -> void:
	wasi_ctx.bypass_stdio = true

	wasi_ctx.file_make_dir("/", "in", true)
	wasi_ctx.file_write("/in/hello", "Hello from file!", 0, true, true)
	wasi_ctx.file_make_dir("/in", "empty", true)
	for i in range(10):
		wasi_ctx.file_make_file("/in/empty", "file%d" % i, true)
	wasi_ctx.file_make_dir("/", "out", true)

	task_id = WorkerThreadPool.add_task(__load_modules, true)

func __load_modules() -> void:
	var items: Array[String] = [];

	var path := ProjectSettings.globalize_path("res://").path_join(FOLDER)
	var dir := DirAccess.open(path)
	dir.list_dir_begin()
	var file := dir.get_next()
	while file != "":
		if file.ends_with(".wasm"):
			print("Open module: %s" % file)
			var module := WasmHelper.load_wasm_file(path.path_join(file))
			if module != null:
				modules[file] = module
				items.push_back(file)

		file = dir.get_next()

	__post_load.call_deferred(items)
	__refresh_files.call_deferred()

func __post_load(arr: Array[String]) -> void:
	$Center/Panel/VBox/RunTest.disabled = false
	for i in arr:
		item_list.add_item(i, null, false)

func __list_tree_item(path: String, tree: TreeItem = null) -> void:
	if wasi_ctx.file_is_exist(path, null) != 2:
		return
	var items = wasi_ctx.file_dir_list(path, false)
	if items == null:
		return

	for i in items:
		var p := path.path_join(i)
		var t := file_tree.create_item(tree)
		t.set_cell_mode(0, TreeItem.CELL_MODE_STRING)
		t.set_text(0, i)
		t.set_metadata(0, p)
		__list_tree_item(p, t)

func __refresh_files() -> void:
	file_tree.clear()
	var root := file_tree.create_item()
	root.set_text(0, "/")
	root.set_metadata(0, "/")
	__list_tree_item("/", root)

func __open_file() -> void:
	var t := file_tree.get_selected()
	if t == null:
		return
	var path: String = t.get_metadata(0)

	if wasi_ctx.file_is_exist(path, null) != 1:
		return

	var content = wasi_ctx.file_read(path, 1_000_000, 0, true)
	if content == null:
		message_emitted.emit("Cannot open file!")
		return
	content = content.get_string_from_utf8()

	file_text.text = content

func __run_test() -> void:
	message_emitted.emit("Running tests")
	if task_id is int:
		WorkerThreadPool.wait_for_task_completion(task_id)

	var items: Array[String] = []
	for i in range(item_list.item_count):
		items.push_back(item_list.get_item_text(i))
	items.sort()
	var preview2: CheckBox = $Center/Panel/VBox/HBox/VBox2/Preview2
	task_id = WorkerThreadPool.add_task(__test.bind(items, preview2.button_pressed), true)

func __test(arr: Array[String], preview2: bool) -> void:
	for k in arr:
		print("Running module: %s" % k)

		var m: WasmModule = modules[k]
		var config := {
			"epoch.enable": true,
			"epoch.timeout": 1.0,
			"wasi.enable": true,
			"wasi.context": wasi_ctx,
			"wasi.stdout": "context",
			"wasi.stderr": "context",
		}

		if preview2:
			var inst := WasiCommand.new().initialize(m, config)
			if inst == null:
				continue

			inst.run()
		else:
			var inst := WasmInstance.new().initialize(m, {}, config)
			if inst == null:
				continue

			inst.call_wasm(&"_start", [])

	__refresh_files.call_deferred()
