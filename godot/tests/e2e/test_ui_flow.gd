## Title -> Level Select -> Lobby -> Level with injected keyboard input: join the
## keyboard, add a bot, hold Enter to start. Then Back from the lobby, and a
## bots-only start with Space.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 60.0


func run(t: E2EContext) -> void:
	var title := await t.change_scene(ProjectSettings.get_setting("application/run/main_scene"))
	if not t.check(title != null and title.is_class("TitleScreen"), "main scene is the title"):
		return
	var names: PackedStringArray = title.get_button_names()
	t.check(names.has("Play") and names.has("Settings") and names.has("Credits"), "title menu")
	await t.frames(3)

	# Play is focused by default.
	await _tap(t, KEY_ENTER)
	var select := await _wait_for_scene(t, "LevelSelect")
	if select == null:
		return
	var ids: PackedStringArray = select.get_level_ids()
	t.check(ids.size() >= 3, "level select lists every catalog level")
	t.check(select.is_card_unlocked(0), "first level unlocked")
	await t.frames(3)

	await _tap(t, KEY_ENTER)
	var lobby := await _wait_for_scene(t, "Lobby")
	if lobby == null:
		return
	t.check_eq(t.game_config().selected_level_id, ids[0], "selected level stored in GameConfig")
	t.check_eq(lobby.get_seat_count(), 0, "lobby starts empty")

	await _tap(t, KEY_ENTER)
	await _tap(t, KEY_B)
	await t.frames(2)
	t.check_eq(
		Array(lobby.get_seat_input_types()),
		[GameConfig.KEYBOARD1, GameConfig.BOT],
		"keyboard joined and a bot added"
	)
	await _tap(t, KEY_B)
	await _tap(t, KEY_BACKSPACE)
	await t.frames(2)
	t.check_eq(lobby.get_seat_count(), 2, "Backspace removes the extra bot")
	t.check_eq(lobby.get_bot_skill(), GameConfig.BOT_NORMAL, "bots default to normal")
	await _tap(t, KEY_TAB)
	t.check_eq(lobby.get_bot_skill(), GameConfig.BOT_HARD, "Tab cycles bot skill")

	t.press_key(KEY_ENTER, true)
	var level := await _wait_for_scene(t, "GameManager")
	t.press_key(KEY_ENTER, false)
	if level == null:
		return
	var input_types: Array = []
	for cfg in t.game_config().players:
		input_types.append(cfg.input_type)
	t.check_eq(input_types, [GameConfig.KEYBOARD1, GameConfig.BOT], "GameConfig players")
	var bot_cfg = t.game_config().players[1]
	t.check_eq(bot_cfg.bot_skill, GameConfig.BOT_HARD, "bot keeps the chosen skill")
	t.check_eq(String(bot_cfg.display_name), "BOT 1", "bot seat named")
	t.check_eq(t.players().size(), 2, "both seats spawned")
	t.check_eq(level.get_node("LevelDirector").get_level_id(), ids[0], "the chosen level loaded")
	t.check(level.get_node_or_null("LevelUi/Hud") != null, "HUD present")

	await _back_and_bots_only(t)


func _back_and_bots_only(t: E2EContext) -> void:
	t.game_config().players.clear()
	var lobby := await t.change_scene("res://scenes/ui/lobby.tscn")
	await t.frames(2)
	await _tap(t, KEY_ESCAPE)
	var select := await _wait_for_scene(t, "LevelSelect")
	t.check(select != null, "Esc in an empty lobby goes back to level select")
	await _tap(t, KEY_ESCAPE)
	var title := await _wait_for_scene(t, "TitleScreen")
	t.check(title != null, "Esc in level select goes back to the title")

	lobby = await t.change_scene("res://scenes/ui/lobby.tscn")
	await t.frames(2)
	await _tap(t, KEY_B)
	await _tap(t, KEY_B)
	await t.frames(2)
	t.check_eq(Array(lobby.get_seat_input_types()), [GameConfig.BOT, GameConfig.BOT], "two bots")
	t.press_key(KEY_SPACE, true)
	var level := await _wait_for_scene(t, "GameManager")
	t.press_key(KEY_SPACE, false)
	t.check(level != null, "a bots-only run starts")
	t.check_eq(t.players().size(), 2, "two bot players spawned")


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
