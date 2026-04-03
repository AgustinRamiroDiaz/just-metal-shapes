extends StaticBody2D

@onready var sprite: Sprite2D = $Sprite2D
@onready var health: HealthComponent = $HealthComponent
@onready var shooter: TurretShooterComponent = $TurretShooterComponent


func _ready() -> void:
	health.died.connect(_on_died)


func take_damage(amount: float) -> void:
	health.take_damage(amount)


func _on_died() -> void:
	queue_free()
