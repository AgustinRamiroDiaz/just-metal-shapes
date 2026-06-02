class_name MineDropperComponent
extends Node

@export var drop_interval: float = 2.5

var _mine_scene: PackedScene = preload("res://scenes/mine.tscn")


func _ready() -> void:
	var timer := Timer.new()
	timer.wait_time = drop_interval
	timer.timeout.connect(_drop_mine)
	add_child(timer)
	timer.start()


func _drop_mine() -> void:
	var parent := get_parent() as Node2D
	var mine: Area2D = _mine_scene.instantiate()
	mine.global_position = parent.global_position
	parent.get_parent().add_child(mine)
