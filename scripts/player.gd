extends CharacterBody2D

@export var speed: float = 220.0
@export var range_radius: float = 140.0:
	set = set_range_radius
@export var team_color: Color = Color(0.35, 0.75, 1.0, 1.0)
@export var move_left_action: StringName = &"ui_left"
@export var move_right_action: StringName = &"ui_right"
@export var move_up_action: StringName = &"ui_up"
@export var move_down_action: StringName = &"ui_down"

@onready var range_area: Area2D = $RangeArea
@onready var range_shape: CollisionShape2D = $RangeArea/CollisionShape2D

var targets_in_range: Array[Node2D] = []


func _ready() -> void:
	_update_range_shape()
	queue_redraw()
	range_area.body_entered.connect(_on_range_body_entered)
	range_area.body_exited.connect(_on_range_body_exited)


func _physics_process(_delta: float) -> void:
	var input_dir := Input.get_vector(
		move_left_action,
		move_right_action,
		move_up_action,
		move_down_action
	)
	velocity = input_dir * speed
	move_and_slide()


func _draw() -> void:
	var ring_color := Color(team_color.r, team_color.g, team_color.b, 0.9)
	var fill_color := Color(team_color.r, team_color.g, team_color.b, 0.08)
	draw_arc(Vector2.ZERO, range_radius, 0.0, TAU, 64, ring_color, 2.5)
	draw_circle(Vector2.ZERO, range_radius, fill_color)


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


func _on_damage_timer_timeout() -> void:
	for target in targets_in_range:
		if is_instance_valid(target) and target.has_method("take_damage"):
			target.take_damage(1)
