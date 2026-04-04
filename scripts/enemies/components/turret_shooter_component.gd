class_name TurretShooterComponent
extends ShooterComponent

var phase: int = 0


func _shoot() -> void:
	var origin := (get_parent() as Node2D).global_position

	if phase == 0:
		_spawn_projectile(origin, Vector2.RIGHT)
		_spawn_projectile(origin, Vector2.LEFT)
		_spawn_projectile(origin, Vector2.UP)
		_spawn_projectile(origin, Vector2.DOWN)
	else:
		_spawn_projectile(origin, Vector2(1, 1).normalized())
		_spawn_projectile(origin, Vector2(1, -1).normalized())
		_spawn_projectile(origin, Vector2(-1, 1).normalized())
		_spawn_projectile(origin, Vector2(-1, -1).normalized())

	phase = 1 - phase
