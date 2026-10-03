## One bot against one hazard at a time, in a bare arena. Generic danger shapes cover
## each `core::danger` record type the way chart hazards report them (telegraphed pulse,
## laser cross, wall with a gap, bullet ring); real nodes cover the Pulse hazard, enemy
## projectiles and a mine. Pulses use the chart defaults (radius 100 px, 2-beat
## telegraph). Normal and hard bots must take no hits; easy bots are only
## reported.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const BotArena = preload("res://tests/bot_arena.gd")
const TIMEOUT_SECONDS := 120.0
const TIME_SCALE := 4.0
const CENTER := BotArena.CENTER
const SKILLS := [GameConfig.BOT_NORMAL, GameConfig.BOT_HARD, GameConfig.BOT_EASY]


func run(t: E2EContext) -> void:
	var previous := BotArena.set_speed(TIME_SCALE)
	for skill in SKILLS:
		await _case(t, skill, "pulse", 2.0, _pulse_shapes)
		await _case(t, skill, "laser_cross", 2.0, _laser_shapes)
		await _case(t, skill, "wall_gap", 5.0, _wall_shapes)
		await _case(t, skill, "bullet_ring", 3.0, _ring_shapes)
		await _case(t, skill, "real_pulse", 2.0, _real_pulse)
		await _case(t, skill, "projectile_fan", 3.0, _real_projectiles)
		await _case(t, skill, "mine", 3.0, _real_mine)
	BotArena.restore_speed(previous)


## Runs `setup(arena, bot)` then plays `seconds` of game time, counting physics frames
## the bot overlaps an active shape plus hits it takes from real hazards.
func _case(t: E2EContext, skill: int, label: String, seconds: float, setup: Callable) -> void:
	var arena := BotArena.create(t.tree)
	var bot := BotArena.add_player(arena, CENTER, GameConfig.BOT, 0, skill)
	var field: Node = arena.get_node("DangerField")
	var damage := {"hits": 0}
	bot.damaged.connect(func(_lives: int) -> void: damage.hits += 1)
	await t.tree.physics_frame
	setup.call(arena, bot)
	var overlap_frames := 0
	for frame in int(seconds * 60.0):
		await t.tree.physics_frame
		field.refresh()
		if field.is_hit(bot.global_position, BotArena.HIT_RADIUS, 0.0):
			overlap_frames += 1
	var skill_name := GameConfig.bot_skill_name(skill)
	var hits: int = damage.hits
	t.note(
		(
			"%-15s %-6s overlap_frames=%d hits=%d end=%s"
			% [label, skill_name, overlap_frames, hits, bot.global_position.round()]
		)
	)
	if skill != GameConfig.BOT_EASY:
		t.check_eq(overlap_frames + hits, 0, "%s bot dodges %s" % [skill_name, label])
	await BotArena.remove(arena)


func _pulse_shapes(arena: Node2D, _bot: Node) -> void:
	BotArena.scripted(arena, [BotArena.circle(CENTER, 100.0, Vector2.ZERO, 0.8)])


func _laser_shapes(arena: Node2D, _bot: Node) -> void:
	(
		BotArena
		. scripted(
			arena,
			[
				BotArena.capsule(Vector2(0, 360), Vector2(1280, 360), 22.0, 0.6),
				BotArena.capsule(Vector2(640, 0), Vector2(640, 720), 22.0, 0.9),
			]
		)
	)


func _wall_shapes(arena: Node2D, _bot: Node) -> void:
	var gap_y := 600.0
	var gap_half := 60.0
	var top_half := (gap_y - gap_half) * 0.5
	var bottom_half := (720.0 - gap_y - gap_half) * 0.5
	var velocity := Vector2(-250, 0)
	(
		BotArena
		. scripted(
			arena,
			[
				BotArena.rect(Vector2(1150, top_half), Vector2(18, top_half), 0.0, velocity, 0.0),
				BotArena.rect(
					Vector2(1150, 720.0 - bottom_half), Vector2(18, bottom_half), 0.0, velocity, 0.0
				),
			]
		)
	)


func _ring_shapes(arena: Node2D, _bot: Node) -> void:
	var origin := CENTER + Vector2(-160, 20)
	var shapes := []
	for i in 16:
		var dir := Vector2.from_angle(TAU * i / 16.0)
		shapes.append(BotArena.circle(origin + dir * 20.0, 10.0, dir * 220.0, 0.4))
	BotArena.scripted(arena, shapes)


func _real_pulse(arena: Node2D, _bot: Node) -> void:
	var pulse := PulseHazard.new()
	pulse.radius = 100.0
	pulse.position = CENTER
	pulse.configure(0.0, 0.8, 1.6)
	pulse.add_to_group("danger")
	arena.add_child(pulse)


func _real_projectiles(arena: Node2D, _bot: Node) -> void:
	var scene: PackedScene = load("res://scenes/projectile.tscn")
	var origin := CENTER + Vector2(-320, 0)
	for i in 5:
		var projectile: Node2D = scene.instantiate()
		projectile.position = origin
		projectile.direction = Vector2.RIGHT.rotated(deg_to_rad(-20 + 10 * i))
		projectile.speed = 260.0
		arena.add_child(projectile)


func _real_mine(arena: Node2D, _bot: Node) -> void:
	var mine: Node2D = load("res://scenes/mine.tscn").instantiate()
	mine.position = CENTER + Vector2(10, 0)
	arena.add_child(mine)
