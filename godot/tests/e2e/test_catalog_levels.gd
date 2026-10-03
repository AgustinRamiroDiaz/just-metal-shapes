## Every LevelCatalog level has music, a parseable analysis and a playable chart.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 30.0


func run(t: E2EContext) -> void:
	var count: int = LevelCatalog.count()
	t.check(count >= 3, "catalog has at least 3 levels")
	var last_difficulty := 0
	for i in count:
		var info: Dictionary = LevelCatalog.get_level(i)
		var id: String = info.id
		t.check_eq(LevelCatalog.index_of(id), i, "index_of(%s)" % id)
		t.check(info.difficulty > last_difficulty, "%s difficulty increases" % id)
		last_difficulty = info.difficulty
		t.check(ResourceLoader.exists(info.music_path), "%s music exists" % id)
		t.check(load(info.music_path) is AudioStream, "%s music loads" % id)
		t.check(FileAccess.file_exists(info.analysis_path), "%s analysis exists" % id)

		var director := LevelDirector.new()
		t.tree.root.add_child(director)
		if t.check(director.load_level(id), "%s chart generates" % id):
			var summary: Dictionary = director.get_chart_summary()
			t.check(summary.bpm > 60.0 and summary.bpm < 200.0, "%s bpm sane" % id)
			t.check(summary.duration_seconds > 60.0, "%s duration" % id)
			t.check(summary.hazard_count > 10, "%s has hazards" % id)
			t.check(summary.enemy_count > 0, "%s has enemy spawns" % id)
			t.check(summary.checkpoint_count > 0, "%s has checkpoints" % id)
			var previous := -1.0
			var sorted := true
			for event in director.get_events():
				sorted = sorted and event.beat >= previous
				previous = event.beat
			t.check(sorted, "%s events sorted" % id)
			t.note(
				(
					"%s: bpm %.1f, %d events, %d hazards"
					% [id, summary.bpm, summary.event_count, summary.hazard_count]
				)
			)
		director.queue_free()
	await t.frames(1)
	t.check(LevelCatalog.find_level("missing").is_empty(), "unknown id gives empty dictionary")
