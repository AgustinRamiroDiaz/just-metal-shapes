## All players down: normal mode rewinds to the last checkpoint with players revived
## and the arena cleared; hardcore mode ends the run.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 60.0
const TIME_SCALE := 4.0


func run(t: E2EContext) -> void:
	await _normal_rewinds(t)
	await _hardcore_game_over(t)


func _normal_rewinds(t: E2EContext) -> void:
	var manager := await t.start_level("celtic", GameConfig.NORMAL, TIME_SCALE, true)
	if manager == null:
		return
	var conductor: Node = manager.get_node("Conductor")
	var director: Node = manager.get_node("LevelDirector")
	manager.skip_countdown()
	var checkpoints: PackedFloat64Array = director.get_checkpoint_beats()
	if not t.check(checkpoints.size() >= 2, "level has a second checkpoint"):
		return
	var checkpoint := checkpoints[1]
	# Jump past the second checkpoint and let it be reached.
	conductor.seek(conductor.beat_to_time(checkpoint + 8.0))
	await t.frames(5)
	t.check_near(director.current_checkpoint_beat(), checkpoint, 0.001, "checkpoint reached")
	await t.wait_until(
		func() -> bool: return t.tree.get_nodes_in_group("hazards").size() > 0,
		10.0,
		"a hazard to be on screen"
	)
	var rewound_beats: Array = []
	manager.rewound.connect(func(_count: int, beat: float) -> void: rewound_beats.append(beat))
	checkpoint = director.current_checkpoint_beat()
	t.check(checkpoint >= checkpoints[1], "a later checkpoint is the rewind target")

	for player in t.players():
		player.kill()
	await t.frames(2)

	t.check_eq(manager.get_rewinds(), 1, "one rewind")
	t.check_eq(rewound_beats, [checkpoint], "rewound signal carries the checkpoint")
	t.check_eq(manager.get_state(), "playing", "level keeps playing")
	t.check_near(conductor.song_beat(), checkpoint, 1.0, "song seeked to the checkpoint")
	t.check_eq(t.tree.get_nodes_in_group("hazards").size(), 0, "hazards cleared")
	t.check_eq(t.tree.get_nodes_in_group("enemy_projectiles").size(), 0, "projectiles cleared")
	for player in t.players():
		t.check(not player.is_dead, "player revived")
		t.check_eq(player.lives, Player.MAX_LIVES, "player lives restored")

	# The chart replays from the checkpoint.
	var before: int = director.get_cursor()
	await t.wait_until(
		func() -> bool: return director.get_cursor() > before, 10.0, "chart to replay"
	)


func _hardcore_game_over(t: E2EContext) -> void:
	var manager := await t.start_level("celtic", GameConfig.HARDCORE, TIME_SCALE, true)
	if manager == null:
		return
	var conductor: Node = manager.get_node("Conductor")
	manager.skip_countdown()
	await t.frames(10)
	for player in t.players():
		player.kill()
	await t.frames(2)
	t.check(manager.is_game_over(), "hardcore: game over")
	t.check_eq(manager.get_rewinds(), 0, "hardcore: no rewinds")
	t.check(not conductor.is_playing(), "hardcore: song stopped")
	t.check(t.tree.current_scene.get_node_or_null("EndScreen") != null, "end screen shown")
