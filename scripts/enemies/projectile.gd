extends Area2D

var direction: Vector2 = Vector2.RIGHT
var speed: float = 100.0
var lifetime: float = 30.0


func _ready() -> void:
	body_entered.connect(_on_body_entered)
	var notifier := VisibleOnScreenNotifier2D.new()
	notifier.screen_exited.connect(queue_free)
	add_child(notifier)
	queue_redraw()


func _process(delta: float) -> void:
	position += direction * speed * delta
	lifetime -= delta
	if lifetime <= 0.0:
		queue_free()


func _draw() -> void:
	draw_circle(Vector2.ZERO, 10.0, Color(1.0, 0.9, 0.2, 1.0))
	draw_arc(Vector2.ZERO, 10.0, 0.0, TAU, 16, Color(1.0, 0.6, 0.0, 1.0), 1.5)


func _on_body_entered(body: Node) -> void:
	if body.has_method("take_damage"):
		body.take_damage(1)
	queue_free()
