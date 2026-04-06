class_name LightningComponent
extends Node

const TEXTURES: Array[Texture2D] = [
	preload("res://assets/kenney-particles/Rotated/spark_05_rotated.png"),
	preload("res://assets/kenney-particles/Rotated/spark_06_rotated.png"),
]
const FPS: float = 12.0
const WIDTH: float = 80.0
const RAY_SPACING: float = 10.0

var _lines: Dictionary = {}  # target_id -> Array[Line2D]
var _frame_timer: float = 0.0
var _frame_index: int = 0


func update(delta: float, active_targets: Dictionary) -> void:
	_cycle_frame(delta)
	_sync_lines(active_targets)


func clear() -> void:
	for lines: Array in _lines.values():
		for line: Line2D in lines:
			line.queue_free()
	_lines.clear()


func remove_target(target_id: int) -> void:
	if target_id in _lines:
		for line: Line2D in _lines[target_id]:
			line.queue_free()
		_lines.erase(target_id)


func _cycle_frame(delta: float) -> void:
	_frame_timer += delta
	if _frame_timer >= 1.0 / FPS:
		_frame_timer -= 1.0 / FPS
		_frame_index = (_frame_index + 1) % TEXTURES.size()
		for lines: Array in _lines.values():
			for line: Line2D in lines:
				line.texture = TEXTURES[_frame_index]


func _sync_lines(active_targets: Dictionary) -> void:
	var parent: Player = get_parent() as Player
	var color: Color = parent.team_color

	# Remove lines for targets no longer active
	for target_id in _lines.keys():
		if target_id not in active_targets:
			remove_target(target_id)

	# Update or create lines for active targets
	for target_id: int in active_targets:
		var info: Dictionary = active_targets[target_id]
		var target: Node2D = info["target"]
		var ray_count: int = info["ray_count"]
		var lines: Array = _lines.get(target_id, []) as Array

		while lines.size() < ray_count:
			var line := Line2D.new()
			line.texture = TEXTURES[_frame_index]
			line.texture_mode = Line2D.LINE_TEXTURE_TILE
			line.width = WIDTH
			line.default_color = Color(color, 0.8)
			line.z_index = -1
			parent.add_child(line)
			lines.append(line)
		while lines.size() > ray_count:
			var extra: Line2D = lines.pop_back()
			extra.queue_free()

		_lines[target_id] = lines

		var target_local := parent.to_local(target.global_position)
		var perp := target_local.normalized().rotated(PI / 2.0)
		for i in range(ray_count):
			var offset := perp * (i - (ray_count - 1) / 2.0) * RAY_SPACING
			lines[i].clear_points()
			lines[i].add_point(target_local + offset)
			lines[i].add_point(Vector2.ZERO + offset)
