class_name Player
extends CharacterBody2D

signal died
signal state_changed(from: int, to: int)

enum State { IDLE, ATTACKING, DEAD }

const MAX_LIVES: int = 3
const INVINCIBILITY_DURATION: float = 3.0
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

var _sm: StateMachine
var _face_sprite: Sprite2D
var _face_tex_idle: Texture2D
var _face_tex_attacking: Texture2D
var _face_tex_dead: Texture2D

@onready var range_area: Area2D = $RangeArea
@onready var range_shape: CollisionShape2D = $RangeArea/CollisionShape2D
@onready var _lightning: LightningComponent = $LightningComponent
@onready var _body_sprite: Sprite2D = $Sprite2D


func _apply_deadzone(value: float) -> float:
	if absf(value) < joystick_deadzone:
		return 0.0
	return signf(value) * (absf(value) - joystick_deadzone) / (1.0 - joystick_deadzone)


func _ready() -> void:
	_sm = (
		StateMachine
		. new(
			State.IDLE,
			{
				State.IDLE: [State.ATTACKING, State.DEAD],
				State.ATTACKING: [State.IDLE, State.DEAD],
				State.DEAD: [],
			}
		)
	)
	_sm.state_changed.connect(_on_state_changed)
	_sm.state_changed.connect(func(from, to): state_changed.emit(from, to))

	add_to_group("players")
	_update_range_shape()
	_load_sprites()
	queue_redraw()
	range_area.body_entered.connect(_on_range_body_entered)
	range_area.body_exited.connect(_on_range_body_exited)


func _load_sprites() -> void:
	var shader_mat := ShaderMaterial.new()
	shader_mat.shader = load("res://shaders/player_color.gdshader")
	shader_mat.set_shader_parameter("player_color", team_color)

	_body_sprite.texture = load(GameConfig.PLAYER_BODY_TEXTURE)
	_body_sprite.scale = Vector2(0.2, 0.2)
	_body_sprite.material = shader_mat

	_face_tex_idle = load(GameConfig.PLAYER_FACE_IDLE)
	_face_tex_attacking = load(GameConfig.PLAYER_FACE_ATTACKING)
	_face_tex_dead = load(GameConfig.PLAYER_FACE_DEAD)

	_face_sprite = Sprite2D.new()
	_face_sprite.texture = _face_tex_idle
	_face_sprite.scale = Vector2(0.2, 0.2)
	add_child(_face_sprite)


func _on_state_changed(_from: int, to: int) -> void:
	match to:
		State.IDLE:
			_face_sprite.texture = _face_tex_idle
		State.ATTACKING:
			_face_sprite.texture = _face_tex_attacking
		State.DEAD:
			_face_sprite.texture = _face_tex_dead


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
		var alpha := 0.12 / TIER_COUNT
		var ring_color := Color(team_color.r, team_color.g, team_color.b, alpha)
		for tier in range(TIER_COUNT - 1, -1, -1):
			draw_circle(Vector2.ZERO, _get_tier_radius(tier), ring_color)
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
		_lightning.remove_target(body.get_instance_id())


func take_damage(amount: int) -> void:
	if invincible_timer > 0.0 or is_dead:
		return
	lives -= amount
	invincible_timer = INVINCIBILITY_DURATION
	if lives <= 0:
		lives = 0
		is_dead = true
		_sm.transition(State.DEAD)
		_lightning.clear()
		modulate = Color(0.4, 0.4, 0.4, 1.0)
		queue_redraw()
		died.emit()


func revive() -> void:
	lives = 1
	is_dead = false
	revival_progress = 0.0
	invincible_timer = INVINCIBILITY_DURATION
	modulate = Color.WHITE
	_sm.force(State.IDLE)
	queue_redraw()


func _apply_continuous_damage(delta: float) -> void:
	if damage_per_second <= 0.0:
		_lightning.clear()
		_sm.transition(State.IDLE)
		return

	var active_targets: Dictionary = {}
	for target in targets_in_range:
		if is_instance_valid(target) and target.has_method("take_damage"):
			var ray_count := _get_ray_count(target)
			var damage_amount := damage_per_second * ray_count * delta
			var did_damage: bool = target.take_damage(damage_amount, team_color)
			if did_damage:
				active_targets[target.get_instance_id()] = {
					"target": target, "ray_count": ray_count
				}

	_lightning.update(delta, active_targets)

	if active_targets.is_empty():
		_sm.transition(State.IDLE)
	else:
		_sm.transition(State.ATTACKING)
