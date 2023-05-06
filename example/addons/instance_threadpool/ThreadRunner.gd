# Thread pool runner object

@tool
extends RefCounted

const Controller = preload("InstanceThreadpoolAutoload.gd")

var handle: Thread = Thread.new()

func _run(c: Controller):
	while true:
		c._sema.wait()
		c._lock.lock()

		if len(c._queue) == 0:
			c._lock.unlock()
			return

		var v: Callable = c._queue.pop_front()
		c._lock.unlock()

		v.call()

func _start(v) -> int:
	return handle.start(Callable(self, "_run").bind(v))

func _stop():
	handle.wait_to_finish()

func _is_running() -> bool:
	return handle.is_alive()
