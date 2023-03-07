# Thread pool runner object

@tool
extends RefCounted

const Callable = preload("Callable.gd")
const Controller = preload("InstanceThreadpoolAutoload.gd")

var handle: Thread = Thread.new()

func _run(c: Controller):
	while true:
		c._sema.wait()
		false # c._lock.lock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

		if len(c._queue) == 0:
			false # c._lock.unlock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed
			return

		var v: Callable = c._queue.pop_front()
		false # c._lock.unlock() # TODOConverter40, Image no longer requires locking, `false` helps to not break one line if/else, so it can freely be removed

		v._call()

func _start(v) -> int:
	return handle.start(Callable(self,"_run").bind(v))

func _stop():
	handle.wait_to_finish()
