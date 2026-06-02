extends Area2D

const ARM_TIME: float = 0.5
const LIFETIME: float = 15.0
const RADIUS: float = 10.0

var _armed: bool = false


func _ready() -> void:
	collision_layer = 0
	collision_mask = 1
	body_entered.connect(_on_body_entered)
	queue_redraw()

	var arm_timer := Timer.new()
	arm_timer.wait_time = ARM_TIME
	arm_timer.one_shot = true
	arm_timer.timeout.connect(_arm)
	add_child(arm_timer)
	arm_timer.start()

	var lifetime_timer := Timer.new()
	lifetime_timer.wait_time = LIFETIME
	lifetime_timer.one_shot = true
	lifetime_timer.timeout.connect(queue_free)
	add_child(lifetime_timer)
	lifetime_timer.start()


func _arm() -> void:
	_armed = true
	queue_redraw()


func _process(_delta: float) -> void:
	if _armed:
		queue_redraw()


func _draw() -> void:
	var color := Color(1.0, 0.3, 0.1, 1.0) if _armed else Color(0.5, 0.5, 0.5, 0.6)
	draw_circle(Vector2.ZERO, RADIUS, color)
	if _armed:
		var pulse := sin(Time.get_ticks_msec() / 1000.0 * 6.0) * 0.3 + 0.7
		var glow := Color(1.0, 0.5, 0.1, pulse * 0.4)
		draw_circle(Vector2.ZERO, RADIUS + 4.0, glow)


func _on_body_entered(body: Node) -> void:
	if not _armed:
		return
	if body.has_method("take_damage"):
		body.take_damage(1)
	queue_free()
