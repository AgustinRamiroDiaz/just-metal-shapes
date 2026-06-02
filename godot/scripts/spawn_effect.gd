extends GPUParticles2D

signal spawn_ready

@export var duration: float = 0.8


func _ready() -> void:
	emitting = true
	await get_tree().create_timer(duration).timeout
	spawn_ready.emit()
	emitting = false
	await get_tree().create_timer(lifetime).timeout
	queue_free()
