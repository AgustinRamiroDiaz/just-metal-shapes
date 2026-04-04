class_name ShotgunShooterComponent
extends ShooterComponent

@export var shot_count: int = 3
@export var spread_angle: float = 45.0


func _shoot() -> void:
	var origin := (get_parent() as Node2D).global_position
	var sprite: Sprite2D = (get_parent() as Node2D).get_node("Sprite2D")
	var base_direction := Vector2.RIGHT.rotated(sprite.rotation)
	var start_angle := -spread_angle / 2
	var angle_step := spread_angle / (shot_count - 1) if shot_count > 1 else 0.0

	for i in shot_count:
		var angle := deg_to_rad(start_angle + angle_step * i)
		var direction := base_direction.rotated(angle)
		_spawn_projectile(origin, direction)
