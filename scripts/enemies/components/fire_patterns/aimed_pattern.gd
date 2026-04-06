class_name AimedPattern
extends Node

@export var target_group: StringName = &"players"


func get_directions(origin: Vector2) -> Array[Vector2]:
	var nearest := Targeting.get_nearest_alive(get_tree(), origin, target_group)
	if nearest == null:
		return []
	return [(nearest.global_position - origin).normalized()]
