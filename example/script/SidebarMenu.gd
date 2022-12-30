tool
extends Container
class_name SidebarMenu

enum SidebarAnchor {
	LEFT = 1
	RIGHT
	TOP
	BOTTOM
}

export(SidebarAnchor) var side := SidebarAnchor.LEFT setget set_side
export(float, 0, 1) var offset: float = 0 setget set_offset

func set_side(v: int):
	if side != v:
		queue_sort()
	side = v

func set_offset(v: float):
	if offset != v:
		queue_sort()
	offset = v

func _get_minimum_size() -> Vector2:
	var sz := Vector2.ZERO
	for c in get_children():
		if c is Control:
			var t: Vector2 = c.get_minimum_size()
			sz.x = max(sz.x, t.x)
			sz.y = max(sz.y, t.y)
	return sz

func _notification(what):
	if what == NOTIFICATION_SORT_CHILDREN:
		for child in get_children():
			if child is Control:
				__set_child(child)

func __set_child(child: Control):
	match side:
		SidebarAnchor.LEFT:
			var t := child.get_combined_minimum_size()
			t.y = max(t.y, rect_size.y)
			var p := Vector2(-t.x, 0) * offset
			child.rect_position = p
			child.rect_size = t

		SidebarAnchor.RIGHT:
			var t := child.get_combined_minimum_size()
			t.y = max(t.y, rect_size.y)
			var p := Vector2(rect_size.x, 0)
			p += Vector2(t.x, 0) * (offset - 1)
			child.rect_position = p
			child.rect_size = t

		SidebarAnchor.TOP:
			var t := child.get_combined_minimum_size()
			t.x = max(t.x, rect_size.x)
			var p := Vector2(0, -t.y) * offset
			child.rect_position = p
			child.rect_size = t

		SidebarAnchor.BOTTOM:
			var t := child.get_combined_minimum_size()
			t.x = max(t.x, rect_size.x)
			var p := Vector2(0, rect_size.y)
			p += Vector2(0, t.y) * (offset - 1)
			child.rect_position = p
			child.rect_size = t
