extends Node2D

signal message_emitted(msg)

func _ready():
	emit_signal("message_emitted", """
		Starting double pendulum problem.
		This example shows how a chaotic system will behave.
		The pendulums are 0.01 degrees different.
	""")


func _on_Timer_timeout():
	emit_signal("message_emitted", "")

	for p in $Pendulums.get_children():
		if p is preload("res://script/Pendulum.gd"):
			emit_signal("message_emitted", "%s: [%6.1f %6.1f %6.1f %6.1f]" % [
				p.name,
				p.angle1,
				p.velocity1,
				p.angle2,
				p.velocity2,
			])

func __log(msg: String):
	emit_signal("message_emitted", msg)
