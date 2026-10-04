## Visual check for enemy kinds: in a level arena with two players and no chart events,
## spawns each enemy alone and saves one strip per kind (idle, wind-up, action), cropped
## around the enemy.
##
## Needs a renderer (not --headless); off-screen and silent:
##   xvfb-run -a -s "-screen 0 1280x720x24" \
##       godot --audio-driver Dummy --path godot \
##       -s res://tests/tools/enemy_shots.gd -- --out=/tmp/jms_shots/enemies [--only=hopper]
extends SceneTree

const KINDS := [
	"static_shooter",
	"shotgun",
	"turret",
	"runner",
	"mine_layer",
	"hopper",
	"pulser",
	"bouncer",
	"dasher",
	"lancer",
	"splitter",
	"splitter_mini",
	"chameleon",
	"warden",
]
const CENTER := Vector2(640, 360)
const CROP := Vector2i(440, 360)

var out_dir := "/tmp/jms_shots/enemies"
var only := ""
var manager: Node


func _initialize() -> void:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--out="):
			out_dir = arg.trim_prefix("--out=")
		elif arg.begins_with("--only="):
			only = arg.trim_prefix("--only=")
	DirAccess.make_dir_recursive_absolute(out_dir)
	_run()


func _run() -> void:
	await process_frame
	var config: Node = root.get_node("GameConfig")
	var colors: Array = GameConfig.get_player_colors()
	config.players.clear()
	for i in 2:
		config.players.append(PlayerConfig.new_config(2 + i, colors[i]))
	config.selected_level_id = "celtic"
	change_scene_to_file("res://main_level.tscn")
	await _frames(3)
	manager = current_scene
	var conductor: Node = manager.get_node("Conductor")
	var director: Node = manager.get_node("LevelDirector")
	conductor.use_clock = true
	manager.skip_countdown()
	await _seconds(1.5)
	director.set_process(false)
	director.clear_arena()
	var players := get_nodes_in_group("players")
	players[0].global_position = CENTER + Vector2(-150, 110)
	players[1].global_position = CENTER + Vector2(170, -60)
	for p in players:
		p.god_mode = true
		p.set_physics_process(false)

	for kind in KINDS:
		if only.is_empty() or kind == only:
			await _strip(kind)
	quit(0)


func _strip(kind: String) -> void:
	var enemy: Node2D = load("res://scenes/%s_enemy.tscn" % kind).instantiate()
	enemy.position = CENTER
	var health: Node = enemy.get_node("HealthComponent")
	health.max_life = 500.0
	manager.add_child(enemy)
	# A neighbor for the Warden to ward.
	var neighbor: Node2D = null
	if kind == "warden":
		neighbor = load("res://scenes/hopper_enemy.tscn").instantiate()
		neighbor.position = CENTER + Vector2(120, 40)
		manager.add_child(neighbor)
	var visual: Node = enemy.get_node("EnemyVisual")
	var acted := {"count": 0}
	for child in enemy.get_children():
		if child.has_signal("acted"):
			child.acted.connect(func(_a: float, _b: float) -> void: acted.count += 1)
	await _seconds(0.8)
	var idle := await _crop(enemy)
	await _wait(func() -> bool: return visual.get_windup() > 0.7, 8.0)
	var windup := await _crop(enemy)
	var seen: int = acted.count
	await _wait(func() -> bool: return acted.count > seen, 8.0)
	await _frames(4)
	var action := await _crop(enemy)
	var strip := Image.create(CROP.x * 3, CROP.y, false, idle.get_format())
	for i in 3:
		strip.blit_rect(
			[idle, windup, action][i], Rect2i(Vector2i.ZERO, CROP), Vector2i(CROP.x * i, 0)
		)
	var path := out_dir.path_join("%s.png" % kind)
	strip.save_png(path)
	print("shot: ", path)
	enemy.queue_free()
	if neighbor:
		neighbor.queue_free()
	for group in ["enemies", "enemy_projectiles", "mines"]:
		for node in get_nodes_in_group(group):
			node.queue_free()
	await _seconds(0.3)


func _crop(enemy: Node2D) -> Image:
	await RenderingServer.frame_post_draw
	var image := root.get_texture().get_image()
	var at := (
		Vector2i(enemy.global_position) - CROP / 2
		if is_instance_valid(enemy)
		else Vector2i(CENTER) - CROP / 2
	)
	at.x = clampi(at.x, 0, image.get_width() - CROP.x)
	at.y = clampi(at.y, 0, image.get_height() - CROP.y)
	return image.get_region(Rect2i(at, CROP))


func _wait(predicate: Callable, timeout: float) -> void:
	var start := Time.get_ticks_msec()
	while not predicate.call() and (Time.get_ticks_msec() - start) / 1000.0 < timeout:
		await process_frame


func _frames(count: int) -> void:
	for i in count:
		await process_frame


func _seconds(duration: float) -> void:
	await create_timer(duration, true, false, true).timeout
