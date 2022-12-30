# Handler for calling WASM instance

tool
extends Reference

var name: String
var args: Array
var ret_obj: Object
var ret_method: String
var ret_binds: Array
var err_obj: Object
var err_method: String
var err_binds: Array

func _init(
	name: String = "",
	args: Array = [],
	ret_obj: Object = null,
	ret_method: String = "",
	err_obj: Object = null,
	err_method: String = "",
	ret_binds: Array = [],
	err_binds: Array = []
):
	self.name = name
	self.args = args
	self.ret_obj = ret_obj
	self.ret_method = ret_method
	self.err_obj = err_obj
	self.err_method = err_method
	self.ret_binds = ret_binds
	self.err_binds = err_binds

func _call(inst: Object):
	inst.connect(
		"error_happened",
		self,
		"_error_handle",
		[],
		CONNECT_ONESHOT
	)

	var ret = inst.call_wasm(name, args)
	if ret != null and ret_obj != null:
		ret = [ret]
		ret.append_array(ret_binds)
		InstanceThreadpoolAutoload.queue_call_main(
			ret_obj,
			ret_method,
			ret
		)

func _error_handle(msg: String):
	if err_obj != null:
		var a: Array = [msg]
		a.append_array(err_binds)
		InstanceThreadpoolAutoload.queue_call_main(
			err_obj,
			err_method,
			a
		)
