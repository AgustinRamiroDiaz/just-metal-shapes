## Bots revive downed teammates: a bot walks over to a downed human and revives them;
## a downed bot stays put (no movement, no decisions) until a bot teammate revives it.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const BotArena = preload("res://tests/bot_arena.gd")
const TIMEOUT_SECONDS := 60.0
const TIME_SCALE := 4.0
const REVIVE_DISTANCE := 60.0


func run(t: E2EContext) -> void:
	var previous := BotArena.set_speed(TIME_SCALE)
	await _bot_revives_human(t)
	await _bot_revives_bot(t)
	BotArena.restore_speed(previous)


func _bot_revives_human(t: E2EContext) -> void:
	var arena := BotArena.create(t.tree)
	# A human seat with no input pressed stands still.
	var human := BotArena.add_player(arena, Vector2(250, 500), GameConfig.KEYBOARD2, 0)
	var bot := BotArena.add_player(arena, Vector2(1050, 200), GameConfig.BOT, 1)
	await t.tree.physics_frame
	human.kill()
	t.check(human.is_dead, "human is down")
	var revived := {"distance": -1.0}
	human.revived.connect(
		func() -> void: revived.distance = bot.global_position.distance_to(human.global_position)
	)
	await t.wait_until(
		func() -> bool: return not human.is_dead, 10.0, "the bot to revive the human"
	)
	t.check(revived.distance >= 0.0, "revived signal emitted")
	t.check(
		revived.distance <= REVIVE_DISTANCE,
		"bot within revive distance (%.0f px)" % revived.distance
	)
	t.check_eq(human.lives, 1, "revived with one life")
	t.note("bot revived the human from %.0f px" % revived.distance)
	await BotArena.remove(arena)


func _bot_revives_bot(t: E2EContext) -> void:
	var arena := BotArena.create(t.tree)
	var downed := BotArena.add_player(arena, Vector2(400, 360), GameConfig.BOT, 0)
	var rescuer := BotArena.add_player(arena, Vector2(1000, 200), GameConfig.BOT, 1)
	await t.frames(5)
	downed.kill()
	var brain: Node = downed.get_node("BotBrain")
	var down_position: Vector2 = downed.global_position
	var decisions_when_down: int = brain.get_stats().decisions
	await t.seconds(0.3)
	t.check(downed.is_dead, "downed bot stays down while nobody is near")
	t.check_eq(downed.global_position, down_position, "downed bot does not move")
	t.check_eq(brain.get_move_direction(), Vector2.ZERO, "downed bot has no heading")
	t.check_eq(brain.get_stats().decisions, decisions_when_down, "downed bot makes no decisions")

	await t.wait_until(func() -> bool: return not downed.is_dead, 10.0, "the rescuer to revive it")
	t.check(
		rescuer.global_position.distance_to(downed.global_position) <= REVIVE_DISTANCE,
		"rescuer within revive distance"
	)
	await t.seconds(0.5)
	t.check(brain.get_stats().decisions > decisions_when_down, "revived bot decides again")
	t.check(downed.global_position != down_position, "revived bot moves again")
	await BotArena.remove(arena)
