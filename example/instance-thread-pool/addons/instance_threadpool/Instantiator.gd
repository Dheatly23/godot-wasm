# Handler for module instantiation

tool
extends "Callable.gd"

var handle
var module
var host: Dictionary
var config: Dictionary
var err_obj: Object
var err_method: String
var err_binds: Array

func _init(
	handle,
	module,
	host: Dictionary = {},
	config: Dictionary = {},
	err_obj: Object = null,
	err_method: String = "",
	err_binds: Array = []
):
	self.handle = handle
	self.module = module
	self.host = host
	self.config = config
	self.err_obj = err_obj
	self.err_method = err_method
	self.err_binds = err_binds

func _call():
	var inst = WasmInstance.new()
	inst.connect(
		"error_happened",
		self,
		"_error_handle",
		[],
		CONNECT_ONESHOT
	)
	handle._lock.lock()
	handle.inst = inst.initialize(module, host, config)
	handle._finalize_inst()
	handle._lock.unlock()

func _error_handle(msg: String):
	if err_obj != null:
		var a: Array = [msg]
		a.append_array(err_binds)
		InstanceThreadpoolAutoload.queue_call_main(
			err_obj,
			err_method,
			a
		)
