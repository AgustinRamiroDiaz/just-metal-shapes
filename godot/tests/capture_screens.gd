## Visual check: plays a level with four players and stages moments worth looking at
## (idle arena, shielded enemies under lightning, a player hit, a downed player being
## revived, an enemy death, a checkpoint sweep, a rewind), saving a PNG of each, and
## prints a frame-time sample while enemies, lightning and hazards are all on screen.
##
## Needs a display (not --headless):
##   godot --path godot -s res://tests/capture_screens.gd -- --out=/tmp/jms_shots \
##       [--level=celtic] [--players=8]
extends SceneTree

var out_dir := "/tmp/jms_shots"
var level_id := "wonders-of-the-earth"
var player_count := 4
var manager: Node


func _initialize() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--out="):
			out_dir = arg.trim_prefix("--out=")
		elif arg.begins_with("--level="):
			level_id = arg.trim_prefix("--level=")
		elif arg.begins_with("--players="):
			player_count = clampi(int(arg.trim_prefix("--players=")), 4, 8)
	DirAccess.make_dir_recursive_absolute(out_dir)
	_run()


func _run() -> void:
	await process_frame
	var config: Node = root.get_node("GameConfig")
	var colors: Array = GameConfig.get_player_colors()
	config.players.clear()
	for i in player_count:
		config.players.append(PlayerConfig.new_config(2 + i, colors[i]))
	config.selected_level_id = level_id
	change_scene_to_file("res://main_level.tscn")
	await _frames(3)
	manager = current_scene
	var conductor: Node = manager.get_node("Conductor")
	var director: Node = manager.get_node("LevelDirector")
	conductor.use_clock = true
	manager.skip_countdown()
	var players := get_nodes_in_group("players")
	for p in players:
		p.god_mode = true

	# Into the busiest section.
	conductor.seek(conductor.get_duration() * 0.45)
	await _seconds(1.5)
	await _shot("01_idle_arena")
	await _sample_perf(1.0)

	# Shielded enemies, one per type, with player colors as shields.
	var scenes := [
		"res://scenes/static_shooter_enemy.tscn",
		"res://scenes/shotgun_enemy.tscn",
		"res://scenes/turret_enemy.tscn",
		"res://scenes/runner_enemy.tscn",
		"res://scenes/mine_layer_enemy.tscn",
	]
	var spots := [
		Vector2(250, 200),
		Vector2(640, 160),
		Vector2(1030, 200),
		Vector2(330, 540),
		Vector2(950, 540)
	]
	var enemies: Array = []
	for i in scenes.size():
		var enemy: Node2D = load(scenes[i]).instantiate()
		var health: Node = enemy.get_node("HealthComponent")
		health.shield_colors = PackedColorArray(
			[colors[i % 4], colors[(i + 1) % 4], colors[(i + 2) % 4]]
		)
		health.max_life = 50.0
		enemy.position = spots[i]
		for child in enemy.get_children():
			if child.get_class() in ["ChaserComponent", "ColorChaserComponent"]:
				child.move_speed = 0.0
		manager.add_child(enemy)
		enemies.append(enemy)
	await _seconds(0.9)
	await _shot("02_enemies_spawned")

	# Player 0 lasers the static shooter (matching shield); player 1 hits the shotgun
	# enemy's wrong-color shield; player 2 lasers the turret.
	players[0].global_position = spots[0] + Vector2(70, 60)
	players[1].global_position = spots[0] + Vector2(-60, 90)
	players[2].global_position = spots[2] + Vector2(-90, 40)
	players[3].global_position = Vector2(640, 420)
	await _seconds(0.6)
	await _shot("03_lightning_and_deflect")
	await _seconds(1.2)
	await _shot("04_lightning_later")
	await _sample_perf(2.0)

	# Shield break: drain the static shooter's outer layer quickly.
	for i in 6:
		enemies[0].take_damage(0.25, colors[0])
	await _frames(2)
	await _shot("05_shield_break")

	# Player hit.
	players[3].god_mode = false
	players[3].take_damage(1.0)
	await _frames(2)
	await _shot("06_player_hit")
	await _seconds(0.25)
	await _shot("07_player_hit_after")

	# Downed and reviving.
	players[3].kill()
	await _frames(3)
	await _shot("08_player_down")
	players[2].global_position = players[3].global_position + Vector2(40, 10)
	await _seconds(1.0)
	await _shot("09_reviving")
	await _seconds(1.3)
	await _shot("10_revived")

	# Enemy death.
	var victim: Node2D = enemies[4]
	var victim_health: Node = victim.get_node("HealthComponent")
	for i in 8:
		if not is_instance_valid(victim):
			break
		victim.take_damage(1000.0, victim_health.get_active_color())
	await _frames(3)
	await _shot("11_enemy_death")

	# Checkpoint sweep.
	director.emit_signal("checkpoint_reached", 2, conductor.song_beat())
	await _seconds(0.3)
	await _shot("12_checkpoint_sweep")

	# Rewind.
	for p in players:
		p.god_mode = false
		p.kill()
	await _frames(4)
	await _shot("13_rewind")
	await _seconds(1.5)
	await _shot("14_after_rewind")
	quit(0)


func _sample_perf(duration: float) -> void:
	var frames := 0
	var worst := 0.0
	var process_total := 0.0
	var draw_calls := 0
	var started := Time.get_ticks_usec()
	var last := started
	while (Time.get_ticks_usec() - started) / 1e6 < duration:
		await process_frame
		var now := Time.get_ticks_usec()
		worst = maxf(worst, (now - last) / 1000.0)
		last = now
		frames += 1
		process_total += Performance.get_monitor(Performance.TIME_PROCESS)
		draw_calls = maxi(
			draw_calls, int(Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME))
		)
	var elapsed := (Time.get_ticks_usec() - started) / 1e6
	print(
		(
			(
				"perf: %d players, %.1f fps, worst frame %.1f ms, avg process %.2f ms,"
				+ " max draw calls %d, nodes %d"
			)
			% [
				player_count,
				frames / elapsed,
				worst,
				process_total / frames,
				draw_calls,
				Performance.get_monitor(Performance.OBJECT_NODE_COUNT),
			]
		)
	)


func _frames(count: int) -> void:
	for i in count:
		await process_frame


func _seconds(duration: float) -> void:
	await create_timer(duration, true, false, true).timeout


func _shot(shot_name: String) -> void:
	await RenderingServer.frame_post_draw
	var path := out_dir.path_join(shot_name + ".png")
	root.get_texture().get_image().save_png(path)
	print("shot: ", path)
