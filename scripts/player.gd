class_name Player
extends CharacterBody2D

signal died

const MAX_LIVES: int = 3
const INVINCIBILITY_DURATION: float = 3.0
const LIGHTNING_TEXTURES: Array[Texture2D] = [
	preload("res://assets/kenney-particles/Rotated/spark_05_rotated.png"),
	preload("res://assets/kenney-particles/Rotated/spark_06_rotated.png"),
]
const LIGHTNING_FPS: float = 12.0
const LIGHTNING_WIDTH: float = 80.0
const TIER_COUNT: int = 3

@export var speed: float = 220.0
@export var range_radius: float = 140.0:
	set = set_range_radius
@export var damage_per_second: float = 1.0
@export var team_color: Color = Color(0.35, 0.75, 1.0, 1.0)
@export var move_left_action: StringName = &"ui_left"
@export var move_right_action: StringName = &"ui_right"
@export var move_up_action: StringName = &"ui_up"
@export var move_down_action: StringName = &"ui_down"
@export var input_type: int = 0
@export var joystick_deadzone: float = 0.2

var targets_in_range: Array[Node2D] = []
var lives: int = MAX_LIVES
var invincible_timer: float = 0.0
var is_dead: bool = false
var revival_progress: float = 0.0
var _lightning_lines: Dictionary = {}  # target_id -> Array[Line2D]
var _lightning_frame_timer: float = 0.0
var _lightning_frame_index: int = 0

@onready var range_area: Area2D = $RangeArea
@onready var range_shape: CollisionShape2D = $RangeArea/CollisionShape2D


func _apply_deadzone(value: float) -> float:
	if absf(value) < joystick_deadzone:
		return 0.0
	return signf(value) * (absf(value) - joystick_deadzone) / (1.0 - joystick_deadzone)


func _ready() -> void:
	add_to_group("players")
	_update_range_shape()
	queue_redraw()
	range_area.body_entered.connect(_on_range_body_entered)
	range_area.body_exited.connect(_on_range_body_exited)


func _physics_process(delta: float) -> void:
	if is_dead:
		return

	if invincible_timer > 0.0:
		invincible_timer -= delta
		modulate.a = 0.3 if fmod(invincible_timer * 6.0, 1.0) > 0.5 else 1.0
		if invincible_timer <= 0.0:
			modulate.a = 1.0

	var input_dir: Vector2
	if input_type >= GameConfig.InputType.GAMEPAD_RIGHT_0:
		var dev := input_type - GameConfig.InputType.GAMEPAD_RIGHT_0
		input_dir = Vector2(
			_apply_deadzone(Input.get_joy_axis(dev, JOY_AXIS_RIGHT_X)),
			_apply_deadzone(Input.get_joy_axis(dev, JOY_AXIS_RIGHT_Y))
		)
	elif input_type >= GameConfig.InputType.GAMEPAD_LEFT_0:
		var dev := input_type - GameConfig.InputType.GAMEPAD_LEFT_0
		input_dir = Vector2(
			_apply_deadzone(Input.get_joy_axis(dev, JOY_AXIS_LEFT_X)),
			_apply_deadzone(Input.get_joy_axis(dev, JOY_AXIS_LEFT_Y))
		)
	else:
		input_dir = Input.get_vector(
			move_left_action, move_right_action, move_up_action, move_down_action
		)
	velocity = input_dir * speed
	move_and_slide()
	_clamp_to_viewport()
	_apply_continuous_damage(delta)


func _clamp_to_viewport() -> void:
	var rect := get_viewport().get_visible_rect()
	global_position.x = clampf(global_position.x, rect.position.x, rect.end.x)
	global_position.y = clampf(global_position.y, rect.position.y, rect.end.y)


func _get_tier_radius(tier: int) -> float:
	return range_radius * (float(tier + 1) / TIER_COUNT)


func _get_ray_count(target: Node2D) -> int:
	var dist := global_position.distance_to(target.global_position)
	for tier in range(TIER_COUNT):
		if dist <= _get_tier_radius(tier):
			return TIER_COUNT - tier
	return 1


func _draw() -> void:
	if not is_dead:
		for tier in range(TIER_COUNT):
			var r := _get_tier_radius(tier)
			var alpha := 0.12 * (TIER_COUNT - tier) / TIER_COUNT
			var tier_fill := Color(team_color.r, team_color.g, team_color.b, alpha)
			if tier == 0:
				draw_circle(Vector2.ZERO, r, tier_fill)
			else:
				# Draw filled ring between previous tier and this one
				var prev_r := _get_tier_radius(tier - 1)
				draw_arc(Vector2.ZERO, (prev_r + r) / 2.0, 0.0, TAU, 64, tier_fill, r - prev_r)
	if is_dead and revival_progress > 0.0:
		draw_arc(
			Vector2.ZERO, 18.0, -PI / 2.0, -PI / 2.0 + TAU * revival_progress, 32, Color.WHITE, 3.0
		)


func set_range_radius(value: float) -> void:
	range_radius = max(value, 8.0)
	if is_inside_tree():
		_update_range_shape()
		queue_redraw()


func _update_range_shape() -> void:
	var circle_shape := range_shape.shape as CircleShape2D
	if circle_shape == null:
		circle_shape = CircleShape2D.new()
		range_shape.shape = circle_shape
	circle_shape.radius = range_radius


func _on_range_body_entered(body: Node) -> void:
	if body is Node2D and body.has_method("take_damage"):
		targets_in_range.append(body)


func _on_range_body_exited(body: Node) -> void:
	if body is Node2D:
		targets_in_range.erase(body)
		var target_id := body.get_instance_id()
		if target_id in _lightning_lines:
			for line: Line2D in _lightning_lines[target_id]:
				line.queue_free()
			_lightning_lines.erase(target_id)


func take_damage(amount: int) -> void:
	if invincible_timer > 0.0 or is_dead:
		return
	lives -= amount
	invincible_timer = INVINCIBILITY_DURATION
	if lives <= 0:
		lives = 0
		is_dead = true
		_clear_lightning()
		modulate = Color(0.4, 0.4, 0.4, 1.0)
		queue_redraw()
		died.emit()


func revive() -> void:
	lives = 1
	is_dead = false
	revival_progress = 0.0
	invincible_timer = INVINCIBILITY_DURATION
	modulate = Color.WHITE
	queue_redraw()


func _apply_continuous_damage(delta: float) -> void:
	if damage_per_second <= 0.0:
		_clear_lightning()
		return

	# Cycle lightning animation frame
	_lightning_frame_timer += delta
	if _lightning_frame_timer >= 1.0 / LIGHTNING_FPS:
		_lightning_frame_timer -= 1.0 / LIGHTNING_FPS
		_lightning_frame_index = (_lightning_frame_index + 1) % LIGHTNING_TEXTURES.size()
		for lines: Array in _lightning_lines.values():
			for line: Line2D in lines:
				line.texture = LIGHTNING_TEXTURES[_lightning_frame_index]

	var active_targets: Array[Node2D] = []
	for target in targets_in_range:
		if is_instance_valid(target) and target.has_method("take_damage"):
			var ray_count := _get_ray_count(target)
			var damage_amount := damage_per_second * ray_count * delta
			var did_damage: bool = target.take_damage(damage_amount, team_color)
			if did_damage:
				active_targets.append(target)
				_update_lightning(target, ray_count)

	# Remove lightning for targets no longer being damaged
	for target_id in _lightning_lines.keys():
		var found := false
		for t in active_targets:
			if t.get_instance_id() == target_id:
				found = true
				break
		if not found:
			for line: Line2D in _lightning_lines[target_id]:
				line.queue_free()
			_lightning_lines.erase(target_id)


func _update_lightning(target: Node2D, ray_count: int) -> void:
	var target_id := target.get_instance_id()
	var lines: Array = _lightning_lines.get(target_id, []) as Array

	# Add or remove lines to match ray_count
	while lines.size() < ray_count:
		var line := Line2D.new()
		line.texture = LIGHTNING_TEXTURES[_lightning_frame_index]
		line.texture_mode = Line2D.LINE_TEXTURE_TILE
		line.width = LIGHTNING_WIDTH
		line.default_color = Color(team_color, 0.8)
		line.z_index = -1
		add_child(line)
		lines.append(line)
	while lines.size() > ray_count:
		var extra: Line2D = lines.pop_back()
		extra.queue_free()

	_lightning_lines[target_id] = lines

	var target_local := to_local(target.global_position)
	var perp := target_local.normalized().rotated(PI / 2.0)
	for i in range(ray_count):
		var offset := perp * (i - (ray_count - 1) / 2.0) * 10.0
		lines[i].clear_points()
		lines[i].add_point(target_local + offset)
		lines[i].add_point(Vector2.ZERO + offset)


func _clear_lightning() -> void:
	for lines: Array in _lightning_lines.values():
		for line: Line2D in lines:
			line.queue_free()
	_lightning_lines.clear()
