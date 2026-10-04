## Visual check for Las Huevas: plays the level with two god-mode bots, jumps to a few
## lyric cues and saves a frame just after each word lands (its enemy popping in, its
## caption up), plus one from a band jam.
##
## Needs a renderer (not --headless); off-screen and silent:
##   xvfb-run -a -s "-screen 0 1280x720x24" \
##       godot --audio-driver Dummy --path godot \
##       -s res://tests/tools/lyric_shots.gd -- --out=/tmp/jms_shots/lyrics
extends SceneTree

## Shot name -> song seconds to capture at (each cue time plus a beat or so).
const MOMENTS := {
	"01_patatas_jedi": 88.6,
	"02_caja_corazon": 112.4,
	"03_morfeo_matrix": 134.6,
	"04_yoda": 154.0,
	"05_jam_globo": 170.0,
	"06_diego_cielo": 309.2,
	"07_huevos": 313.2,
	"08_oveja_enjambre": 326.4,
	"09_mano_arriba": 330.0,
	"10_citricos": 410.6,
	"11_climas": 482.6,
}

var out_dir := "/tmp/jms_shots/lyrics"


func _initialize() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--out="):
			out_dir = arg.trim_prefix("--out=")
	DirAccess.make_dir_recursive_absolute(out_dir)
	_run()


func _run() -> void:
	await process_frame
	var config: Node = root.get_node("GameConfig")
	var colors: Array = GameConfig.get_player_colors()
	config.players.clear()
	for i in 2:
		config.players.append(
			PlayerConfig.new_bot(GameConfig.BOT_NORMAL, colors[i], "Bot %d" % (i + 1))
		)
	config.selected_level_id = "las-huevas"
	change_scene_to_file("res://main_level.tscn")
	await _frames(3)
	var manager := current_scene
	var conductor: Node = manager.get_node("Conductor")
	conductor.use_clock = true
	manager.skip_countdown()
	for p in get_nodes_in_group("players"):
		p.god_mode = true
	for shot in MOMENTS:
		var at: float = MOMENTS[shot]
		# Lead in by a few seconds so the cue's spawn effect and caption play out.
		conductor.seek(at - 4.0)
		await _seconds(4.0)
		await RenderingServer.frame_post_draw
		var path := out_dir.path_join(shot + ".png")
		root.get_texture().get_image().save_png(path)
		print("shot: ", path, " enemies=", get_nodes_in_group("enemies").size())
	quit(0)


func _frames(count: int) -> void:
	for i in count:
		await process_frame


func _seconds(duration: float) -> void:
	await create_timer(duration, true, false, true).timeout
