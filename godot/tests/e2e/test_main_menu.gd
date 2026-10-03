## Boot reaches the main menu; joining the keyboard in split mode and holding Enter
## starts the level with both keyboard players from GameConfig.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 30.0


func run(t: E2EContext) -> void:
	var menu := await t.change_scene(ProjectSettings.get_setting("application/run/main_scene"))
	if not t.check(menu != null and menu.is_class("MainMenu"), "main scene is MainMenu"):
		return
	var devices: Node = menu.get_node("CenterContainer/VBox/DeviceList")
	t.check(devices.get_child_count() >= 1, "device list shows the keyboard")
	t.check_eq(t.game_config().players.size(), 0, "menu clears GameConfig players")

	# Join, switch to split (2 players), then hold Enter to start.
	t.press_key(KEY_ENTER, true)
	t.press_key(KEY_ENTER, false)
	t.press_key(KEY_RIGHT, true)
	t.press_key(KEY_RIGHT, false)
	await t.frames(2)
	Engine.time_scale = 4.0
	t.press_key(KEY_ENTER, true)
	var menu_id := menu.get_instance_id()
	await t.wait_until(
		func() -> bool:
			var scene := t.tree.current_scene
			return scene != null and scene.get_instance_id() != menu_id,
		10.0,
		"hold Enter to start the level"
	)
	t.press_key(KEY_ENTER, false)
	await t.frames(3)

	var level := t.tree.current_scene
	t.check(level != null and level.is_class("GameManager"), "level scene loaded")
	var configs: Array = t.game_config().players
	t.check_eq(configs.size(), 2, "GameConfig has the two joined keyboard players")
	var input_types: Array = []
	for player in t.players():
		input_types.append(player.input_type)
	input_types.sort()
	t.check_eq(input_types, [GameConfig.KEYBOARD1, GameConfig.KEYBOARD2], "spawned player inputs")
