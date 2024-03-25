extends Node3D

signal message_emitted(msg)

@export var wasm_file: WasmModule

@onready var wasi_ctx: WasiContext = WasiContext.new()

@onready var _mesh := ArrayMesh.new()

var instance: WasmInstance = null

func __selected(index):
	instance = WasmInstance.new()
	instance.error_happened.connect(__emit_log)
	instance = instance.initialize(
		wasm_file,
		{},
		{
			"epoch.enable": true,
			"epoch.timeout": 1.0,
			"wasi.enable": true,
			"wasi.context": wasi_ctx,
		}
	)
	if instance == null:
		message_emitted.emit("Failed to instantiate module")
		return

	if instance.call_wasm("init", [index]) == null:
		message_emitted.emit("Failed to call init")

func _ready():
	$Mesh.mesh = _mesh

	$UI/Root/TypeLst.select(0)

#	var data := []
#	data.resize(Mesh.ARRAY_MAX)
#	data[Mesh.ARRAY_VERTEX] = PoolVector3Array([
#		Vector3(-1, -1, 0),
#		Vector3(-1, 1, 0),
#		Vector3(1, -1, 0),
#		Vector3(1, 1, 0),
#	])
#	data[Mesh.ARRAY_TEX_UV] = PoolVector3Array([
#		Vector3(0, 1, 0),
#		Vector3(0, 0, 0),
#		Vector3(1, 1, 0),
#		Vector3(1, 0, 0),
#	])
#	data[Mesh.ARRAY_NORMAL] = PoolVector3Array([
#		Vector3(0, 0, 1),
#		Vector3(0, 0, 1),
#		Vector3(0, 0, 1),
#		Vector3(0, 0, 1),
#	])
#	data[Mesh.ARRAY_TANGENT] = PoolRealArray([
#		1, 0, 0, 1,
#		1, 0, 0, 1,
#		1, 0, 0, 1,
#		1, 0, 0, 1,
#	])
#	data[Mesh.ARRAY_INDEX] = PoolIntArray([
#		0, 3, 2,
#		0, 1, 3,
#	])
#	_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, data)
#	return

	wasi_ctx.stdout_emit.connect(__emit_log)
	wasi_ctx.stderr_emit.connect(__emit_log)

	__selected(0)

func _process(delta):
	if instance == null:
		return

	var start := Time.get_ticks_usec()
	var ret = instance.call_wasm("process", [delta])
	if ret == null:
		message_emitted.emit("Failed to call process")
		instance = null
		return
	var end := Time.get_ticks_usec()
	__emit_log("WASM Time: %.3f ms" % ((end - start) / 1e3))

	var p: int = ret[0]
	if p == 0:
		return
	var data := []
	data.resize(Mesh.ARRAY_MAX)
	data[Mesh.ARRAY_VERTEX] = instance.get_array(
		instance.get_32(p),
		instance.get_32(p + 4),
		TYPE_PACKED_VECTOR3_ARRAY
	)
	data[Mesh.ARRAY_NORMAL] = instance.get_array(
		instance.get_32(p + 8),
		instance.get_32(p + 12),
		TYPE_PACKED_VECTOR3_ARRAY
	)
	data[Mesh.ARRAY_TANGENT] = instance.get_array(
		instance.get_32(p + 16),
		instance.get_32(p + 20) * 4,
		TYPE_PACKED_FLOAT32_ARRAY
	)
	data[Mesh.ARRAY_TEX_UV] = instance.get_array(
		instance.get_32(p + 24),
		instance.get_32(p + 28),
		TYPE_PACKED_VECTOR2_ARRAY
	)
	data[Mesh.ARRAY_COLOR] = instance.get_array(
		instance.get_32(p + 32),
		instance.get_32(p + 36),
		TYPE_PACKED_COLOR_ARRAY
	)
	data[Mesh.ARRAY_INDEX] = instance.get_array(
		instance.get_32(p + 40),
		instance.get_32(p + 44),
		TYPE_PACKED_INT32_ARRAY
	)

	_mesh.clear_surfaces()
	if len(data[Mesh.ARRAY_INDEX]) != 0:
		_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, data)

func __emit_log(msg):
	message_emitted.emit(msg.strip_edges())
