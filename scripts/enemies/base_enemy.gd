class_name BaseEnemy
extends StaticBody2D

signal died

@onready var health: HealthComponent = $HealthComponent


func _ready() -> void:
	health.died.connect(_on_died)


func take_damage(amount: float, damage_color: Color = Color.WHITE) -> bool:
	return health.take_damage(amount, damage_color)


func _on_died() -> void:
	died.emit()
	queue_free()
