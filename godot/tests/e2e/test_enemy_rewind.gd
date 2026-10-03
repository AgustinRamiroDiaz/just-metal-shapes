## Enemies stay on the beat through a checkpoint rewind: a level plays into a stretch
## with enemies, every player goes down while enemies are alive, and after the rewind
## the old enemies are gone, the chart respawns them, and every action they take lands
## on its cadence beat (computed from the song beat, never a burst of stale actions).
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 120.0
const TIME_SCALE := 4.0
const LEVEL_ID := "celtic"
## Beats an action may land late (a frame at this time scale is ~0.14 beats).
const BEAT_TOLERANCE := 0.3


func run(t: E2EContext) -> void:
	var manager := await t.start_level(LEVEL_ID, GameConfig.NORMAL, TIME_SCALE)
	if manager == null:
		return
	var conductor: Node = manager.get_node("Conductor")
	var director: Node = manager.get_node("LevelDirector")
	manager.skip_countdown()

	var spawn_beat := -1.0
	for event in director.get_events():
		if event.kind == "SpawnEnemy" and event.beat > 16.0:
			spawn_beat = event.beat
			break
	if not t.check(spawn_beat > 0.0, "the level spawns enemies"):
		return
	var checkpoint := 0.0
	for beat in director.get_checkpoint_beats():
		if beat <= spawn_beat:
			checkpoint = beat

	var acts: Array = []
	director.enemy_spawned.connect(
		func(enemy: Node) -> void:
			for child in enemy.get_children():
				if child.has_signal("acted") and child.has_method("get_cadence"):
					var cadence: Vector3 = child.get_cadence()
					child.acted.connect(
						func(action_beat: float, song_beat: float) -> void:
							acts.append([action_beat, song_beat, cadence, manager.get_rewinds()])
					)
	)
	# Play from just before the checkpoint so it is reached, up to living, acting enemies.
	conductor.seek(conductor.beat_to_time(maxf(checkpoint - 1.0, 0.0)))
	await t.wait_until(func() -> bool: return acts.size() >= 2, 40.0, "enemies to spawn and act")
	var old_enemies: Array = t.tree.get_nodes_in_group("enemies")
	t.check(old_enemies.size() > 0, "enemies alive before the rewind")
	t.check(director.current_checkpoint_beat() >= checkpoint, "checkpoint reached")
	checkpoint = director.current_checkpoint_beat()

	for player in t.players():
		player.kill()
	await t.frames(2)
	t.check_eq(manager.get_rewinds(), 1, "one rewind")
	var resume_beat: float = conductor.song_beat()
	t.check(
		old_enemies.all(func(e: Variant) -> bool: return not is_instance_valid(e)),
		"enemies from before the rewind are gone"
	)

	await t.wait_until(
		func() -> bool: return acts.filter(func(a: Array) -> bool: return a[3] == 1).size() >= 4,
		40.0,
		"respawned enemies to act"
	)
	var after := acts.filter(func(a: Array) -> bool: return a[3] == 1)
	t.check(after.size() >= 4, "respawned enemies act after the rewind")
	for act in after:
		var action_beat: float = act[0]
		var song_beat: float = act[1]
		var cadence: Vector3 = act[2]
		var k := (action_beat - cadence.y) / cadence.x
		t.check_near(k, roundf(k), 0.001, "action on its cadence beat (%.2f)" % action_beat)
		t.check(
			song_beat >= action_beat - 0.001 and song_beat - action_beat <= BEAT_TOLERANCE,
			"action at %.3f lands on beat %.3f" % [song_beat, action_beat]
		)
		t.check(action_beat >= resume_beat, "no stale action from before the resume point")
	t.note(
		(
			"rewound to %.1f (resume %.1f); %d actions before, %d after"
			% [checkpoint, resume_beat, acts.size() - after.size(), after.size()]
		)
	)
