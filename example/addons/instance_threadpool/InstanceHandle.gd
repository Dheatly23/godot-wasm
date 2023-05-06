# Instance handler, has internal queue to maintain dispatch lock

@tool
class_name InstanceHandle

var inst = null

var _instantiate: bool = true
var _queue: Array[Callable] = []
var _is_exec: bool = false
var _lock: Mutex = Mutex.new()

# Queue instantiation of the module
# Returns true on successful queue invocation
func instantiate(
	module: WasmModule,
	host: Dictionary = {},
	config: Dictionary = {},
	err_callable: Callable = Callable(),
) -> bool:
	var lambda := func ():
		var inst := WasmInstance.new()
		inst.connect("error_happened", err_callable, CONNECT_ONE_SHOT)

		_lock.lock()
		self.inst = inst.initialize(module, host, config)
		_instantiate = false
		if len(_queue) == 0:
			_is_exec = false
		else:
			InstanceThreadpoolAutoload._push_queue(Callable(self, "_call"))

		inst.disconnect("error_happened", err_callable)
		_lock.unlock()

	var ret: bool = false
	_lock.lock()
	if _instantiate and not _is_exec:
		_is_exec = true
		InstanceThreadpoolAutoload._push_queue(lambda)
		ret = true
	_lock.unlock()

	return ret

# Queue WASM call
func call_queue(
	name: String,
	args: Array,
	ret_callable: Callable = Callable(),
	err_callable: Callable = Callable(),
):
	var lambda := func (inst: WasmInstance):
		inst.connect("error_happened", err_callable, CONNECT_ONE_SHOT)
		var ret: Array = inst.call_wasm(name, args)
		if inst.is_connected("error_happened", err_callable):
			InstanceThreadpoolAutoload.queue_call_main(ret_callable.bind(ret))
			inst.disconnect("error_happened", err_callable)

	_lock.lock()
	_queue.push_back(lambda)
	if not _is_exec:
		_is_exec = true
		InstanceThreadpoolAutoload._push_queue(Callable(self, "_call"))
	_lock.unlock()

func _call():
	var stamp: int = Time.get_ticks_msec()
	_lock.lock()

	while len(_queue) > 0:
		var exec: Callable = _queue.pop_front()
		_lock.unlock()

		if inst != null:
			exec.call(inst)

		if (Time.get_ticks_msec() - stamp) > 1000:
			InstanceThreadpoolAutoload._push_queue(Callable(self, "_call"))
			return

		_lock.lock()

	_is_exec = false
	_lock.unlock()
