class_name ChaserComponent
extends Node

signal moved(amount: float)

@export var move_speed: float = 30.0
@export var target_group: StringName = &"players"


func _process(delta: float) -> void:
	var parent := get_parent() as Node2D
	var nearest := Targeting.get_nearest_alive(get_tree(), parent.global_position, target_group)
	if nearest == null:
		return
	var direction := (nearest.global_position - parent.global_position).normalized()
	parent.global_position += direction * move_speed * delta
	moved.emit(move_speed * delta)
