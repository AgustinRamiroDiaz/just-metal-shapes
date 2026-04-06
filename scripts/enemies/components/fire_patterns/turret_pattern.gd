class_name TurretPattern
extends Node

var _phase: int = 0


func get_directions(_origin: Vector2) -> Array[Vector2]:
	var directions: Array[Vector2]
	if _phase == 0:
		directions = [Vector2.RIGHT, Vector2.LEFT, Vector2.UP, Vector2.DOWN]
	else:
		directions = [
			Vector2(1, 1).normalized(),
			Vector2(1, -1).normalized(),
			Vector2(-1, 1).normalized(),
			Vector2(-1, -1).normalized(),
		]
	_phase = 1 - _phase
	return directions
