class_name SpawnerComponent
extends Node

signal spawned(node: Node)

@export var spawn_interval: float = 2.0
@export var enemy_scene: PackedScene
@export var spawn_area_width: float = 800.0
@export var spawn_y_offset: float = -50.0

var spawn_timer: float = 0.0


func _ready() -> void:
	if enemy_scene == null:
		enemy_scene = load("res://scenes/enemy.tscn")
	spawn_timer = randf_range(0.0, spawn_interval)


func _process(delta: float) -> void:
	spawn_timer += delta
	if spawn_timer >= spawn_interval:
		spawn_timer = 0.0
		_spawn()


func _spawn() -> void:
	var scene_to_spawn := enemy_scene
	if enemy_scene == null:
		scene_to_spawn = load("res://scenes/enemy.tscn")
	
	var enemy: StaticBody2D = scene_to_spawn.instantiate()
	var spawn_x := randf_range(0.0, spawn_area_width)
	enemy.global_position = Vector2(spawn_x, spawn_y_offset)
	get_tree().current_scene.add_child(enemy)
	spawned.emit(enemy)
