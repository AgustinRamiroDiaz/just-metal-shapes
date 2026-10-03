## Bots only (no humans) play the first level to the end at raised time scale and clear
## it. Physics ticks scale with the time scale so bots keep their per-song-second
## decision rate. Prints run stats and per-bot decision stats.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 150.0
## 8x is the fastest that stays faithful: at 16x hazards (processed per frame) step in
## larger song-time increments than the physics-ticked bots and good bots start taking
## hits.
const TIME_SCALE := 8.0
## The first level; `-- --bot-level=<id>` plays another one.
const LEVEL_ID := "wonders-of-the-earth"
const SKILLS := [GameConfig.BOT_NORMAL, GameConfig.BOT_HARD, GameConfig.BOT_EASY]


func run(t: E2EContext) -> void:
	var config := t.game_config()
	config.players.clear()
	for skill in SKILLS:
		t.check(config.add_bot(skill), "add_bot(%d)" % skill)
	t.check_eq(config.bot_count(), SKILLS.size(), "bot_count")
	t.check_eq(config.human_count(), 0, "human_count")

	var ticks := Engine.physics_ticks_per_second
	var max_steps := Engine.max_physics_steps_per_frame
	Engine.physics_ticks_per_second = int(60 * TIME_SCALE)
	Engine.max_physics_steps_per_frame = int(8 * TIME_SCALE)
	await _play(t)
	Engine.physics_ticks_per_second = ticks
	Engine.max_physics_steps_per_frame = max_steps


func _level_id() -> String:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--bot-level="):
			return arg.trim_prefix("--bot-level=")
	return LEVEL_ID


func _play(t: E2EContext) -> void:
	var level_id := _level_id()
	var manager := await t.start_level(level_id, GameConfig.NORMAL, TIME_SCALE, true)
	if manager == null:
		return
	var bots := t.players()
	t.check_eq(bots.size(), SKILLS.size(), "one player per bot seat")
	for bot in bots:
		t.check_eq(bot.input_type, GameConfig.BOT, "bot input type")
		t.check(bot.get_node_or_null("BotBrain") != null, "bot has a BotBrain")
	var conductor: Node = manager.get_node("Conductor")
	manager.skip_countdown()

	var start := Time.get_ticks_msec()
	var travel := {"px": 0.0}
	var last_positions := {}
	var hits := {}
	for bot in bots:
		hits[bot] = 0
		bot.damaged.connect(func(_lives: int) -> void: hits[bot] += 1)
	await t.wait_until(
		func() -> bool:
			for bot in bots:
				var pos: Vector2 = bot.global_position
				if last_positions.has(bot):
					travel.px += pos.distance_to(last_positions[bot])
				last_positions[bot] = pos
			return manager.get_state() in ["cleared", "game_over"],
		TIMEOUT_SECONDS - 10.0,
		"the level to end"
	)
	var travelled: float = travel.px
	var real := (Time.get_ticks_msec() - start) / 1000.0
	var stats: Dictionary = manager.get_run_stats()
	(
		t
		. note(
			(
				"%s: %s in %.1fs real (%.1fs song), hits=%d downs=%d revives=%d rewinds=%d kills=%d rank=%s"
				% [
					level_id,
					manager.get_state(),
					real,
					conductor.song_time(),
					stats.hits_taken,
					stats.downs,
					stats.revives,
					stats.rewinds,
					stats.enemies_killed,
					stats.rank
				]
			)
		)
	)
	for bot in bots:
		var brain: Node = bot.get_node("BotBrain")
		var s: Dictionary = brain.get_stats()
		t.note(
			(
				"  %s (%s): hits=%d decisions=%d avg=%.0fus max=%dus hit_predictions=%d"
				% [
					bot.get_meta("display_name"),
					GameConfig.bot_skill_name(s.skill),
					hits[bot],
					s.decisions,
					s.avg_decide_usec,
					s.max_decide_usec,
					s.hit_predictions
				]
			)
		)
		t.check(s.decisions > 1000, "bot made decisions")
	t.check(manager.is_level_clear(), "bots cleared the level")
	t.check(stats.completed, "run stats: completed")
	t.check(travelled > 2000.0, "bots moved (%.0f px)" % travelled)
