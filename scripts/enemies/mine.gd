extends Area2D

const ARM_TIME: float = 0.5
const LIFETIME: float = 15.0
const RADIUS: float = 10.0

var _timer: float = 0.0
var _armed: bool = false


func _ready() -> void:
	collision_layer = 0
	collision_mask = 1
	body_entered.connect(_on_body_entered)
	queue_redraw()


func _process(delta: float) -> void:
	_timer += delta
	if not _armed and _timer >= ARM_TIME:
		_armed = true
		queue_redraw()
	if _timer >= LIFETIME:
		queue_free()


func _draw() -> void:
	var color := Color(1.0, 0.3, 0.1, 1.0) if _armed else Color(0.5, 0.5, 0.5, 0.6)
	draw_circle(Vector2.ZERO, RADIUS, color)
	if _armed:
		var pulse := sin(_timer * 6.0) * 0.3 + 0.7
		var glow := Color(1.0, 0.5, 0.1, pulse * 0.4)
		draw_circle(Vector2.ZERO, RADIUS + 4.0, glow)


func _on_body_entered(body: Node) -> void:
	if not _armed:
		return
	if body.has_method("take_damage"):
		body.take_damage(1)
	queue_free()
