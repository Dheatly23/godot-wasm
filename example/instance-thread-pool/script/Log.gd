extends VBoxContainer

onready var labels: Array = [
	$Label0,
	$Label1,
	$Label2,
	$Label3,
	$Label4,
	$Label5,
	$Label6,
	$Label7,
	$Label8,
	$Label9,
	$Label10,
	$Label11,
	$Label12,
	$Label13,
	$Label14,
	$Label15,
	$Label16,
	$Label17,
	$Label18,
	$Label19,
]

func add_line(msg: String):
	for i in range(len(labels) - 1, 0, -1):
		labels[i].text = labels[i - 1].text
	labels[0].text = msg
