class_name MineDropperComponent
extends Node

@export var drop_interval: float = 2.5

var _mine_scene: PackedScene = preload("res://scenes/mine.tscn")
var _timer: float = 0.0


func _process(delta: float) -> void:
	_timer += delta
	if _timer >= drop_interval:
		_timer -= drop_interval
		_drop_mine()


func _drop_mine() -> void:
	var parent := get_parent() as Node2D
	var mine: Area2D = _mine_scene.instantiate()
	mine.global_position = parent.global_position
	parent.get_parent().add_child(mine)
