class_name TurnComponent
extends Node

@export var turn_speed: float = 0.5
@export var target_group: StringName = &"players"
@export var sprite_path: NodePath = "Sprite2D"

@onready var _sprite: Sprite2D = get_parent().get_node(sprite_path)


func _process(delta: float) -> void:
	var origin := (get_parent() as Node2D).global_position
	var nearest := Targeting.get_nearest_alive(get_tree(), origin, target_group)
	if nearest == null:
		return
	var target_angle := (nearest.global_position - origin).angle()
	_sprite.rotation = lerp_angle(_sprite.rotation, target_angle, turn_speed * delta)
