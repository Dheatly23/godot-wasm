extends Control

const INTERVAL = 10

# A buggy factorial implementation
# It will run infinitely for *almost* every input
# Challenge: Fix the module
const WAT = """
(module
	(func
		(export "fact")
		(param $num i64)
		(result i64)
		(local $out i64)
		i64.const 1
		local.set $out
		block $b
			loop $l
				local.get $num
				i64.eqz
				br_if $b
				local.get $num
				local.get $out
				i64.mul
				local.set $out
				br $l
			end
		end
		local.get $out
	)
)
"""

# Last time
var prev_time: float = 0.0

# Last value
var prev_i: int = 0

# Label node
onready var label: Label = $Label

# WASM instance
var instance

func _ready():
	# Compile module
	var module = WasmHelper.load_wasm("test", WAT)

	# Instantiate module
	instance = module.instantiate(
		# No host bindings
		{},

		# Configuration
		{
			# Use epoch to limit execution time
			"engine.use_epoch": true,
		}
	)

	# Hook error signal
	if instance.connect(
		"error_happened",
		self,
		"_on_instance_error"
	) != OK:
		printerr("Cannot connect signal!")

# Print new factorial every second
func _process(delta: float):
	while delta > INTERVAL:
		delta -= INTERVAL
		prev_time += INTERVAL
		_process_fact()

	var temp: float = fmod(prev_time, INTERVAL)
	prev_time += delta
	if prev_time - temp > INTERVAL:
		_process_fact()

# Run single computation
func _process_fact():
	# Store temporary value
	var x = prev_i

	# Call WASM
	var v = instance.call_wasm("fact", [x])

	# Increment counter
	prev_i += 1

	# Calling WASM returns nil if it fails for any reason
	if v == null:
		return

	# Prints result
	label.text += "Factorial of {0} is {1}\n".format([
		x,
		v[0],
	])

# Handle WASM error
func _on_instance_error(msg: String):
	# Just display it
	label.text += "Factorial of {0} produces error!\n{1}\n".format([
		prev_i,
		msg,
	])
