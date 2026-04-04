class_name GameManager
extends Node2D

@export var player_scene: PackedScene
@export var base_spawn_interval: float = 2.0
@export var spawn_circle_radius: float = 500.0

var enemy_scenes: Array[PackedScene] = []
var spawn_timers: Array[float] = []
var spawn_intervals: Array[float] = []
var score: int = 0
var game_time: float = 0.0
var is_game_over: bool = false
var viewport_rect: Rect2

@onready var score_label: Label = $ScoreLabel
@onready var game_over_label: Label = $GameOverLabel


func _ready() -> void:
	viewport_rect = get_viewport().get_visible_rect()
	player_scene = load("res://scenes/player.tscn")
	enemy_scenes = [
		load("res://scenes/enemy.tscn"),
		load("res://scenes/shotgun_enemy.tscn"),
		load("res://scenes/turret_enemy.tscn")
	]
	
	for i in enemy_scenes.size():
		spawn_timers.append(randf_range(0.0, base_spawn_interval))
		spawn_intervals.append(base_spawn_interval * (1.0 + i * 0.5))
	
	_spawn_players()


func _process(delta: float) -> void:
	if is_game_over:
		return
	
	game_time += delta
	_update_spawn_intervals()
	_handle_spawning(delta)
	_update_ui()


func _spawn_players() -> void:
	var p1: CharacterBody2D = player_scene.instantiate()
	p1.position = Vector2(240, 300)
	p1.team_color = Color(0.35, 0.75, 1, 1)
	p1.add_to_group("players")
	add_child(p1)
	p1.died.connect(_on_player_died)
	
	var p2: CharacterBody2D = player_scene.instantiate()
	p2.position = Vector2(240, 470)
	p2.team_color = Color(1, 0.6, 0.2, 1)
	p2.move_left_action = &"p2_left"
	p2.move_right_action = &"p2_right"
	p2.move_up_action = &"p2_up"
	p2.move_down_action = &"p2_down"
	p2.use_right_stick = true
	p2.add_to_group("players")
	add_child(p2)
	p2.died.connect(_on_player_died)


func _update_spawn_intervals() -> void:
	var difficulty_factor: float = 1.0 - min(game_time / 300.0, 0.7)
	for i in spawn_intervals.size():
		spawn_intervals[i] = base_spawn_interval * (1.0 + i * 0.5) * difficulty_factor


func _handle_spawning(delta: float) -> void:
	for i in enemy_scenes.size():
		spawn_timers[i] += delta
		if spawn_timers[i] >= spawn_intervals[i]:
			spawn_timers[i] = 0.0
			_spawn_enemy(i)


func _spawn_enemy(index: int) -> void:
	var enemy: StaticBody2D = enemy_scenes[index].instantiate()
	var spawn_pos: Vector2
	
	if index == 2:
		spawn_pos = _get_spawn_inside_viewport()
	else:
		spawn_pos = _get_spawn_on_circle()
	
	enemy.global_position = spawn_pos
	add_child(enemy)
	enemy.died.connect(_on_enemy_died)


func _get_spawn_on_circle() -> Vector2:
	var center := viewport_rect.get_center()
	var angle := randf() * TAU
	return center + Vector2(cos(angle), sin(angle)) * spawn_circle_radius


func _get_spawn_inside_viewport() -> Vector2:
	var margin := 50.0
	return Vector2(
		randf_range(viewport_rect.position.x + margin, viewport_rect.end.x - margin),
		randf_range(viewport_rect.position.y + margin, viewport_rect.end.y - margin)
	)


func _on_enemy_died() -> void:
	score += 10


func _on_player_died() -> void:
	var players := get_tree().get_nodes_in_group("players")
	var alive_count := 0
	for p in players:
		if is_instance_valid(p):
			alive_count += 1
	
	if alive_count == 0:
		_game_over()


func _game_over() -> void:
	is_game_over = true
	game_over_label.visible = true


func _update_ui() -> void:
	score_label.text = "Score: %d" % score
