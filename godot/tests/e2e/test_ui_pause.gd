## The pause action freezes the tree and Conductor.song_time; resuming counts three
## beats back in, then the song continues.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 40.0


func run(t: E2EContext) -> void:
	var manager := await t.start_level("celtic", GameConfig.NORMAL, 2.0)
	if manager == null:
		return
	var conductor: Node = manager.get_node("Conductor")
	var pause: Node = manager.get_node("LevelUi/PauseMenu")
	manager.skip_countdown()
	await t.wait_until(func() -> bool: return conductor.song_time() > 1.0, 10.0, "song to run")

	t.press_key(KEY_ESCAPE, true)
	t.press_key(KEY_ESCAPE, false)
	await t.frames(2)
	t.check_eq(pause.get_state(), "menu", "Esc opens the pause menu")
	t.check(t.tree.paused, "tree paused")
	t.check(conductor.is_paused(), "conductor paused")
	var frozen: float = conductor.song_time()
	await t.seconds(0.6)
	t.check_eq(conductor.song_time(), frozen, "song time frozen while paused")
	var focused := t.tree.root.get_viewport().gui_get_focus_owner()
	t.check(focused != null and focused.name == "Resume", "Resume has focus")

	# Settings inside pause, then back.
	pause.call("_on_settings")
	await t.frames(2)
	t.check_eq(pause.get_state(), "settings", "settings open from pause")
	t.press_key(KEY_ESCAPE, true)
	t.press_key(KEY_ESCAPE, false)
	await t.frames(2)
	t.check_eq(pause.get_state(), "menu", "Esc returns from settings to the pause menu")

	t.press_key(KEY_ESCAPE, true)
	t.press_key(KEY_ESCAPE, false)
	await t.frames(2)
	t.check_eq(pause.get_state(), "resuming", "Esc again starts the resume countdown")
	t.check(t.tree.paused, "still paused during the countdown")
	t.check_eq(conductor.song_time(), frozen, "song time frozen during the countdown")
	await t.wait_until(
		func() -> bool: return pause.get_state() == "running", 10.0, "resume countdown"
	)
	t.check(not t.tree.paused, "tree unpaused")
	t.check(conductor.is_playing(), "conductor playing")
	await t.wait_until(
		func() -> bool: return conductor.song_time() > frozen + 0.3, 5.0, "song time to advance"
	)
