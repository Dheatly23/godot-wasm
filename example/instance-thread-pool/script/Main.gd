extends Control

const WAT = """
(module
	(func
		(export "calculate")
		(param $p i64)
		(result i64)
		(local $a i64)
		(local $b i64)
		local.get $p
		i64.eqz
		if
			i64.const 1
			return
		end
		i64.const 1
		local.tee $a
		local.set $b
		loop $l
			local.get $p
			i64.const 1
			i64.sub
			local.tee $p
			i64.eqz
			if
				local.get $b
				return
			end
			local.get $b
			local.get $a
			local.get $b
			i64.add
			local.set $b
			local.set $a
			br $l
		end
		unreachable
	)
)
"""

const N = 10

var instances: Array = []

onready var _log := $LogContainer

func _ready():
	var module = WasmHelper.load_wasm("calculator", WAT)

	for _i in range(N):
		var h: InstanceHandle = InstanceHandle.new()
		h.instantiate(
			module,
			{},
			{
				"engine.use_epoch": true,
			}
		)
		instances.append(h)

	for i in range(1, 101):
		var h: InstanceHandle = instances[i % len(instances)]
		# Change the scaling factor to make it sometimes trap
		i *= 50_000_000
		h.call_queue(
			"calculate",
			[i],
			self,
			"_on_success",
			self,
			"_on_error",
			[i],
			[i]
		)

func _on_success(ret: Array, i: int):
	_log.add_line("calculate(%d) = %d" % [i, ret[0]])

func _on_error(msg: String, i: int):
	_log.add_line("calculate(%d): %s" % [i, msg])
