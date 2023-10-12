extends Node2D

const Pendulum = preload("res://script/Pendulum.gd")

signal message_emitted(msg)

func _ready():
	message_emitted.emit("""
		Starting double pendulum problem.
		This example shows how a chaotic system will behave.
		The pendulums are 0.01 degrees different.
	""")


func _on_Timer_timeout():
	message_emitted.emit("")

	for p in $Pendulums.get_children():
		if p is Pendulum:
			message_emitted.emit("%s: [%6.1f %6.1f %6.1f %6.1f]" % [
				p.name,
				p.angle1,
				p.velocity1,
				p.angle2,
				p.velocity2,
			])

func __log(msg: String):
	message_emitted.emit(msg)
