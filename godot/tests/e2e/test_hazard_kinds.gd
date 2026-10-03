## Every hazard kind, spawned on its own through `LevelDirector.spawn_event`:
## - the director has a handler for it;
## - it reports danger shapes while telegraphing (all with `activates_in > 0`) and
##   while active;
## - a player standing where it will hit takes no damage during the telegraph;
## - a player standing in it once active is damaged;
## - it frees itself when done.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 120.0
const TIME_SCALE := 2.0
## Beats of warning (1 s at the unloaded director's 120 BPM grid).
const TELEGRAPH := 2.0
const RECORD_LEN := 10

const CASES := {
	"Laser": {"x": 0.5, "y": 0.5, "angle": PI / 2.0, "size": 0.07, "duration_beats": 2.0},
	"LaserSweep":
	{"x": 0.5, "y": 0.5, "angle": 0.0, "size": 0.05, "duration_beats": 3.0, "variant": 0},
	"BulletRing":
	{
		"x": 0.5,
		"y": 0.5,
		"count": 12,
		"speed": 0.35,
		"size": 0.028,
		"duration_beats": 4.0,
		"variant": 1
	},
	"Spiral": {"x": 0.5, "y": 0.5, "count": 3, "speed": 0.4, "size": 0.026, "duration_beats": 4.0},
	"Wall": {"angle": 0.0, "x": 0.5, "y": 0.2, "size": 0.06, "duration_beats": 4.0},
	"Pulse": {"x": 0.5, "y": 0.5, "size": 0.14, "duration_beats": 2.0},
	"Bomb": {"x": 0.5, "y": 0.5, "size": 0.2, "count": 8, "speed": 0.45, "duration_beats": 3.0},
	"Spikes": {"variant": 0, "count": 6, "size": 0.2, "duration_beats": 3.0},
	"Barrage":
	{"x": 0.03, "y": 0.5, "count": 6, "speed": 0.7, "size": 0.026, "duration_beats": 4.0},
}


## Stands in for a Player: counts hits instead of losing lives.
class DummyPlayer:
	extends Node2D
	var is_dead := false
	var hits := 0

	func take_damage(_amount: float, _color := Color.WHITE) -> bool:
		hits += 1
		return true


func run(t: E2EContext) -> void:
	Engine.time_scale = TIME_SCALE
	var director := LevelDirector.new()
	t.tree.root.add_child(director)
	await t.frames(1)
	var implemented: PackedStringArray = director.get_implemented_kinds()
	for kind in CASES:
		t.check(implemented.has(kind), "%s has a handler" % kind)
		await _check_kind(t, director, kind, CASES[kind])
	director.queue_free()


func _check_kind(t: E2EContext, director: Node, kind: String, params: Dictionary) -> void:
	var player := DummyPlayer.new()
	player.add_to_group("players")
	player.position = Vector2(640, 360)
	t.tree.root.add_child(player)

	var before := director.get_child_count()
	if not t.check(director.spawn_event(kind, TELEGRAPH, TELEGRAPH, params), "%s spawns" % kind):
		player.queue_free()
		return
	if not t.check(director.get_child_count() == before + 1, "%s added a node" % kind):
		player.queue_free()
		return
	var hazard: Node = director.get_child(before)
	t.check(hazard.is_in_group("hazards"), "%s in hazards group" % kind)
	t.check(hazard.is_in_group("danger"), "%s in danger group" % kind)

	# Telegraph: stand where the hazard will hit; nothing may hurt while it warns. A hit
	# on the frame it turns active belongs to the active phase.
	var telegraph_frames := 0
	var telegraph_shapes_ok := true
	var telegraph_hits := 0
	while is_instance_valid(hazard) and not hazard.is_active():
		var records: PackedFloat32Array = hazard.danger_shapes()
		if records.is_empty():
			telegraph_shapes_ok = false
		for i in range(0, records.size(), RECORD_LEN):
			if _activates_in(records, i) <= 0.0:
				telegraph_shapes_ok = false
		if not records.is_empty():
			player.global_position = _landing_point(records, 0)
		telegraph_frames += 1
		await t.frames(1)
		if is_instance_valid(hazard) and not hazard.is_active():
			telegraph_hits = player.hits
	t.check(telegraph_frames > 3, "%s telegraphed (%d frames)" % [kind, telegraph_frames])
	t.check(telegraph_shapes_ok, "%s reports pending danger while telegraphing" % kind)
	t.check_eq(telegraph_hits, 0, "%s telegraph is harmless" % kind)

	# Active: keep standing in the first active shape until hit.
	var active_shapes := 0
	var started := Time.get_ticks_msec()
	while is_instance_valid(hazard) and (player.hits == 0 or active_shapes == 0):
		var records: PackedFloat32Array = hazard.danger_shapes()
		for i in range(0, records.size(), RECORD_LEN):
			if _activates_in(records, i) <= 0.0:
				active_shapes += 1
				player.global_position = _landing_point(records, i)
				break
		if (Time.get_ticks_msec() - started) / 1000.0 > 10.0:
			break
		await t.frames(1)
	t.check(active_shapes > 0, "%s reports active danger shapes" % kind)
	t.check(player.hits > 0, "%s damages a player standing in it" % kind)

	# It frees itself.
	var id := hazard.get_instance_id()
	await t.wait_until(
		func() -> bool: return not is_instance_id_valid(id), 15.0, "%s to free" % kind
	)
	t.note("%s: %d telegraph frames, hit %d times" % [kind, telegraph_frames, player.hits])
	player.queue_free()
	await t.frames(1)


func _activates_in(records: PackedFloat32Array, i: int) -> float:
	match int(records[i]):
		0, 1:
			return records[i + 6]
		_:
			return records[i + 8]


## Where the shape will be once it activates (moving shapes report positions that
## advance by their velocity until then).
func _landing_point(records: PackedFloat32Array, i: int) -> Vector2:
	match int(records[i]):
		0:
			var wait := maxf(records[i + 6], 0.0)
			return (
				Vector2(records[i + 1], records[i + 2])
				+ Vector2(records[i + 4], records[i + 5]) * wait
			)
		1:
			# Capsules here are lines across the arena: use the point nearest the center.
			var a := Vector2(records[i + 1], records[i + 2])
			var b := Vector2(records[i + 3], records[i + 4])
			return Geometry2D.get_closest_point_to_segment(Vector2(640, 360), a, b)
		_:
			var wait := maxf(records[i + 8], 0.0)
			return (
				Vector2(records[i + 1], records[i + 2])
				+ Vector2(records[i + 6], records[i + 7]) * wait
			)
