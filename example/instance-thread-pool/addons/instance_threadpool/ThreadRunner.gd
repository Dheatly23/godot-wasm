# Thread pool runner object

tool
extends Reference

const Callable = preload("Callable.gd")
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

		v._call()

func _start(v) -> int:
	return handle.start(self, "_run", v)

func _stop():
	handle.wait_to_finish()
