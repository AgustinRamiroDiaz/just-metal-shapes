## Every LevelCatalog level plays its whole chart at high speed with god-mode players:
## the song finishes and the level is cleared, every chart event is dispatched once
## (per kind, matching the chart), hazards clean up after themselves, and the
## events-per-kind breakdown is reported.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 400.0
const TIME_SCALE := 12.0
## Real seconds allowed per level (the longest song is ~162 s, ~14 s at 12x).
const LEVEL_TIMEOUT := 70.0


func run(t: E2EContext) -> void:
	for i in LevelCatalog.count():
		var info: Dictionary = LevelCatalog.get_level(i)
		await _play(t, info.id)


func _play(t: E2EContext, id: String) -> void:
	var manager := await t.start_level(id, GameConfig.NORMAL, TIME_SCALE)
	if manager == null:
		return
	var director: Node = manager.get_node("LevelDirector")
	var expected := {}
	for event in director.get_events():
		expected[event.kind] = expected.get(event.kind, 0) + 1
	# Dictionaries are shared with the lambdas below (plain locals would be copied).
	var stats := {"counts": {}, "max_hazards": 0, "max_shapes": 0}
	director.event_spawned.connect(
		func(kind: String, _beat: float, _song_beat: float) -> void:
			stats.counts[kind] = stats.counts.get(kind, 0) + 1
	)
	var field: Node = manager.get_node("DangerField")
	manager.skip_countdown()
	var started := Time.get_ticks_msec()
	var cleared := await t.wait_until(
		func() -> bool:
			stats.max_hazards = maxi(stats.max_hazards, t.tree.get_nodes_in_group("hazards").size())
			stats.max_shapes = maxi(stats.max_shapes, field.shape_count())
			return manager.get_state() == "cleared",
		LEVEL_TIMEOUT,
		"%s to be cleared" % id
	)
	if not cleared:
		return
	var seconds := (Time.get_ticks_msec() - started) / 1000.0
	t.check_eq(director.get_cursor(), director.get_event_count(), "%s: every event dispatched" % id)
	for kind in expected:
		t.check_eq(
			stats.counts.get(kind, 0), expected[kind], "%s: %s events dispatched" % [id, kind]
		)
	t.check(stats.max_hazards > 0, "%s: hazards appeared" % id)
	t.check(stats.max_shapes > 0, "%s: DangerField saw shapes" % id)
	await t.wait_until(
		func() -> bool: return t.tree.get_nodes_in_group("hazards").is_empty(),
		2.0,
		"%s: hazards to clean up after the song" % id
	)

	var kinds: Array = stats.counts.keys()
	kinds.sort()
	var parts: PackedStringArray = []
	for kind in kinds:
		parts.append("%s=%d" % [kind, stats.counts[kind]])
	t.note(
		(
			"%s: cleared in %.1fs real, peak %d hazards / %d shapes"
			% [id, seconds, stats.max_hazards, stats.max_shapes]
		)
	)
	t.note("  " + " ".join(parts))
