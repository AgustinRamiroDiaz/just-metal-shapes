extends StaticBody2D

@onready var sprite: Sprite2D = $Sprite2D
@onready var health: HealthComponent = $HealthComponent
@onready var shooter: ShooterComponent = $ShooterComponent


func _ready() -> void:
	health.died.connect(_on_died)


func take_damage(amount: float) -> void:
	health.take_damage(amount)


func _on_died() -> void:
	sprite.modulate = Color(0.35, 0.35, 0.35, 1.0)
	health._life_label.text = "Defeated"
	shooter.set_process(false)
