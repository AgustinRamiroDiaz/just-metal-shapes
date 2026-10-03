## Eight bots fill every seat (the lobby API refuses a ninth and keeps colors unique)
## and play a level, then eight hard bots face a dense bullet field. Prints decision
## cost and physics frame time; fails if bots would eat a large share of a frame.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const BotArena = preload("res://tests/bot_arena.gd")
const TIMEOUT_SECONDS := 90.0
const TIME_SCALE := 4.0
const LEVEL_SECONDS := 30.0
const STRESS_SECONDS := 8.0
const BULLETS := 160
## Budgets for all bots' decisions per game second, for the debug build of the extension
## (about 5x the release cost: 8 bots in a level take ~5 ms/s in release, the bullet
## stress ~40 ms/s).
const LEVEL_BUDGET_MS := 100.0
const STRESS_BUDGET_MS := 400.0


func run(t: E2EContext) -> void:
	_lobby_api(t)
	var previous := BotArena.set_speed(TIME_SCALE)
	await _eight_bots_in_a_level(t)
	await _eight_bots_under_fire(t)
	BotArena.restore_speed(previous)


func _lobby_api(t: E2EContext) -> void:
	var config := t.game_config()
	config.players.clear()
	t.check(config.add_human(GameConfig.KEYBOARD1), "add a human")
	t.check(not config.add_human(GameConfig.KEYBOARD1), "same input cannot join twice")
	for i in GameConfig.MAX_PLAYERS - 1:
		t.check(config.add_bot(i % 3), "add bot %d" % (i + 1))
	t.check(not config.add_bot(GameConfig.BOT_NORMAL), "ninth seat refused")
	t.check_eq(config.bot_count(), GameConfig.MAX_PLAYERS - 1, "bot_count")
	t.check_eq(config.human_count(), 1, "human_count")
	var colors := {}
	for cfg in config.players:
		colors[cfg.color] = true
	t.check_eq(colors.size(), GameConfig.MAX_PLAYERS, "every seat has its own color")
	t.check_eq(config.players[1].display_name, "BOT 1", "bots are numbered")
	t.check_eq(config.players[0].display_name, "P1", "humans are numbered")
	t.check(config.remove_human(GameConfig.KEYBOARD1), "remove the human")
	t.check(config.remove_last_bot(), "remove a bot")
	t.check(config.add_bot(GameConfig.BOT_HARD), "add a bot back")
	t.check(config.add_bot(GameConfig.BOT_HARD), "the freed seat takes another bot")
	t.check_eq(config.bot_count(), GameConfig.MAX_PLAYERS, "eight bots")
	colors.clear()
	for cfg in config.players:
		colors[cfg.color] = true
	t.check_eq(colors.size(), GameConfig.MAX_PLAYERS, "colors stay unique after edits")


func _eight_bots_in_a_level(t: E2EContext) -> void:
	var manager := await t.start_level("celtic", GameConfig.NORMAL, TIME_SCALE, true)
	if manager == null:
		return
	var bots := t.players()
	t.check_eq(bots.size(), GameConfig.MAX_PLAYERS, "eight bots spawned")
	var conductor: Node = manager.get_node("Conductor")
	manager.skip_countdown()
	var physics := {"sum": 0.0, "max": 0.0, "frames": 0}
	await t.wait_until(
		func() -> bool:
			var ms := Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS) * 1000.0
			physics.sum += ms
			physics.max = maxf(physics.max, ms)
			physics.frames += 1
			return conductor.song_time() >= LEVEL_SECONDS or manager.get_state() != "playing",
		LEVEL_SECONDS,
		"%ds of song" % LEVEL_SECONDS
	)
	var song_seconds: float = conductor.song_time()
	_report(t, "celtic, 8 bots", bots, song_seconds, LEVEL_BUDGET_MS)
	t.note(
		(
			"  physics frame: avg %.2f ms, max %.2f ms (whole physics step, all nodes)"
			% [physics.sum / maxi(physics.frames, 1), physics.max]
		)
	)


func _eight_bots_under_fire(t: E2EContext) -> void:
	var arena := BotArena.create(t.tree)
	var bots := []
	for i in GameConfig.MAX_PLAYERS:
		var position := BotArena.CENTER + Vector2.from_angle(TAU * i / 8.0) * 150.0
		bots.append(BotArena.add_player(arena, position, GameConfig.BOT, i, GameConfig.BOT_HARD))
	var shapes := []
	var rng := RandomNumberGenerator.new()
	rng.seed = 42
	for i in BULLETS:
		var from := Vector2(rng.randf_range(0, 1280), rng.randf_range(0, 720))
		var velocity := Vector2.from_angle(rng.randf() * TAU) * rng.randf_range(80, 220)
		shapes.append(BotArena.circle(from, 10.0, velocity, rng.randf_range(0.0, 0.5)))
	BotArena.scripted(arena, shapes)
	for frame in int(STRESS_SECONDS * 60.0):
		await t.tree.physics_frame
	_report(t, "%d bullets, 8 hard bots" % BULLETS, bots, STRESS_SECONDS, STRESS_BUDGET_MS)
	await BotArena.remove(arena)


func _report(
	t: E2EContext, label: String, bots: Array, game_seconds: float, budget_ms: float
) -> void:
	var decisions := 0
	var total_usec := 0.0
	var max_usec := 0
	for bot in bots:
		var s: Dictionary = bot.get_node("BotBrain").get_stats()
		decisions += s.decisions
		total_usec += s.avg_decide_usec * s.decisions
		max_usec = maxi(max_usec, s.max_decide_usec)
	var avg := total_usec / maxi(decisions, 1)
	var ms_per_second := total_usec / 1000.0 / maxf(game_seconds, 0.001)
	(
		t
		. note(
			(
				"%s: %d decisions over %.1fs, avg %.0f us, max %d us, %.1f ms of bot time per game second"
				% [label, decisions, game_seconds, avg, max_usec, ms_per_second]
			)
		)
	)
	t.check(decisions > 0, "%s: bots decided" % label)
	t.check(
		ms_per_second < budget_ms,
		"%s: bot cost %.1f ms/s under %.0f" % [label, ms_per_second, budget_ms]
	)
