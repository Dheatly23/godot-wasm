extends Control

onready var labels: Array = [
	$Label0,
	$Label1,
	$Label2,
	$Label3,
	$Label4,
	$Label5,
]

func add_line(msg: String):
	for i in range(len(labels) - 1, 0, -1):
		labels[i].text = labels[i - 1].text
	labels[0].text = msg
