class_name ShotgunPattern
extends Node

@export var shot_count: int = 3
@export var spread_angle: float = 45.0
@export var sprite_path: NodePath = "Sprite2D"

@onready var _sprite: Sprite2D = get_parent().get_parent().get_node(sprite_path)


func get_directions(_origin: Vector2) -> Array[Vector2]:
	var base_direction := Vector2.RIGHT.rotated(_sprite.rotation)
	var start_angle := -spread_angle / 2.0
	var angle_step := spread_angle / (shot_count - 1) if shot_count > 1 else 0.0
	var directions: Array[Vector2] = []
	for i in shot_count:
		var angle := deg_to_rad(start_angle + angle_step * i)
		directions.append(base_direction.rotated(angle))
	return directions
