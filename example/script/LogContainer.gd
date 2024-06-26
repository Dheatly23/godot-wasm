extends VBoxContainer

@export_range(0, 100) var label_count := 0

var labels := []

func _ready():
	for _i in range(label_count):
		var l := Label.new()
		l.text = ""
		l.horizontal_alignment = HORIZONTAL_ALIGNMENT_LEFT
		add_child(l)
		labels.append(l)

func add_text(s: String) -> void:
	var a := s.split("\n", true)
	if len(a) >= len(labels):
		for i in range(-len(labels), 0, 1):
			labels[i].text = a[i]
	else:
		var l := len(a)
		for i in range(len(labels) - l):
			labels[i].text = labels[i + l].text
		for i in range(-l, 0, 1):
			labels[i].text = a[i]

func clear_log() -> void:
	for l in labels:
		l.text = ""
