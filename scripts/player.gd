extends CharacterBody2D

@export var speed: float = 220.0
@export var range_radius: float = 140.0:
	set = set_range_radius

@onready var range_area: Area2D = $RangeArea
@onready var range_shape: CollisionShape2D = $RangeArea/CollisionShape2D

var target_enemy: Node = null


func _ready() -> void:
	_update_range_shape()
	queue_redraw()
	range_area.body_entered.connect(_on_range_body_entered)
	range_area.body_exited.connect(_on_range_body_exited)


func _physics_process(_delta: float) -> void:
	var input_dir := Input.get_vector("ui_left", "ui_right", "ui_up", "ui_down")
	velocity = input_dir * speed
	move_and_slide()


func _draw() -> void:
	draw_arc(Vector2.ZERO, range_radius, 0.0, TAU, 64, Color(0.4, 0.8, 1.0, 0.9), 2.5)
	draw_circle(Vector2.ZERO, range_radius, Color(0.4, 0.8, 1.0, 0.08))


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
	if body.has_method("take_damage"):
		target_enemy = body


func _on_range_body_exited(body: Node) -> void:
	if body == target_enemy:
		target_enemy = null


func _on_damage_timer_timeout() -> void:
	if target_enemy != null and is_instance_valid(target_enemy):
		target_enemy.take_damage(1)
