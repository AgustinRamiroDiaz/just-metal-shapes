## Fx autoload and level presentation: shake decays back to a still camera, hit-stop
## restores Engine.time_scale (also when paused or overridden), the Arena reacts to
## arena_pulse, and enemy/player visuals run their effects without errors.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 60.0
const ENEMY_SCENES := [
	"res://scenes/static_shooter_enemy.tscn",
	"res://scenes/shotgun_enemy.tscn",
	"res://scenes/turret_enemy.tscn",
	"res://scenes/runner_enemy.tscn",
	"res://scenes/mine_layer_enemy.tscn",
]


func run(t: E2EContext) -> void:
	var fx: Node = t.tree.root.get_node_or_null("Fx")
	if not t.check(fx != null, "Fx autoload exists"):
		return
	var manager := await t.start_level("wonders-of-the-earth", GameConfig.NORMAL, 1.0)
	if manager == null:
		return
	manager.skip_countdown()
	await t.frames(2)
	await _shake(t, fx, manager)
	await _hitstop(t, fx)
	await _arena(t, manager)
	await _effects(t, fx, manager)
	await _enemies(t, manager)
	await _player_hit(t)


func _shake(t: E2EContext, fx: Node, manager: Node) -> void:
	var camera: Camera2D = manager.get_node_or_null("Camera2D")
	if not t.check(camera != null, "level has a Camera2D"):
		return
	fx.shake(0.9)
	if fx.shake_scale() <= 0.0:
		t.note("screen shake disabled in settings; checking it stays still")
		await t.frames(3)
		t.check_eq(camera.offset, Vector2.ZERO, "no shake when disabled")
		return
	t.check(fx.get_trauma() > 0.5, "shake adds trauma")
	await t.frames(3)
	t.check(camera.offset.length() > 0.0, "camera offset moves while shaking")
	await t.wait_until(func() -> bool: return fx.get_trauma() == 0.0, 3.0, "trauma to decay")
	await t.frames(2)
	t.check_eq(camera.offset, Vector2.ZERO, "camera offset back to rest")
	t.check_eq(camera.rotation, 0.0, "camera roll back to rest")


func _hitstop(t: E2EContext, fx: Node) -> void:
	await t.seconds(0.3)  # clear any cooldown from earlier hits
	Engine.time_scale = 2.0
	fx.hitstop(0.08)
	t.check(Engine.time_scale < 0.5, "hitstop dips time scale")
	await t.wait_until(func() -> bool: return not fx.is_hitstop_active(), 1.0, "hitstop to end")
	t.check_near(Engine.time_scale, 2.0, 0.0001, "hitstop restores the previous time scale")

	# Spamming hitstop never leaves the game slowed.
	var started := Time.get_ticks_msec()
	while Time.get_ticks_msec() - started < 600:
		fx.hitstop(0.1)
		await t.tree.process_frame
	await t.wait_until(func() -> bool: return not fx.is_hitstop_active(), 1.0, "spam to end")
	t.check_near(Engine.time_scale, 2.0, 0.0001, "time scale restored after repeated hitstops")

	# Pausing mid-stop restores the scale at once.
	await t.seconds(0.3)
	fx.hitstop(0.1)
	t.tree.paused = true
	await t.frames(2)
	t.check(not fx.is_hitstop_active(), "pause cancels hitstop")
	t.check_near(Engine.time_scale, 2.0, 0.0001, "pause restores time scale")
	t.tree.paused = false

	# Someone else changing the time scale wins.
	await t.seconds(0.3)
	fx.hitstop(0.1)
	Engine.time_scale = 3.0
	await t.seconds(0.2)
	t.check_near(Engine.time_scale, 3.0, 0.0001, "external time scale change is kept")
	Engine.time_scale = 1.0


func _arena(t: E2EContext, manager: Node) -> void:
	var arena: Node = manager.get_node_or_null("Arena")
	if not t.check(arena != null, "level has an Arena"):
		return
	var director: Node = manager.get_node("LevelDirector")
	t.check_eq(
		arena.get_accent_color(),
		director.get_level_info()["accent_color"],
		"arena uses the level accent"
	)
	director.arena_pulse.emit(0.0, 1.0)
	t.check(arena.get_bar_pulse() >= 0.99, "arena_pulse kicks the arena")
	await t.wait_until(
		func() -> bool: return arena.get_bar_pulse() < 0.05, 3.0, "arena pulse to decay"
	)
	var before: float = arena.get_energy()
	director.palette_shift.emit("main", 1.0)
	await t.seconds(1.5)
	t.check(arena.get_energy() > before, "palette_shift to main raises arena energy")


func _effects(t: E2EContext, fx: Node, manager: Node) -> void:
	var center := Vector2(640, 360)
	fx.burst(center, Color.RED, 12)
	for style in [Fx.BURST_SPARKS, Fx.BURST_DOTS, Fx.BURST_SHARDS]:
		fx.burst_style(center, Color.CYAN, 8, style)
	fx.flash(Color.WHITE, 0.2)
	fx.ring(center, Color.WHITE, 80.0, 0.3)
	fx.sweep(Color.WHITE, 0.4)
	fx.rewind_effect(0.3)
	t.check(fx.play_sfx("checkpoint"), "play_sfx plays a known sound")
	t.check(not fx.play_sfx("checkpoint"), "an immediate repeat is throttled")
	var director: Node = manager.get_node("LevelDirector")
	director.checkpoint_reached.emit(1, 0.0)
	director.rewound.emit(0.0)
	director.flash.emit(Color.WHITE, 0.1)
	director.camera_kick.emit(1.0)
	await t.seconds(0.6)


func _enemies(t: E2EContext, manager: Node) -> void:
	var enemies: Array[Node2D] = []
	for i in ENEMY_SCENES.size():
		var enemy: Node2D = load(ENEMY_SCENES[i]).instantiate()
		enemy.position = Vector2(200 + i * 200, 200)
		manager.add_child(enemy)
		enemies.append(enemy)
		t.check(enemy.get_node_or_null("EnemyVisual") != null, "%s has EnemyVisual" % enemy.name)
	await t.seconds(0.5)
	# Break every shield layer and kill each enemy with its matching colors.
	for enemy in enemies:
		var health: Node = enemy.get_node("HealthComponent")
		for i in 8:
			if not is_instance_valid(enemy):
				break
			enemy.take_damage(1000.0, health.get_active_color())
			await t.tree.process_frame
		t.check(not is_instance_valid(enemy), "enemy died")
	await t.seconds(0.5)


func _player_hit(t: E2EContext) -> void:
	var player: Node = t.players()[0]
	player.god_mode = false
	player.take_damage(1.0)
	await t.frames(2)
	t.check(player.get_node("PlayerVisual").get_hit_flash() > 0.3, "player hit flashes")
	player.kill()
	await t.frames(2)
	player.revive()
	await t.seconds(0.4)
	t.check(not player.is_dead, "player revived")
