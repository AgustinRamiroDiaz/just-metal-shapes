extends StaticBody2D

@export var max_life: int = 3
@export var shield_color: Color = Color(1.0, 0.35, 0.35, 1.0)

@onready var sprite: Sprite2D = $Sprite2D
@onready var life_label: Label = $LifeLabel

var life: int = 0


func _ready() -> void:
	life = max_life
	_update_life_label()
	queue_redraw()


func take_damage(amount: int) -> void:
	if life <= 0:
		return

	life = max(life - amount, 0)
	_update_life_label()
	queue_redraw()

	if life == 0:
		sprite.modulate = Color(0.35, 0.35, 0.35, 1.0)
		life_label.text = "Defeated"


func _draw() -> void:
	var ratio := float(life) / float(max(max_life, 1))
	draw_arc(Vector2.ZERO, 34.0, 0.0, TAU, 64, Color(0.2, 0.2, 0.2, 0.5), 6.0)
	draw_arc(
		Vector2.ZERO,
		34.0,
		-PI / 2.0,
		-PI / 2.0 + (TAU * ratio),
		64,
		Color(shield_color.r, shield_color.g, shield_color.b, 1.0),
		6.0
	)


func _update_life_label() -> void:
	life_label.text = "Life: %d" % life
