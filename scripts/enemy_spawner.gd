class_name EnemySpawner
extends Node

signal enemy_died

enum SpawnType { INSIDE, OUTSIDE }


class EnemyConfig:
	var scene: PackedScene
	var spawn_type: SpawnType
	var interval_multiplier: float
	var timer: float
	var spawn_interval: float

	func _init(p_scene: PackedScene, p_spawn_type: SpawnType, p_interval_multiplier: float) -> void:
		scene = p_scene
		spawn_type = p_spawn_type
		interval_multiplier = p_interval_multiplier


@export var base_spawn_interval: float = 7.0

var enemy_configs: Array[EnemyConfig] = []
var _spawn_effect_scene: PackedScene = preload("res://scenes/spawn_effect.tscn")
var _viewport_rect: Rect2
var _active: bool = true


func _ready() -> void:
	_viewport_rect = get_viewport().get_visible_rect()
	enemy_configs = [
		EnemyConfig.new(load("res://scenes/static_shooter_enemy.tscn"), SpawnType.INSIDE, 1.0),
		EnemyConfig.new(load("res://scenes/shotgun_enemy.tscn"), SpawnType.OUTSIDE, 1.5),
		EnemyConfig.new(load("res://scenes/turret_enemy.tscn"), SpawnType.INSIDE, 2.0),
		EnemyConfig.new(load("res://scenes/runner_enemy.tscn"), SpawnType.OUTSIDE, 1.2),
		EnemyConfig.new(load("res://scenes/mine_layer_enemy.tscn"), SpawnType.OUTSIDE, 2.5),
	]
	for cfg in enemy_configs:
		cfg.timer = randf_range(0.0, base_spawn_interval)
		cfg.spawn_interval = base_spawn_interval * cfg.interval_multiplier


func stop() -> void:
	_active = false


func update_difficulty(game_time: float) -> void:
	var difficulty_factor: float = 1.0 - min(game_time / 300.0, 0.7)
	for cfg in enemy_configs:
		cfg.spawn_interval = base_spawn_interval * cfg.interval_multiplier * difficulty_factor


func _process(delta: float) -> void:
	if not _active:
		return
	for cfg in enemy_configs:
		cfg.timer += delta
		if cfg.timer >= cfg.spawn_interval:
			cfg.timer = 0.0
			_spawn_enemy(cfg)


func _spawn_enemy(cfg: EnemyConfig) -> void:
	var spawn_pos := (
		_get_spawn_inside_viewport()
		if cfg.spawn_type == SpawnType.INSIDE
		else _get_spawn_on_circle()
	)

	if cfg.spawn_type == SpawnType.INSIDE:
		var effect: GPUParticles2D = _spawn_effect_scene.instantiate()
		effect.global_position = spawn_pos
		get_parent().add_child(effect)
		effect.spawn_ready.connect(_place_enemy.bind(cfg, spawn_pos))
	else:
		_place_enemy(cfg, spawn_pos)


func _place_enemy(cfg: EnemyConfig, spawn_pos: Vector2) -> void:
	if not _active:
		return
	var enemy: BaseEnemy = cfg.scene.instantiate()
	enemy.global_position = spawn_pos
	get_parent().add_child(enemy)
	var player_count := GameConfig.players.size()
	if player_count > 1:
		enemy.health.max_life *= player_count
		enemy.health.life = enemy.health.max_life
	enemy.died.connect(func() -> void: enemy_died.emit())


func _get_spawn_on_circle() -> Vector2:
	var center := _viewport_rect.get_center()
	var angle := randf() * TAU
	var radius := _viewport_rect.size.length() / 2.0 + 50.0
	return center + Vector2(cos(angle), sin(angle)) * radius


func _get_spawn_inside_viewport() -> Vector2:
	var margin := 50.0
	return Vector2(
		randf_range(_viewport_rect.position.x + margin, _viewport_rect.end.x - margin),
		randf_range(_viewport_rect.position.y + margin, _viewport_rect.end.y - margin)
	)
