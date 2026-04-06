class_name ShotgunShooterComponent
extends Node

signal fired(projectile: Node)

@export var shoot_interval: float = 3.0
@export var shot_count: int = 3
@export var spread_angle: float = 45.0
@export var sprite_path: NodePath = "Sprite2D"

var _projectile_scene: PackedScene
var _timer: Timer

@onready var _sprite: Sprite2D = get_parent().get_node(sprite_path)


func _ready() -> void:
	_projectile_scene = load("res://scenes/projectile.tscn")
	_timer = Timer.new()
	_timer.wait_time = shoot_interval
	_timer.timeout.connect(_shoot)
	add_child(_timer)
	_timer.start(shoot_interval)


func _shoot() -> void:
	var origin := (get_parent() as Node2D).global_position
	var base_direction := Vector2.RIGHT.rotated(_sprite.rotation)
	var start_angle := -spread_angle / 2.0
	var angle_step := spread_angle / (shot_count - 1) if shot_count > 1 else 0.0

	for i in shot_count:
		var angle := deg_to_rad(start_angle + angle_step * i)
		_spawn_projectile(origin, base_direction.rotated(angle))


func _spawn_projectile(origin: Vector2, direction: Vector2) -> void:
	var proj: Area2D = _projectile_scene.instantiate()
	proj.global_position = origin
	proj.direction = direction
	get_tree().current_scene.add_child(proj)
	fired.emit(proj)
