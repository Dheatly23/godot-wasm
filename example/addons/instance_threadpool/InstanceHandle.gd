# Instance handler, has internal queue to maintain dispatch lock

tool
extends "Callable.gd"
class_name InstanceHandle

const CallHandle = preload("CallHandle.gd")
const Instantiator = preload("Instantiator.gd")

var inst = null

var _instantiate: bool = true
var _queue: Array = []
var _is_exec: bool = false
var _lock: Mutex = Mutex.new()

# Queue instantiation of the module
# Returns true on successful queue invocation
func instantiate(
	module,
	host: Dictionary = {},
	config: Dictionary = {},
	err_obj: Object = null,
	err_method: String = "",
	err_binds: Array = []
) -> bool:
	var x: Instantiator = Instantiator.new(
		self,
		module,
		host,
		config,
		err_obj,
		err_method,
		err_binds
	)

	var ret: bool = false
	_lock.lock()
	if _instantiate and not _is_exec:
		_is_exec = true
		InstanceThreadpoolAutoload._push_queue(x)
		ret = true
	_lock.unlock()

	return ret

# Queue WASM call
func call_queue(
	name: String,
	args: Array,
	ret_obj: Object = null,
	ret_method: String = "",
	err_obj: Object = null,
	err_method: String = "",
	ret_binds: Array = [],
	err_binds: Array = []
):
	var exec: CallHandle = CallHandle.new(
		name,
		args,
		ret_obj,
		ret_method,
		err_obj,
		err_method,
		ret_binds,
		err_binds
	)

	_lock.lock()
	_queue.push_back(exec)
	if not _is_exec:
		_is_exec = true
		InstanceThreadpoolAutoload._push_queue(self)
	_lock.unlock()

func _call():
	var stamp: int = Time.get_ticks_msec()
	_lock.lock()

	while len(_queue) > 0:
		var exec: CallHandle = _queue.pop_front()
		_lock.unlock()

		if inst != null:
			exec._call(inst)

		if (Time.get_ticks_msec() - stamp) > 1000:
			InstanceThreadpoolAutoload._push_queue(self)
			return

		_lock.lock()

	_is_exec = false
	_lock.unlock()

func _finalize_inst():
	_instantiate = false
	if len(_queue) == 0:
		_is_exec = false
	else:
		InstanceThreadpoolAutoload._push_queue(self)
