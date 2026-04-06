class_name ShooterComponent
extends Node

signal fired(projectile: Node)

@export var shoot_interval: float = 2.0

var _projectile_scene: PackedScene
var _timer: Timer
var _pattern: Node


func _ready() -> void:
	_projectile_scene = load("res://scenes/projectile.tscn")
	_pattern = _find_pattern()
	_timer = Timer.new()
	_timer.wait_time = shoot_interval
	_timer.timeout.connect(_shoot)
	add_child(_timer)
	_timer.start(shoot_interval)


func _shoot() -> void:
	var origin := (get_parent() as Node2D).global_position
	var directions: Array[Vector2] = _pattern.get_directions(origin)
	for dir: Vector2 in directions:
		_spawn_projectile(origin, dir)


func _spawn_projectile(origin: Vector2, direction: Vector2) -> void:
	var proj: Area2D = _projectile_scene.instantiate()
	proj.global_position = origin
	proj.direction = direction
	get_tree().current_scene.add_child(proj)
	fired.emit(proj)


func _find_pattern() -> Node:
	for child in get_children():
		if child.has_method("get_directions"):
			return child
	# Default: aimed at nearest player
	var aimed: AimedPattern = AimedPattern.new()
	add_child(aimed)
	return aimed
