class_name Player
extends CharacterBody2D

signal died

const MAX_LIVES: int = 3
const INVINCIBILITY_DURATION: float = 3.0
const LIGHTNING_TEXTURES: Array[Texture2D] = [
	preload("res://assets/PNG (Transparent)/Rotated/spark_05_rotated.png"),
	preload("res://assets/PNG (Transparent)/Rotated/spark_06_rotated.png"),
]
const LIGHTNING_FPS: float = 12.0
const LIGHTNING_WIDTH: float = 80.0

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
var _lightning_lines: Dictionary = {}
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


func _draw() -> void:
	var ring_color := Color(team_color.r, team_color.g, team_color.b, 0.9)
	var fill_color := Color(team_color.r, team_color.g, team_color.b, 0.08)
	if not is_dead:
		draw_arc(Vector2.ZERO, range_radius, 0.0, TAU, 64, ring_color, 2.5)
		draw_circle(Vector2.ZERO, range_radius, fill_color)
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
			_lightning_lines[target_id].queue_free()
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
		for line: Line2D in _lightning_lines.values():
			line.texture = LIGHTNING_TEXTURES[_lightning_frame_index]

	var active_targets: Array[Node2D] = []
	var damage_amount := damage_per_second * delta
	for target in targets_in_range:
		if is_instance_valid(target) and target.has_method("take_damage"):
			var did_damage: bool = target.take_damage(damage_amount, team_color)
			if did_damage:
				active_targets.append(target)
				_update_lightning(target)

	# Remove lightning for targets no longer being damaged
	for target_id in _lightning_lines.keys():
		var found := false
		for t in active_targets:
			if t.get_instance_id() == target_id:
				found = true
				break
		if not found:
			_lightning_lines[target_id].queue_free()
			_lightning_lines.erase(target_id)


func _update_lightning(target: Node2D) -> void:
	var target_id := target.get_instance_id()
	var line: Line2D
	if target_id in _lightning_lines:
		line = _lightning_lines[target_id]
	else:
		line = Line2D.new()
		line.texture = LIGHTNING_TEXTURES[_lightning_frame_index]
		line.texture_mode = Line2D.LINE_TEXTURE_TILE
		line.width = LIGHTNING_WIDTH
		line.default_color = Color(team_color, 0.8)
		line.z_index = -1
		add_child(line)
		_lightning_lines[target_id] = line
	line.clear_points()
	line.add_point(to_local(target.global_position))
	line.add_point(Vector2.ZERO)


func _clear_lightning() -> void:
	for line: Line2D in _lightning_lines.values():
		line.queue_free()
	_lightning_lines.clear()
