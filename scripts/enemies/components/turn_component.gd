class_name TurnComponent
extends Node

@export var turn_speed: float = 0.5
@export var target_group: StringName = &"players"


func _process(delta: float) -> void:
	var targets := get_tree().get_nodes_in_group(target_group)
	var origin := (get_parent() as Node2D).global_position
	var nearest: Node2D = _get_nearest_target(targets, origin)
	if nearest == null:
		return

	var sprite: Sprite2D = (get_parent() as Node2D).get_node("Sprite2D")
	var target_angle := (nearest.global_position - origin).angle()
	sprite.rotation = lerp_angle(sprite.rotation, target_angle, turn_speed * delta)


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
