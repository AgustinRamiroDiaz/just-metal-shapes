class_name ChaserComponent
extends Node

signal moved(amount: float)

@export var move_speed: float = 30.0
@export var target_group: StringName = &"players"


func _process(delta: float) -> void:
	var targets := get_tree().get_nodes_in_group(target_group)
	var origin := (get_parent() as Node2D).global_position
	var nearest: Node2D = _get_nearest_target(targets, origin)
	if nearest == null:
		return
	var direction := (nearest.global_position - origin).normalized()
	origin += direction * move_speed * delta
	(get_parent() as Node2D).global_position = origin
	moved.emit(move_speed * delta)


func _get_nearest_target(targets: Array, origin: Vector2) -> Node2D:
	var nearest: Node2D = null
	var min_dist: float = INF
	for t in targets:
		if not is_instance_valid(t) or not t is Node2D:
			continue
		if t.get("is_dead") == true:
			continue
		var d := origin.distance_to(t.global_position)
		if d < min_dist:
			min_dist = d
			nearest = t as Node2D
	return nearest
