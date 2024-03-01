extends Control

signal message_emitted(msg)

@export_file("*.wasm","*.wat") var wasm_file := ""

@onready var wasi_ctx: WasiContext = WasiContext.new()

@onready var file_tree: Tree = $Center/Panel/Margin/VBox/FileEdit/VBox2/FileTree
@onready var file_title: LineEdit = $Center/Panel/Margin/VBox/FileEdit/VBox/HBoxContainer/FileLabel
@onready var file_text := $Center/Panel/Margin/VBox/FileEdit/VBox/TextBox
@onready var file_popup: PopupMenu = $PopupFileMenu
@onready var file_name_dialog := $FileNameDialog
@onready var file_name_dialog_text: LineEdit = $FileNameDialog/Box/LineEdit

@onready var arg_list: ItemList = $ArgEnvDialog/Panel/Margin/Tabs/Arguments/Args
@onready var env_list: ItemList = $"ArgEnvDialog/Panel/Margin/Tabs/Environment Variables/Envs"
@onready var mnt_list: ItemList = $ArgEnvDialog/Panel/Margin/Tabs/Mounts/Mounts

@onready var arg_dialog_text: TextEdit = $ArgEnvDialog/ArgDialog/ArgTxt
@onready var env_dialog_key: LineEdit = $ArgEnvDialog/EnvDialog/Grid/KeyTxt
@onready var env_dialog_val: TextEdit = $ArgEnvDialog/EnvDialog/Grid/ValTxt
@onready var mnt_dialog_host: LineEdit = $ArgEnvDialog/MountDialog/Grid/HostTxt
@onready var mnt_dialog_guest: LineEdit = $ArgEnvDialog/MountDialog/Grid/GuestTxt

@onready var exec_file_box := $Center/Panel/Margin/VBox/HBox/ExecFile

var select_file_cmd := 0
var create_file := false
var edited_arg_ix := 0
var edited_env_ix := 0

var wasm_module: WasmModule = null
var last_file_path: String = ""

func _ready():
#	wasi_ctx.connect("stdout_emit", self, "__emit_log")
#	wasi_ctx.connect("stderr_emit", self, "__emit_log")

	wasi_ctx.file_make_dir(".", "python", null)
	wasi_ctx.file_make_file("python", "hello_world.py", null)
	wasi_ctx.file_write("python/hello_world.py", """# hello_world.py

print('Hello from Python!')
""", null, null, null)
	wasi_ctx.file_make_file("python", "primes.py", null)
	wasi_ctx.file_write("python/primes.py", """# primes.py

def primes(n):
	r = [2]
	for i in range(3, n):
		if all(i % j for j in r):
			r.append(i)
	return r

print("First 1000 primes:")
p = primes(1000)
for i in range(0, len(p), 16):
	print(*(f"{x:3d}" for x in p[i:i+16]))
""", null, null, null)
	wasi_ctx.file_make_file("python", "read_file.py", null)
	wasi_ctx.file_write("python/read_file.py", """# read_file.py

print("Reading data/text.txt")
with open("data/text.txt", "rt") as f:
	for l in f:
		print(l.strip())
""", null, null, null)
	wasi_ctx.file_make_file("python", "write_file.py", null)
	wasi_ctx.file_write("python/write_file.py", """# write_file.py

import json

print("Writing data/output.json")
with open("data/output.json", "wt") as f:
	json.dump({
		"first_name": "Jack",
		"last_name": "Coe",
		"alive": False,
		"profession": "preacher",
	}, f)
""", null, null, null)
	wasi_ctx.file_make_dir(".", "js", null)
	wasi_ctx.file_make_file("js", "hello_world.js", null)
	wasi_ctx.file_write("js/hello_world.js", """// hello_world.js

console.log('Hello from Javascript!')
""", null, null, null)
	wasi_ctx.file_make_dir(".", "data", null)
	wasi_ctx.file_make_file("data", "text.txt", null)
	wasi_ctx.file_write("data/text.txt", """
A text data read by read_file.py

I don't really feel like putting Lorem Ipsum here :)
""", null, null, null)

	file_popup.add_item("Create New File")
	file_popup.add_item("Create New Folder")
	file_popup.add_separator()
	file_popup.add_item("Delete File")

	file_tree.set_column_clip_content(0, false)
	file_tree.set_column_expand(0, true)
	file_tree.set_column_custom_minimum_width(0, 1000)

	__refresh_files()

func __exec_file_pressed():
	$ExecFileDialog.popup_centered_clamped(
		Vector2(500, 500),
		get_viewport_rect().size.aspect()
	)

func __select_exec_file(path):
	exec_file_box.text = path

func __list_tree_item(path: String, tree: TreeItem = null):
	if wasi_ctx.file_is_exist(path, null) != 2:
		return
	var items = wasi_ctx.file_dir_list(path, false)
	if items == null:
		return

	for i in items:
		var p := "%s/%s" % [path, i]
		var t := file_tree.create_item(tree)
		t.set_cell_mode(0, TreeItem.CELL_MODE_STRING)
		t.set_text(0, i)
		t.set_metadata(0, p)
		__list_tree_item(p, t)

func __refresh_files():
	file_tree.clear()
	var root := file_tree.create_item()
	root.set_text(0, ".")
	root.set_metadata(0, ".")
	__list_tree_item(".", root)

func __open_file_context(position: Vector2, mouse_button_index: int):
	if mouse_button_index != MOUSE_BUTTON_RIGHT:
		return
	var t := file_tree.get_selected()
	if t == null:
		return
	var path: String = t.get_metadata(0)

	var is_not_dir: bool = wasi_ctx.file_is_exist(path, null) != 2
	file_popup.set_item_disabled(0, is_not_dir)
	file_popup.set_item_disabled(1, is_not_dir)

	position += file_tree.get_global_position()
	file_popup.popup(Rect2(position, Vector2(50, 10)))

func __select_popup(id):
	match id:
		0:
			create_file = true
			file_name_dialog.title = "New File Name"
			file_name_dialog_text.text = ""
			file_name_dialog.popup_centered_clamped(Vector2(150, 50))
		1:
			create_file = false
			file_name_dialog.title = "New Folder Name"
			file_name_dialog_text.text = ""
			file_name_dialog.popup_centered_clamped(Vector2(150, 50))
		3:
			var t := file_tree.get_selected()
			if t == null:
				return
			var path: String = t.get_parent().get_metadata(0)
			var name := t.get_parent().get_text(0)

			if !wasi_ctx.file_delete_file(path, name, false):
				message_emitted.emit("Cannot delete file")

			__refresh_files()

func __create_file():
	var t := file_tree.get_selected()
	if t == null:
		return
	var path: String = t.get_metadata(0)

	var name := file_name_dialog_text.text
	if name == "":
		return

	if create_file:
		if !wasi_ctx.file_make_file(path, name, false):
			message_emitted.emit("Cannot create file")
	else:
		if !wasi_ctx.file_make_dir(path, name, false):
			message_emitted.emit("Cannot create folder")

	__refresh_files()

func __open_file():
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

	file_title.text = path
	file_text.text = content

func __save_file():
	var path := file_title.text
	if path == "":
		return
	if !wasi_ctx.file_write(path, file_text.text, 0, true, true):
		message_emitted.emit("Cannot save file!")

func __emit_log(msg):
	message_emitted.emit(msg)

func __file_name_dialog_entered(_new_text):
	file_name_dialog.get_ok().pressed.emit()

func __open_arg_dialog():
	$ArgEnvDialog.popup_centered_clamped(
		Vector2(400, 200),
		get_viewport_rect().size.aspect()
	)

func __add_argument():
	var i := arg_list.get_selected_items()
	var j := arg_list.get_item_count()

	arg_list.add_item("")
	if !i.is_empty():
		arg_list.move_item(j, i[0])
		j = i[0]

	arg_list.select(j)
	__edit_argument(j)

func __delete_argument():
	var i := arg_list.get_selected_items()
	if i.is_empty():
		return
	arg_list.remove_item(i[0])

func __edit_argument(index: int):
	edited_arg_ix = index
	arg_dialog_text.text = arg_list.get_item_text(index)
	$ArgEnvDialog/ArgDialog.popup_centered_clamped(Vector2(150, 150))

func __edited_argument():
	arg_list.set_item_text(edited_arg_ix, arg_dialog_text.text)

func __add_environment():
	var i := env_list.get_selected_items()
	edited_env_ix = -1
	if !i.is_empty():
		edited_env_ix = i[0]

	env_dialog_key.text = ""
	env_dialog_val.text = ""
	env_dialog_key.editable = true
	$ArgEnvDialog/EnvDialog.popup_centered_clamped(
		Vector2(200, 100),
		get_viewport_rect().size.aspect()
	)

func __delete_environment():
	var i := env_list.get_selected_items()
	if i.is_empty():
		return
	var j := i[0]
	wasi_ctx.delete_env_variable(env_list.get_item_metadata(j))
	env_list.remove_item(j)

func __edit_environment(index):
	edited_env_ix = index

	var k: String = env_list.get_item_metadata(index)
	env_dialog_key.text = k
	env_dialog_val.text = wasi_ctx.get_env_variable(k)
	env_dialog_key.editable = false
	$ArgEnvDialog/EnvDialog.popup_centered_clamped(
		Vector2(200, 100),
		get_viewport_rect().size.aspect()
	)

func __edited_environment():
	if env_dialog_key.editable:
		var k := env_dialog_key.text
		var v := env_dialog_val.text
		if wasi_ctx.get_env_variable(k) != null:
			return

		wasi_ctx.add_env_variable(k, v)
		var j := env_list.get_item_count()
		env_list.add_item("%s : %s" % [k, v])
		env_list.set_item_metadata(j, k)
		if edited_env_ix != -1:
			env_list.move_item(j, edited_env_ix)
	else:
		var k := env_dialog_key.text
		var v := env_dialog_val.text
		wasi_ctx.add_env_variable(k, v)
		env_list.set_item_text(edited_env_ix, "%s : %s" % [k, v])

func __refresh_mounts():
	var d: Dictionary = wasi_ctx.get_mounts()
	var j := 0
	mnt_list.clear()
	for k in d:
		var v = d[k]
		mnt_list.add_item("%s : %s" % [k, v])
		mnt_list.set_item_metadata(j, k)
		j += 1

func __add_mount():
	mnt_dialog_host.text = ""
	mnt_dialog_guest.text = ""
	$ArgEnvDialog/MountDialog.popup_centered_clamped(
		Vector2(200, 100),
		get_viewport_rect().size.aspect()
	)

func __delete_mount():
	var i := mnt_list.get_selected_items()
	if i.is_empty():
		return
	wasi_ctx.unmount_physical_dir(mnt_list.get_item_metadata(i[0]))
	__refresh_mounts()

func __added_mount():
	var host := mnt_dialog_host.text
	var guest := mnt_dialog_guest.text
	if host == "" or guest == "":
		return
	wasi_ctx.mount_physical_dir(host, guest)
	__refresh_mounts()

func __open_mount_file():
	$ArgEnvDialog/MountDialog/MountFileDialog.popup_centered_clamped(
		Vector2(500, 500),
		get_viewport_rect().size.aspect()
	)

func __select_mount_file(dir):
	mnt_dialog_host.text = dir

func __execute():
	if wasm_module == null or last_file_path != exec_file_box.text:
		last_file_path = exec_file_box.text
		wasm_module = WasmHelper.load_wasm_file(last_file_path)
	if wasm_module == null:
		message_emitted.emit("Cannot open executable!")
		return

	message_emitted.emit("Running file")
	var args := ["wasm_file"]
	for i in range(0, arg_list.get_item_count()):
		args.append(arg_list.get_item_text(i))

	var instance := WasmInstance.new().initialize(
		wasm_module,
		{},
		{
			"wasi.enable": true,
			"wasi.context": wasi_ctx,
			"wasi.args": args,
		}
	)

	instance.call_wasm(&"_start", [])
	__refresh_files()
