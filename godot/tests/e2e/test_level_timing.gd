## A level loads, the countdown hands over to the Conductor, song time advances, and
## the director spawns every chart event at its spawn beat (within tolerance).
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 60.0
const TIME_SCALE := 4.0
const RUN_UNTIL_BEAT := 40.0
## One frame at 4x is ~0.07s of song time; allow a generous margin for slow CI frames.
const BEAT_TOLERANCE := 0.5

var _dispatched: Array = []
var _beats_signalled := 0
var _max_hazards := 0
var _max_shapes := 0


func run(t: E2EContext) -> void:
	var manager := await t.start_level("celtic", GameConfig.NORMAL, TIME_SCALE)
	if manager == null:
		return
	var conductor: Node = manager.get_node("Conductor")
	var director: Node = manager.get_node("LevelDirector")
	var field: Node = manager.get_node("DangerField")
	t.check_eq(director.get_level_id(), "celtic", "selected level loaded")
	t.check_eq(manager.get_state(), "countdown", "level starts with the countdown")
	t.check(not conductor.is_playing(), "song waits for the countdown")
	director.event_spawned.connect(
		func(kind: String, beat: float, song_beat: float) -> void:
			_dispatched.append({"kind": kind, "beat": beat, "song_beat": song_beat})
	)
	conductor.beat.connect(func(_index: int) -> void: _beats_signalled += 1)

	await t.wait_until(
		func() -> bool: return manager.get_state() == "playing", 5.0, "countdown to finish"
	)
	await t.wait_until(
		func() -> bool:
			_max_hazards = maxi(_max_hazards, t.tree.get_nodes_in_group("hazards").size())
			_max_shapes = maxi(_max_shapes, field.shape_count())
			return conductor.song_beat() >= RUN_UNTIL_BEAT,
		30.0,
		"song to reach beat %d" % RUN_UNTIL_BEAT
	)
	var final_beat: float = conductor.song_beat()
	t.check(conductor.song_time() > 15.0, "song time advanced")
	t.check_near(_beats_signalled, floorf(final_beat) + 1.0, 1.0, "beat signals emitted")

	var expected := 0
	var spawn_beats := {}
	for event in director.get_events():
		spawn_beats["%s@%.4f" % [event.kind, event.beat]] = event.spawn_beat
		if event.spawn_beat <= final_beat - BEAT_TOLERANCE:
			expected += 1
	t.check(_dispatched.size() >= expected, "dispatched %d >= %d" % [_dispatched.size(), expected])
	t.check(_dispatched.size() > 10, "a meaningful number of events dispatched")

	var late := 0
	for record in _dispatched:
		var key := "%s@%.4f" % [record.kind, record.beat]
		if not t.check(spawn_beats.has(key), "dispatched event %s is in the chart" % key):
			continue
		var lag: float = record.song_beat - spawn_beats[key]
		if lag < -0.01 or lag > BEAT_TOLERANCE:
			late += 1
			t.note(
				"%s spawned at beat %.3f, expected %.3f" % [key, record.song_beat, spawn_beats[key]]
			)
	t.check_eq(late, 0, "events spawned within %.2f beats" % BEAT_TOLERANCE)

	var kinds := {}
	for record in _dispatched:
		kinds[record.kind] = true
	for kind in ["Checkpoint", "ArenaPulse", "Pulse"]:
		t.check(kinds.has(kind), "%s events dispatched" % kind)
	t.check(_max_hazards > 0, "hazard nodes were spawned")
	t.check(_max_shapes > 0, "DangerField saw danger shapes")
	await _enemy_spawns(t, conductor, director)


func _enemy_spawns(t: E2EContext, conductor: Node, director: Node) -> void:
	var first_spawn := -1.0
	for event in director.get_events():
		if event.kind == "SpawnEnemy":
			first_spawn = event.spawn_beat
			break
	if not t.check(first_spawn >= 0.0, "chart has an enemy spawn"):
		return
	var spawned: Array = []
	director.enemy_spawned.connect(func(enemy: Node2D) -> void: spawned.append(enemy))
	conductor.seek(conductor.beat_to_time(first_spawn - 1.0))
	await t.wait_until(
		func() -> bool: return t.tree.get_nodes_in_group("enemies").size() > 0,
		10.0,
		"the chart's first enemy to spawn"
	)
	t.check_eq(spawned.size(), 1, "enemy_spawned emitted once")
