extends Window

@onready var data_dialog: ConfirmationDialog = $DataDialog
@onready var data_dialog_text: TextEdit = $DataDialog/Margin/Text
@onready var data_dialog_grid := $DataDialog/Margin/Grid
@onready var data_dialog_host: LineEdit = $DataDialog/Margin/Grid/HostTxt
@onready var data_dialog_guest: LineEdit = $DataDialog/Margin/Grid/GuestTxt

@onready var arg_popup: PopupMenu = $Panel/Margin/Tabs/Arguments/Popup
@onready var arg_list: Tree = $Panel/Margin/Tabs/Arguments/List

@onready var env_popup: PopupMenu = $Panel/Margin/Tabs/Environment/Popup
@onready var env_list: Tree = $Panel/Margin/Tabs/Environment/List

@onready var mount_popup: PopupMenu = $Panel/Margin/Tabs/Mount/Popup
@onready var mount_list: Tree = $Panel/Margin/Tabs/Mount/List

var wasi_context: WasiContext
var __data_cb := Callable()
var __arg_sel = null
var __env_sel = null
var __mount_sel = null

func get_args() -> Array[String]:
	var ret: Array[String] = []
	var root := arg_list.get_root()
	for i in range(root.get_child_count()):
		ret.push_back(root.get_child(i).get_text(0))
	return ret

func get_envs() -> Dictionary:
	var ret := {}
	var root := env_list.get_root()
	for i in range(root.get_child_count()):
		var node := root.get_child(i)
		ret[node.get_text(0)] = node.get_text(1)
	return ret

func _ready() -> void:
	var ok := data_dialog.get_ok_button()
	var cancel := data_dialog.get_cancel_button()
	cancel.focus_next = cancel.get_path_to(data_dialog_text)
	ok.focus_previous = ok.get_path_to(data_dialog_text)
	data_dialog_text.focus_next = data_dialog_text.get_path_to(ok)
	data_dialog_text.focus_previous = data_dialog_text.get_path_to(cancel)

	arg_list.create_item()

	env_list.create_item()
	env_list.set_column_title(0, "Key")
	env_list.set_column_expand(0, true)
	env_list.set_column_expand_ratio(0, 1)
	env_list.set_column_title(1, "Value")
	env_list.set_column_expand(1, true)
	env_list.set_column_expand_ratio(1, 1)

	mount_list.create_item()
	mount_list.set_column_title(0, "Guest")
	mount_list.set_column_expand(0, true)
	mount_list.set_column_expand_ratio(0, 1)
	mount_list.set_column_title(1, "Host")
	mount_list.set_column_expand(1, true)
	mount_list.set_column_expand_ratio(1, 1)

func __data_ok() -> void:
	var v := __data_cb
	__data_cb = Callable()
	v.call()

func __open_data_dialog(dialog_title: String, text: String, cb: Callable) -> void:
	data_dialog.title = dialog_title
	data_dialog_text.show()
	data_dialog_grid.hide()
	data_dialog_text.text = text
	__data_cb = cb
	data_dialog.popup_centered_clamped(Vector2i(50, 30))
	data_dialog_text.grab_focus.call_deferred()

func __arg_clicked(mouse_position: Vector2, mouse_button_index: int, is_empty: bool) -> void:
	if mouse_button_index != MOUSE_BUTTON_RIGHT:
		return

	if is_empty:
		arg_list.deselect_all()
	var sel := arg_list.get_next_selected(null)
	if sel == null:
		__arg_sel = null
		arg_popup.set_item_text(0, "Add")
		arg_popup.set_item_disabled(1, true)
	else:
		__arg_sel = sel.get_index()
		arg_popup.set_item_text(0, "Insert")
		arg_popup.set_item_disabled(1, false)

	arg_popup.popup(Rect2i(Vector2i(get_screen_transform() * mouse_position), Vector2i(0, 0)))

func __arg_add_item() -> void:
	var child := arg_list.get_root().create_child(__arg_sel + 1 if __arg_sel is int else -1)
	child.set_text(0, data_dialog_text.text)
	child.set_editable(0, true)
	child.set_edit_multiline(0, true)

func __arg_popup_selected(index: int) -> void:
	match index:
		0:
			__open_data_dialog(
				"New Argument",
				"",
				__arg_add_item,
			)
		1:
			arg_list.get_root().get_child(__arg_sel).free()

func __env_clicked(mouse_position: Vector2, mouse_button_index: int, is_empty: bool) -> void:
	if mouse_button_index != MOUSE_BUTTON_RIGHT:
		return

	if is_empty:
		env_list.deselect_all()
	var sel := env_list.get_next_selected(null)
	if sel == null:
		__env_sel = null
		env_popup.set_item_disabled(1, true)
	else:
		__env_sel = sel.get_index()
		env_popup.set_item_disabled(1, false)

	env_popup.popup(Rect2i(Vector2i(get_screen_transform() * mouse_position), Vector2i(0, 0)))

func __env_add_item() -> void:
	var root := env_list.get_root()
	var v := data_dialog_text.text

	var ix := -1
	for i in range(root.get_child_count()):
		var t := root.get_child(i).get_text(0)
		if t == v:
			return
		elif t > v:
			ix = i
			break

	var child := root.create_child(ix)
	child.set_text(0, v)
	child.set_editable(1, true)
	child.set_edit_multiline(1, true)

func __env_popup_selected(index: int) -> void:
	match index:
		0:
			__open_data_dialog(
				"New Environment Variable Key",
				"",
				__env_add_item,
			)
		1:
			var child := env_list.get_root().get_child(__env_sel)
			child.free()

func __mount_clicked(mouse_position: Vector2, mouse_button_index: int, is_empty: bool) -> void:
	if mouse_button_index != MOUSE_BUTTON_RIGHT:
		return

	if is_empty:
		mount_list.deselect_all()
	var sel := mount_list.get_next_selected(null)
	if sel == null:
		__mount_sel = null
		mount_popup.set_item_disabled(1, true)
	else:
		__mount_sel = sel.get_index()
		mount_popup.set_item_disabled(1, false)

	mount_popup.popup(Rect2i(Vector2i(get_screen_transform() * mouse_position), Vector2i(0, 0)))

func __mount_refresh() -> void:
	var root := mount_list.get_root()
	var data: Dictionary = wasi_context.get_mounts()
	var i := 0
	for k in data:
		var v = data[k]

		var node: TreeItem
		if root.get_child_count() <= i:
			node = root.create_child()
		else:
			node = root.get_child(i)

		node.set_text(0, k)
		node.set_text(1, v)
		node.set_editable(1, true)
		i += 1

	while root.get_child_count() > i:
		var node := root.get_child(-1)
		root.remove_child(node)
		node.free()

func __mount_add_item() -> void:
	wasi_context.mount_physical_dir(data_dialog_host.text, data_dialog_guest.text)
	__mount_refresh()

func __mount_edit_item() -> void:
	var sel := mount_list.get_next_selected(null)
	if sel == null:
		return

	wasi_context.mount_physical_dir(sel.get_text(0), sel.get_text(1))
	__mount_refresh()

func __mount_popup_selected(index: int) -> void:
	match index:
		0:
			data_dialog.title = "New Mount Point"
			data_dialog_text.hide()
			data_dialog_grid.show()
			data_dialog_host.text = ""
			data_dialog_guest.text = ""
			__data_cb = __mount_add_item
			data_dialog.popup_centered_clamped(Vector2i(50, 30))
		1:
			var child := mount_list.get_root().get_child(__mount_sel)
			wasi_context.unmount_physical_dir(child.get_text(0))
			__mount_refresh()
