extends StaticBody2D

signal died

@onready var sprite: Sprite2D = $Sprite2D
@onready var health: HealthComponent = $HealthComponent
@onready var shooter: ShotgunShooterComponent = $ShotgunShooterComponent
@onready var chaser: ChaserComponent = $ChaserComponent
@onready var turn: TurnComponent = $TurnComponent


func _ready() -> void:
	health.died.connect(_on_died)


func take_damage(amount: float, damage_color: Color = Color.WHITE) -> void:
	health.take_damage(amount, damage_color)


func _on_died() -> void:
	died.emit()
	queue_free()
