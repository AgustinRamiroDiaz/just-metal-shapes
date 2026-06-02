class_name RevivalComponent
extends Node

const REVIVAL_DISTANCE: float = 60.0
const REVIVAL_TIME: float = 2.0

var _timer: float = 0.0


func _process(delta: float) -> void:
	var player := owner as Player
	if not player.is_dead:
		_reset()
		return

	var reviver := _find_nearest_alive_player(player.global_position)
	if reviver != null:
		_timer += delta
		player.revival_progress = _timer / REVIVAL_TIME
		player.queue_redraw()
		if _timer >= REVIVAL_TIME:
			_reset()
			player.revive()
	else:
		_reset()


func _reset() -> void:
	if _timer > 0.0:
		_timer = 0.0
		var player := owner as Player
		player.revival_progress = 0.0
		player.queue_redraw()


func _find_nearest_alive_player(origin: Vector2) -> Player:
	var nearest: Player = null
	var nearest_dist := REVIVAL_DISTANCE + 1.0
	for p in get_tree().get_nodes_in_group("players"):
		if p == owner or p.is_dead:
			continue
		var d: float = (p as Player).global_position.distance_to(origin)
		if d <= REVIVAL_DISTANCE and d < nearest_dist:
			nearest_dist = d
			nearest = p
	return nearest
