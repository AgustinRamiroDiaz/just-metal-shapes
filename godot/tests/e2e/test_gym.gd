## Enemy gym: Title -> Gym -> Lobby (Back returns to the title), then an endless level
## with no chart where enemy cards are dragged from the sidebar into the arena. Covers a
## real mouse drag, every card spawning its enemy, Clear, God mode, the song switch, the
## song looping, and a downed team respawning without a rewind.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 90.0


func run(t: E2EContext) -> void:
	var title := await t.change_scene(ProjectSettings.get_setting("application/run/main_scene"))
	if not t.check(title != null and title.is_class("TitleScreen"), "title loaded"):
		return
	t.check(title.get_button_names().has("Gym"), "title lists Gym")
	await t.frames(3)
	title.find_child("Gym", true, false).emit_signal("pressed")
	var lobby := await _wait_for_scene(t, "Lobby")
	if lobby == null:
		return
	t.check(t.game_config().gym, "Gym sets GameConfig.gym")
	await _tap(t, KEY_ESCAPE)
	t.check(await _wait_for_scene(t, "TitleScreen") != null, "Back from the gym lobby -> title")

	t.game_config().gym = true
	lobby = await t.change_scene("res://scenes/ui/lobby.tscn")
	await t.frames(2)
	await _tap(t, KEY_ENTER)
	await _tap(t, KEY_B)
	t.press_key(KEY_ENTER, true)
	var manager := await _wait_for_scene(t, "GameManager")
	t.press_key(KEY_ENTER, false)
	if not t.check(manager != null, "gym level starts from the lobby"):
		return
	manager.get_node("Conductor").use_clock = true
	await _check_level(t, manager)


func _check_level(t: E2EContext, manager: Node) -> void:
	var director: Node = manager.get_node("LevelDirector")
	var conductor: Node = manager.get_node("Conductor")
	t.check(manager.is_gym(), "manager runs as the gym")
	t.check_eq(manager.get_state(), "playing", "the gym skips the countdown")
	t.check(not director.active, "the chart is off")
	var panel: Node = manager.get_node_or_null("LevelUi/GymPanel")
	if not t.check(panel != null, "gym panel present"):
		return
	var kinds: Array = LevelCatalog.enemy_kinds()
	t.check_eq(panel.get_card_count(), kinds.size(), "one card per enemy type")
	await t.seconds(1.0)
	t.check_eq(director.get_cursor(), 0, "no chart events dispatched")
	t.check_eq(t.tree.get_nodes_in_group("hazards").size(), 0, "no hazards")

	# A real mouse drag from the Hopper card to a point in the arena.
	var hopper := kinds.map(func(k: Dictionary) -> String: return k.id).find("hopper")
	var card: Control = panel.get_card(hopper)
	var target := Vector2(300, 250)
	await _drag(t, card.get_global_rect().get_center(), target)
	var spawned := await t.wait_until(
		func() -> bool: return t.tree.get_nodes_in_group("enemies").size() == 1,
		5.0,
		"dragged enemy"
	)
	if spawned:
		var enemy: Node2D = t.tree.get_nodes_in_group("enemies")[0]
		t.check(enemy.scene_file_path.ends_with("hopper_enemy.tscn"), "the dragged type spawned")

	manager.gym_clear()
	await t.frames(2)
	t.check_eq(t.tree.get_nodes_in_group("enemies").size(), 0, "Clear removes enemies")

	# Godot takes the drop point from the OS cursor, which a headless window does not
	# have, so placement is checked through the drop handler itself.
	t.check(panel.drop_enemy(hopper, target), "drop at a screen point")
	if await t.wait_until(
		func() -> bool: return t.tree.get_nodes_in_group("enemies").size() == 1,
		5.0,
		"dropped enemy"
	):
		var enemy: Node2D = t.tree.get_nodes_in_group("enemies")[0]
		t.check(enemy.global_position.distance_to(target) < 40.0, "spawned at the drop point")
	manager.gym_clear()
	await t.frames(2)

	# Every card spawns its own scene.
	for i in kinds.size():
		t.check(
			panel.drop_enemy(i, Vector2(200 + 60 * (i % 12), 200 + 30 * i)), "drop %s" % kinds[i].id
		)
	await t.wait_until(
		func() -> bool: return t.tree.get_nodes_in_group("enemies").size() >= kinds.size(),
		5.0,
		"every enemy type"
	)
	var scenes := {}
	for enemy in t.tree.get_nodes_in_group("enemies"):
		scenes[enemy.scene_file_path] = true
	for kind in kinds:
		t.check(scenes.has(kind.scene), "%s spawned" % kind.id)
	manager.gym_clear()

	panel.set_god_mode(true)
	t.check(t.players().all(func(p: Node) -> bool: return p.god_mode), "God mode on")
	panel.set_god_mode(false)
	t.check(t.players().all(func(p: Node) -> bool: return not p.god_mode), "God mode off")

	# A downed team respawns in place.
	for player in t.players():
		player.god_mode = false
		player.take_damage(99.0)
	await t.wait_until(
		func() -> bool: return t.players().all(func(p: Node) -> bool: return not p.is_dead),
		3.0,
		"respawn"
	)
	t.check_eq(manager.get_rewinds(), 0, "no rewind in the gym")

	# The song loops instead of ending the level.
	conductor.seek(conductor.get_duration() - 0.3)
	await t.seconds(1.0)
	t.check(conductor.is_playing(), "the song loops")
	t.check(conductor.song_time() < 5.0, "the loop restarts the song")
	t.check_eq(manager.get_state(), "playing", "the gym never ends")
	t.check(manager.get_node_or_null("EndScreen") == null, "no results screen")

	var before: String = director.get_level_id()
	panel.next_song()
	await t.frames(2)
	t.check(director.get_level_id() != before, "the song switch loads another song")
	t.check(not director.active, "the chart stays off after a switch")
	t.check(conductor.is_playing(), "the new song plays")

	panel.toggle_sidebar()
	t.check(not panel.is_sidebar_visible(), "Hide collapses the sidebar")
	panel.toggle_sidebar()
	t.check(panel.is_sidebar_visible(), "and shows it again")


func _drag(t: E2EContext, from: Vector2, to: Vector2) -> void:
	# Headless windows never report the mouse entering, and the viewport ignores hover
	# until it does.
	t.tree.root.notification(Node.NOTIFICATION_VP_MOUSE_ENTER)
	_mouse_motion(t, from, 0)
	await t.frames(1)
	var press := InputEventMouseButton.new()
	press.button_index = MOUSE_BUTTON_LEFT
	press.button_mask = MOUSE_BUTTON_MASK_LEFT
	press.pressed = true
	press.position = from
	press.global_position = from
	t.tree.root.push_input(press, true)
	await t.frames(1)
	for i in range(1, 11):
		_mouse_motion(t, from.lerp(to, i / 10.0), MOUSE_BUTTON_MASK_LEFT, (to - from) / 10.0)
		await t.frames(1)
	var release := InputEventMouseButton.new()
	release.button_index = MOUSE_BUTTON_LEFT
	release.pressed = false
	release.position = to
	release.global_position = to
	t.tree.root.push_input(release, true)
	await t.frames(2)


func _mouse_motion(t: E2EContext, at: Vector2, mask: int, relative := Vector2.ZERO) -> void:
	var motion := InputEventMouseMotion.new()
	motion.relative = relative
	motion.position = at
	motion.global_position = at
	motion.button_mask = mask
	t.tree.root.push_input(motion, true)


func _tap(t: E2EContext, key: Key) -> void:
	t.press_key(key, true)
	await t.frames(1)
	t.press_key(key, false)
	await t.frames(1)


func _wait_for_scene(t: E2EContext, wanted_class: String) -> Node:
	var ok := await t.wait_until(
		func() -> bool:
			var scene := t.tree.current_scene
			return scene != null and scene.is_class(wanted_class) and not _transitioning(t),
		10.0,
		"scene %s" % wanted_class
	)
	await t.frames(2)
	return t.tree.current_scene if ok else null


func _transitioning(t: E2EContext) -> bool:
	var ui := t.tree.root.get_node_or_null("Ui")
	return ui != null and ui.is_transitioning()
