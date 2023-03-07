# Plugin to manage WASM instance invocation through thread pool

@tool
extends EditorPlugin

func _enter_tree():
	add_autoload_singleton("InstanceThreadpoolAutoload", "res://addons/instance_threadpool/InstanceThreadpoolAutoload.gd")

func _exit_tree():
	remove_autoload_singleton("InstanceThreadpoolAutoload")
