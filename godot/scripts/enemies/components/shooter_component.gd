class_name ShooterComponent
extends Node

signal fired(projectile: Node)

@export var shoot_interval: float = 2.0
@export var target_group: StringName = &"players"

var _projectile_scene: PackedScene
var _timer: Timer


func _ready() -> void:
	_projectile_scene = load("res://scenes/projectile.tscn")
	_timer = Timer.new()
	_timer.wait_time = shoot_interval
	_timer.timeout.connect(_shoot)
	add_child(_timer)
	_timer.start(shoot_interval)


func _shoot() -> void:
	var origin := (get_parent() as Node2D).global_position
	var nearest := Targeting.get_nearest_alive(get_tree(), origin, target_group)
	if nearest == null:
		return
	_spawn_projectile(origin, (nearest.global_position - origin).normalized())


func _spawn_projectile(origin: Vector2, direction: Vector2) -> void:
	var proj: Area2D = _projectile_scene.instantiate()
	proj.global_position = origin
	proj.direction = direction
	get_tree().current_scene.add_child(proj)
	fired.emit(proj)
