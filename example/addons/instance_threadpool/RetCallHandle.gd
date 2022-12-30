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
	if is_instance_valid(obj):
		obj.callv(method, args)
