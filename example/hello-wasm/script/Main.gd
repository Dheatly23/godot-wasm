extends Control

# The WAT module to be loaded
const WAT = """
(module
	(func $write_char
		(import "host" "write_char")
		(param i32)
	)
	(func $run
		(export "run")
		(local $i i32)
		(local $c i32)
		loop $l
			local.get $i
			i32.load8_u
			local.tee $c
			i32.eqz
			if
				return
			end
			local.get $c
			call $write_char
			local.get $i
			i32.const 1
			i32.add
			local.set $i
			br $l
		end
	)
	(memory (export "memory") 1 1)
	(data (memory 0) (i32.const 0)
		"Hello from WASM!\\00"
	)
)
"""

# Label node to display the text
onready var label: Label = $CenterContainer/Label

# Instance variable
var instance

func _ready():
	# Load module
	# Returns nil if compilation fails
	var module = WasmHelper.load_wasm("test", WAT)
	assert(not (module == null))

	# Instantiate module with host bindings
	# Returns nil if instantiation fails
	# (eg. an import missing)
	instance = module.instantiate({
		"write_char": {
			params = [WasmHelper.TYPE_I32],
			results = [],
			object = self,
			method = "_write_char",
		},
	})
	assert(not (instance == null))

	# Call WebAssembly functions
	instance.call_wasm("run", [])

func _write_char(cc: int):
	label.text += char(cc)
