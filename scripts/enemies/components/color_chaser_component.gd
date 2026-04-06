class_name ColorChaserComponent
extends Node

@export var move_speed: float = 50.0
@export var shield_color: Color = Color.WHITE
@export var target_group: StringName = &"players"


func _ready() -> void:
	var health := get_parent().get_node_or_null("HealthComponent") as HealthComponent
	if health:
		shield_color = health.shield_color


func _process(delta: float) -> void:
	var parent := get_parent() as Node2D
	var nearest := _get_nearest_mismatched_target(parent.global_position)
	if nearest == null:
		return
	var direction := (nearest.global_position - parent.global_position).normalized()
	parent.global_position += direction * move_speed * delta


func _get_nearest_mismatched_target(origin: Vector2) -> Node2D:
	var nearest: Node2D = null
	var min_dist: float = INF
	for t in get_tree().get_nodes_in_group(target_group):
		if not is_instance_valid(t) or not t is Node2D:
			continue
		if t.get("is_dead") == true:
			continue
		if ColorUtils.colors_match(Color(t.get("team_color")), shield_color):
			continue
		var d := origin.distance_to(t.global_position)
		if d < min_dist:
			min_dist = d
			nearest = t as Node2D
	return nearest
