## Visual check for the enemy gym: two bots in the gym, a real mouse drag of a card into
## the arena (moving the OS cursor, so the drop point is the real one), then a few more
## enemies. Saves the sidebar, the drag in progress and the populated arena, and prints
## how far the dragged enemy landed from the drop point.
##
## Needs a renderer (not --headless); off-screen and silent:
##   xvfb-run -a -s "-screen 0 1280x720x24" \
##       godot --audio-driver Dummy --path godot \
##       -s res://tests/tools/gym_shots.gd -- --out=/tmp/jms_shots/gym
extends SceneTree

var out_dir := "/tmp/jms_shots/gym"


func _initialize() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--out="):
			out_dir = arg.trim_prefix("--out=")
	DirAccess.make_dir_recursive_absolute(out_dir)
	_run()


func _run() -> void:
	await process_frame
	var config: Node = root.get_node("GameConfig")
	config.gym = true
	config.selected_level_id = "celtic"
	config.players.clear()
	config.add_bot(GameConfig.BOT_NORMAL)
	config.add_bot(GameConfig.BOT_NORMAL)
	change_scene_to_file("res://main_level.tscn")
	await _frames(3)
	var manager := current_scene
	manager.get_node("Conductor").use_clock = true
	var panel: Node = manager.get_node("LevelUi/GymPanel")
	await _seconds(1.5)
	await _save("sidebar")

	var card: Control = panel.get_card(3)
	var from := card.get_global_rect().get_center()
	var to := Vector2(420, 300)
	Input.warp_mouse(from)
	await _frames(2)
	_button(from, true)
	await _frames(2)
	for i in range(1, 21):
		var at := from.lerp(to, i / 20.0)
		Input.warp_mouse(at)
		_motion(at, (to - from) / 20.0)
		await _frames(1)
	await _save("dragging")
	_button(to, false)
	await _seconds(1.2)
	var enemies := get_nodes_in_group("enemies")
	if enemies.is_empty():
		print("gym_shots: no enemy after the drop")
	else:
		var enemy: Node2D = enemies[0]
		print(
			(
				"gym_shots: dropped %s at %s, landed at %s (%.1f px off)"
				% [
					enemy.scene_file_path.get_file(),
					to,
					enemy.global_position,
					enemy.global_position.distance_to(to)
				]
			)
		)
	for i in [2, 3, 5, 11]:
		panel.drop_enemy(i, Vector2(220 + 160 * (i % 4), 420 + 40 * (i % 3)))
	await _seconds(3.0)
	await _save("arena")
	quit(0)


func _button(at: Vector2, pressed: bool) -> void:
	var event := InputEventMouseButton.new()
	event.button_index = MOUSE_BUTTON_LEFT
	event.button_mask = MOUSE_BUTTON_MASK_LEFT if pressed else 0
	event.pressed = pressed
	event.position = at
	event.global_position = at
	Input.parse_input_event(event)


func _motion(at: Vector2, relative: Vector2) -> void:
	var event := InputEventMouseMotion.new()
	event.position = at
	event.global_position = at
	event.relative = relative
	event.button_mask = MOUSE_BUTTON_MASK_LEFT
	Input.parse_input_event(event)


func _save(shot: String) -> void:
	await RenderingServer.frame_post_draw
	var path := out_dir.path_join("%s.png" % shot)
	root.get_texture().get_image().save_png(path)
	print("shot: ", path)


func _frames(count: int) -> void:
	for i in count:
		await process_frame


func _seconds(duration: float) -> void:
	await create_timer(duration, true, false, true).timeout
