tool
extends "Callable.gd"

var obj: Object
var method: String
var args: Array

func _init(
	obj: Object = null,
	method: String = "",
	args: Array = []
):
	self.obj = obj
	self.method = method
	self.args = args

func _call():
	obj.callv(method, args)
