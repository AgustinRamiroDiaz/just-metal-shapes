## Captures every UI screen to PNG for visual review.
##
## Needs a display (not --headless):
##   godot --path godot -s res://tests/tools/screenshots.gd -- --out=/tmp/jms_shots
## Uses a throwaway save (user://screenshot_save.json) seeded with a cleared first level.
extends SceneTree

const SAVE_PATH := "user://screenshot_save.json"

var out_dir := "/tmp/jms_shots"


func _initialize() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--out="):
			out_dir = arg.trim_prefix("--out=")
	DirAccess.make_dir_recursive_absolute(out_dir)
	_run()


func _run() -> void:
	await process_frame
	var save: Node = root.get_node("SaveData")
	save.save_path = SAVE_PATH
	save.reset_to_defaults()
	save.debug_unlocks_all = false
	save.record_result("wonders-of-the-earth", GameConfig.NORMAL, 18450, "A", true)
	save.record_result("celtic", GameConfig.NORMAL, 9120, "C", false)

	await _open("res://scenes/ui/title.tscn", 1.4)
	await _shot("01_title")

	await _open("res://scenes/ui/level_select.tscn", 1.0)
	await _shot("02_level_select")
	_key(KEY_RIGHT)
	await _wait(0.4)
	await _shot("03_level_select_focus2")
	_key(KEY_RIGHT)
	await _wait(0.4)
	await _shot("04_level_select_locked")

	var config: Node = root.get_node("GameConfig")
	config.selected_level_id = "celtic"
	config.difficulty_mode = GameConfig.NORMAL
	await _open("res://scenes/ui/lobby.tscn", 0.6)
	await _shot("05_lobby_empty")
	_key(KEY_ENTER)
	_key(KEY_RIGHT)
	_key(KEY_B)
	_key(KEY_B)
	await _wait(0.2)
	Input.parse_input_event(_key_event(KEY_SPACE, true))
	await _wait(0.45)
	await _shot("06_lobby_joined")
	Input.parse_input_event(_key_event(KEY_SPACE, false))
	await _wait(0.3)

	await _open("res://scenes/ui/settings.tscn", 0.6)
	await _shot("07_settings")

	await _open("res://scenes/ui/credits.tscn", 0.6)
	await _shot("08_credits")

	var manager := await _open("res://main_level.tscn", 0.3)
	await _shot("09_countdown")
	manager.skip_countdown()
	var conductor: Node = manager.get_node("Conductor")
	var director: Node = manager.get_node("LevelDirector")
	conductor.use_clock = true
	for player in get_nodes_in_group("players"):
		player.god_mode = true
	var checkpoints: PackedFloat64Array = director.get_checkpoint_beats()
	conductor.seek(conductor.beat_to_time(checkpoints[min(2, checkpoints.size() - 1)] - 2.0))
	await _wait(1.6)
	var players := get_nodes_in_group("players")
	if players.size() > 1:
		players[1].god_mode = false
		players[1].take_damage(1.0, Color.WHITE)
	if players.size() > 2:
		players[2].god_mode = false
		players[2].kill()
	await _wait(0.25)
	await _shot("10_hud")
	manager.rewound.emit(2, 0.0)
	await _wait(0.5)
	await _shot("11_rewind_toast")

	var pause := manager.get_node("LevelUi/PauseMenu")
	pause.open()
	await _wait(0.4)
	await _shot("12_pause")
	pause.call("_on_settings")
	await _wait(0.3)
	await _shot("13_pause_settings")
	_key(KEY_ESCAPE)
	await _wait(0.2)
	pause.resume()
	await _wait(0.6)
	await _shot("14_resume_countdown")
	await _wait(2.0)

	conductor.seek(conductor.get_duration() - 0.5)
	await _wait(1.0)
	await _wait(1.6)
	await _shot("15_results")

	config.difficulty_mode = GameConfig.HARDCORE
	manager = await _open("res://main_level.tscn", 0.2)
	manager.skip_countdown()
	await _wait(0.5)
	for player in get_nodes_in_group("players"):
		player.kill()
	await _wait(2.0)
	await _shot("16_game_over")

	print("screenshots: saved to %s" % out_dir)
	quit(0)


func _open(path: String, settle: float) -> Node:
	paused = false
	change_scene_to_file(path)
	await process_frame
	await process_frame
	await _wait(settle)
	return current_scene


func _wait(seconds: float) -> void:
	await create_timer(seconds, true, false, true).timeout


func _shot(name: String) -> void:
	await RenderingServer.frame_post_draw
	var image := root.get_viewport().get_texture().get_image()
	var path := out_dir.path_join(name + ".png")
	image.save_png(path)
	print("screenshots: %s" % path)


func _key_event(keycode: Key, pressed: bool) -> InputEventKey:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.physical_keycode = keycode
	event.pressed = pressed
	return event


func _key(keycode: Key) -> void:
	Input.parse_input_event(_key_event(keycode, true))
	Input.parse_input_event(_key_event(keycode, false))
