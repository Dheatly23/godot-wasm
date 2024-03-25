@tool
extends Container
class_name SidebarMenu

enum SidebarAnchor {
	LEFT = 1,
	RIGHT,
	TOP,
	BOTTOM,
}

@export var side: SidebarAnchor = SidebarAnchor.LEFT :
	set(v):
		if side != v:
			queue_sort()
		side = v

@export var offset: float = 0.0:
	set(v):
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
			t.y = max(t.y, size.y)
			var p := Vector2(-t.x, 0) * offset
			child.position = p
			child.size = t

		SidebarAnchor.RIGHT:
			var t := child.get_combined_minimum_size()
			t.y = max(t.y, size.y)
			var p := Vector2(size.x, 0)
			p += Vector2(t.x, 0) * (offset - 1)
			child.position = p
			child.size = t

		SidebarAnchor.TOP:
			var t := child.get_combined_minimum_size()
			t.x = max(t.x, size.x)
			var p := Vector2(0, -t.y) * offset
			child.position = p
			child.size = t

		SidebarAnchor.BOTTOM:
			var t := child.get_combined_minimum_size()
			t.x = max(t.x, size.x)
			var p := Vector2(0, size.y)
			p += Vector2(0, t.y) * (offset - 1)
			child.position = p
			child.size = t
