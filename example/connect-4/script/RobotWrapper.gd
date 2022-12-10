# Wrapper class for robot
#
# Run the robot in different thread to not block
# main thread when thinking

extends Reference
class_name RobotWrapper

# Thread handle
var _handle: Thread = Thread.new()
# Flag to stop
var _stop: bool = false

# Use a token passing through two condition,
# one for main and one for worker thread
var _cond: Semaphore = Semaphore.new()
var _ret_cond: Semaphore = Semaphore.new()

# WebAssembly call parameters and results
var _name
var _arg
var _ret

# Start thread and push a token
func instantiate(instance):
	_handle.start(self, "_run", instance)
	_ret_cond.post()

# Push RPC to worker thread
func start_call(name: String, arg: Array):
	if _stop:
		printerr("Error already stopped!")
		return

	# Acquire token
	_ret_cond.wait()

	# Set parameters
	_name = name
	_arg = arg
	_ret = null

	# Send token to worker thread
	_cond.post()

# Get a single result
# Returns nil if result not ready
func get_result():
	# Try to acquire token
	if _ret_cond.try_wait() == ERR_BUSY:
		# No result available
		return

	# Store result
	var ret = _ret
	_ret = null

	# Resend token
	_ret_cond.post()

	return ret

# Wait for result
func wait_result():
	if _stop:
		printerr("Error already stopped!")
		return

	# Acquire token
	_ret_cond.wait()

	# Store result
	var ret = _ret
	_ret = null

	# Resend token
	_ret_cond.post()

	return ret

# Stop and join thread
func join():
	if _stop:
		printerr("Error already stopped!")
		return

	# Acquire token
	_ret_cond.wait()

	# Signal stop
	_stop = true

	# Send token to worker thread
	_cond.post()

	# Wait until stoppage
	_handle.wait_to_finish()

# Runner function
func _run(instance):
	while true:
		# Acquire token
		_cond.wait()

		# Stop if signalled
		if _stop:
			return

		# Call into WebAssembly
		_ret = instance.call_wasm(_name, _arg)
		_name = null
		_arg = null

		# Send token to main thread
		_ret_cond.post()
