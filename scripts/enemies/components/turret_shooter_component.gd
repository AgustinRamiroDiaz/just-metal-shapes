class_name TurretShooterComponent
extends Node

signal fired(projectile: Node)

@export var shoot_interval: float = 2.0

var _projectile_scene: PackedScene
var _timer: Timer
var _phase: int = 0


func _ready() -> void:
	_projectile_scene = load("res://scenes/projectile.tscn")
	_timer = Timer.new()
	_timer.wait_time = shoot_interval
	_timer.timeout.connect(_shoot)
	add_child(_timer)
	_timer.start(shoot_interval)


func _shoot() -> void:
	var origin := (get_parent() as Node2D).global_position
	if _phase == 0:
		_spawn_projectile(origin, Vector2.RIGHT)
		_spawn_projectile(origin, Vector2.LEFT)
		_spawn_projectile(origin, Vector2.UP)
		_spawn_projectile(origin, Vector2.DOWN)
	else:
		_spawn_projectile(origin, Vector2(1, 1).normalized())
		_spawn_projectile(origin, Vector2(1, -1).normalized())
		_spawn_projectile(origin, Vector2(-1, 1).normalized())
		_spawn_projectile(origin, Vector2(-1, -1).normalized())
	_phase = 1 - _phase


func _spawn_projectile(origin: Vector2, direction: Vector2) -> void:
	var proj: Area2D = _projectile_scene.instantiate()
	proj.global_position = origin
	proj.direction = direction
	get_tree().current_scene.add_child(proj)
	fired.emit(proj)
