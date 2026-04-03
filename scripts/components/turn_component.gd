class_name TurnComponent
extends Node

@export var turn_speed: float = 1.0
@export var target_group: StringName = &"players"


func _process(delta: float) -> void:
	var targets := get_tree().get_nodes_in_group(target_group)
	var origin := (get_parent() as Node2D).global_position
	var nearest: Node2D = _get_nearest_target(targets, origin)
	if nearest == null:
		return
	
	var sprite: Sprite2D = (get_parent() as Node2D).get_node("Sprite2D")
	var target_angle := (nearest.global_position - origin).angle()
	var current_angle := sprite.rotation
	var angle_diff := target_angle - current_angle
	while angle_diff > PI:
		angle_diff -= TAU
	while angle_diff < -PI:
		angle_diff += TAU
	sprite.rotation += sign(angle_diff) * min(abs(angle_diff), turn_speed * delta)


func _get_nearest_target(targets: Array, origin: Vector2) -> Node2D:
	var nearest: Node2D = null
	var min_dist: float = INF
	for t in targets:
		if not is_instance_valid(t) or not t is Node2D:
			continue
		var d := origin.distance_to(t.global_position)
		if d < min_dist:
			min_dist = d
			nearest = t as Node2D
	return nearest
