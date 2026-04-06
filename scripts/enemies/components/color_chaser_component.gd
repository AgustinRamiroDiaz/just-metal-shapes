class_name ColorChaserComponent
extends Node

@export var move_speed: float = 50.0
@export var target_group: StringName = &"players"


func _process(delta: float) -> void:
	var parent := get_parent() as Node2D
	var health: HealthComponent = parent.get_node("HealthComponent")
	var targets := get_tree().get_nodes_in_group(target_group)
	var nearest := _get_nearest_mismatched_target(
		targets, parent.global_position, health.shield_color
	)
	if nearest == null:
		return
	var direction := (nearest.global_position - parent.global_position).normalized()
	parent.global_position += direction * move_speed * delta


func _get_nearest_mismatched_target(targets: Array, origin: Vector2, shield_color: Color) -> Node2D:
	var nearest: Node2D = null
	var min_dist: float = INF
	for t in targets:
		if not is_instance_valid(t) or not t is Node2D:
			continue
		if t.get("is_dead") == true:
			continue
		if _colors_match(t.get("team_color"), shield_color):
			continue
		var d := origin.distance_to(t.global_position)
		if d < min_dist:
			min_dist = d
			nearest = t as Node2D
	return nearest


func _colors_match(c1: Color, c2: Color) -> bool:
	return c1.r == c2.r and c1.g == c2.g and c1.b == c2.b
