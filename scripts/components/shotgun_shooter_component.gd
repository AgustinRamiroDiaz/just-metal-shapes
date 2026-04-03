class_name ShotgunShooterComponent
extends ShooterComponent

@export var shot_count: int = 8
@export var spread_angle: float = 45.0


func _shoot() -> void:
	var targets := get_tree().get_nodes_in_group(target_group)
	var origin := (get_parent() as Node2D).global_position
	var nearest := _get_nearest_target(targets, origin)
	if nearest == null:
		return
	
	var base_direction := (nearest.global_position - origin).normalized()
	var start_angle := -spread_angle / 2
	var angle_step := spread_angle / (shot_count - 1) if shot_count > 1 else 0.0
	
	for i in shot_count:
		var angle := deg_to_rad(start_angle + angle_step * i)
		var direction := base_direction.rotated(angle)
		_spawn_projectile(origin, direction)
