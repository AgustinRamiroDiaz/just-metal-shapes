extends StaticBody2D

@export var max_life: int = 3

@onready var sprite: Sprite2D = $Sprite2D
@onready var life_label: Label = $LifeLabel

var life: int = 0


func _ready() -> void:
	life = max_life
	_update_life_label()


func take_damage(amount: int) -> void:
	if life <= 0:
		return

	life = max(life - amount, 0)
	_update_life_label()

	if life == 0:
		sprite.modulate = Color(0.35, 0.35, 0.35, 1.0)
		life_label.text = "Defeated"


func _update_life_label() -> void:
	life_label.text = "Life: %d" % life
