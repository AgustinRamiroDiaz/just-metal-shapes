class_name ShooterComponent
extends Node

signal fired(projectile: Node)

@export var shoot_interval: float = 2.0
@export var target_group: StringName = &"players"

var shoot_timer: float = 0.0
var _projectile_scene: PackedScene


func _ready() -> void:
	_projectile_scene = load("res://scenes/projectile.tscn")
	shoot_timer = randf_range(0.0, shoot_interval)


func _process(delta: float) -> void:
	if _projectile_scene == null:
		return
	shoot_timer += delta
	if shoot_timer >= shoot_interval:
		shoot_timer = 0.0
		_shoot()


func _shoot() -> void:
	var targets := get_tree().get_nodes_in_group(target_group)
	var origin := (get_parent() as Node2D).global_position
	var nearest: Node2D = null
	var min_dist: float = INF
	for t in targets:
		if not is_instance_valid(t) or not t is Node2D:
			continue
		var d := origin.distance_to(t.global_position)
		if d < min_dist:
			min_dist = d
			nearest = t as Node2D
	if nearest == null:
		return
	var proj: Area2D = _projectile_scene.instantiate()
	proj.global_position = origin
	proj.direction = (nearest.global_position - origin).normalized()
	get_tree().current_scene.add_child(proj)
	fired.emit(proj)
