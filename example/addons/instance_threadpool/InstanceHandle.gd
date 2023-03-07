# Instance handler, has internal queue to maintain dispatch lock

@tool
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
	false # _lock.lock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed
	if _instantiate and not _is_exec:
		_is_exec = true
		InstanceThreadpoolAutoload._push_queue(x)
		ret = true
	false # _lock.unlock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

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

	false # _lock.lock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed
	_queue.push_back(exec)
	if not _is_exec:
		_is_exec = true
		InstanceThreadpoolAutoload._push_queue(self)
	false # _lock.unlock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

func _call():
	var stamp: int = Time.get_ticks_msec()
	false # _lock.lock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

	while len(_queue) > 0:
		var exec: CallHandle = _queue.pop_front()
		false # _lock.unlock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

		if inst != null:
			exec._call(inst)

		if (Time.get_ticks_msec() - stamp) > 1000:
			InstanceThreadpoolAutoload._push_queue(self)
			return

		false # _lock.lock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

	_is_exec = false
	false # _lock.unlock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

func _finalize_inst():
	_instantiate = false
	if len(_queue) == 0:
		_is_exec = false
	else:
		InstanceThreadpoolAutoload._push_queue(self)
