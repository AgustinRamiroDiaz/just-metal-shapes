## Bare arena for bot scenarios: a scene with a DangerField and players, without a
## level, director or conductor, so a single hazard can be tested in isolation.
extends RefCounted

const PLAYER_SCENE := preload("res://scenes/player.tscn")
const CENTER := Vector2(640, 360)
## Hit radius hazards use against players (`hazards::PLAYER_HIT_RADIUS`).
const HIT_RADIUS := 11.0


## Danger records (`core::danger` layout) moved by their velocity and counted down by
## `activates_in` as physics time passes; the node itself never damages anyone.
class ScriptedDanger:
	extends Node2D

	var records := PackedFloat32Array()
	var elapsed := 0.0

	func _ready() -> void:
		add_to_group("danger")

	func _physics_process(delta: float) -> void:
		elapsed += delta

	func danger_shapes() -> PackedFloat32Array:
		var out := records.duplicate()
		for i in range(0, out.size(), 10):
			match int(out[i]):
				0:
					out[i + 1] += out[i + 4] * elapsed
					out[i + 2] += out[i + 5] * elapsed
					out[i + 6] = maxf(0.0, out[i + 6] - elapsed)
				1:
					out[i + 6] = maxf(0.0, out[i + 6] - elapsed)
				2:
					out[i + 1] += out[i + 6] * elapsed
					out[i + 2] += out[i + 7] * elapsed
					out[i + 8] = maxf(0.0, out[i + 8] - elapsed)
		return out


static func create(tree: SceneTree) -> Node2D:
	var arena := Node2D.new()
	arena.name = "BotArena"
	var field := DangerField.new()
	field.name = "DangerField"
	arena.add_child(field)
	tree.root.add_child(arena)
	tree.current_scene = arena
	return arena


static func add_player(
	arena: Node2D, position: Vector2, input_type: int, color_index: int, skill := 1
) -> Node:
	var player: Node = PLAYER_SCENE.instantiate()
	player.position = position
	player.input_type = input_type
	player.team_color = GameConfig.get_player_colors()[color_index]
	if input_type == GameConfig.BOT:
		var brain := BotBrain.new()
		brain.name = "BotBrain"
		brain.skill = skill
		brain.seed = color_index
		player.add_child(brain)
	arena.add_child(player)
	return player


static func circle(center: Vector2, radius: float, velocity: Vector2, activates_in: float) -> Array:
	return [0, center.x, center.y, radius, velocity.x, velocity.y, activates_in, 0, 0, 0]


static func capsule(a: Vector2, b: Vector2, radius: float, activates_in: float) -> Array:
	return [1, a.x, a.y, b.x, b.y, radius, activates_in, 0, 0, 0]


static func rect(
	center: Vector2, half: Vector2, angle: float, velocity: Vector2, activates_in: float
) -> Array:
	return [2, center.x, center.y, half.x, half.y, angle, velocity.x, velocity.y, activates_in, 0]


static func scripted(arena: Node2D, shapes: Array) -> Node2D:
	var node := ScriptedDanger.new()
	for shape in shapes:
		node.records.append_array(PackedFloat32Array(shape))
	arena.add_child(node)
	return node


## Uses the time scale and scales physics ticks with it so bots keep their decision
## rate per game second. Returns the previous settings for `restore_speed`.
static func set_speed(time_scale: float) -> Array:
	var previous := [Engine.physics_ticks_per_second, Engine.max_physics_steps_per_frame]
	Engine.time_scale = time_scale
	Engine.physics_ticks_per_second = int(60 * time_scale)
	Engine.max_physics_steps_per_frame = int(8 * time_scale)
	return previous


static func restore_speed(previous: Array) -> void:
	Engine.time_scale = 1.0
	Engine.physics_ticks_per_second = previous[0]
	Engine.max_physics_steps_per_frame = previous[1]


static func remove(arena: Node2D) -> void:
	var tree := arena.get_tree()
	if tree.current_scene == arena:
		tree.current_scene = null
	arena.queue_free()
	await tree.process_frame
