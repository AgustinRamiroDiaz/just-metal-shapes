## Gamepad navigation on Level Select: B goes back, a held stick moves the card
## carousel and the mode row one step per push, and the focused card stays on screen.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 40.0


func run(t: E2EContext) -> void:
	var select := await _open_level_select(t)
	if select == null:
		return

	# B (east button) goes back to the title.
	t.press_pad_button(JOY_BUTTON_B, true)
	await t.frames(1)
	t.press_pad_button(JOY_BUTTON_B, false)
	var title := await _wait_for_scene(t, "TitleScreen")
	t.check(title != null, "B on level select returns to the title")

	select = await _open_level_select(t)
	if select == null:
		return
	var ids: PackedStringArray = select.get_level_ids()

	# A held stick moves one card per push, like the d-pad.
	t.check_eq(select.get_focused_card(), 0, "first card focused")
	await t.hold_pad_axis(JOY_AXIS_LEFT_X, 0.9, 12)
	t.check_eq(select.get_focused_card(), 1, "one stick push moves one card")
	await t.hold_pad_axis(JOY_AXIS_LEFT_X, 0.9, 12)
	t.check_eq(select.get_focused_card(), 2, "a second push moves one more card")

	# Every card can be reached and the focused one is fully on screen.
	for i in ids.size():
		await t.hold_pad_axis(JOY_AXIS_LEFT_X, 0.9, 4)
	await t.frames(30)
	var viewport_width: float = t.tree.root.get_visible_rect().size.x
	for i in ids.size():
		await t.hold_pad_axis(JOY_AXIS_LEFT_X, -0.9, 4)
		await t.frames(30)
		var rect: Rect2 = select.get_focused_card_rect()
		t.check(
			rect.position.x >= 0.0 and rect.end.x <= viewport_width,
			"card %d fully visible (%s)" % [select.get_focused_card(), rect]
		)

	# Down to the mode row; one stick push changes the mode by exactly one step.
	await t.hold_pad_axis(JOY_AXIS_LEFT_Y, 0.9, 6)
	var mode_row: Control = select.get_mode_row()
	t.check(mode_row.has_focus(), "stick down focuses the mode row")
	var before: int = mode_row.mode
	await t.hold_pad_axis(JOY_AXIS_LEFT_X, 0.9, 20)
	t.check_eq(mode_row.mode, (before + 1) % 3, "one stick push changes the mode once")
	await t.hold_pad_axis(JOY_AXIS_LEFT_X, -0.9, 20)
	t.check_eq(mode_row.mode, before, "pushing back restores the mode")

	# The d-pad still steps the mode.
	t.press_pad_button(JOY_BUTTON_DPAD_RIGHT, true)
	await t.frames(1)
	t.press_pad_button(JOY_BUTTON_DPAD_RIGHT, false)
	await t.frames(2)
	t.check_eq(mode_row.mode, (before + 1) % 3, "d-pad right changes the mode")


func _open_level_select(t: E2EContext) -> Node:
	await t.change_scene("res://scenes/ui/level_select.tscn")
	return await _wait_for_scene(t, "LevelSelect")


func _wait_for_scene(t: E2EContext, wanted_class: String) -> Node:
	var ok := await t.wait_until(
		func() -> bool:
			var scene := t.tree.current_scene
			return scene != null and scene.is_class(wanted_class) and not _transitioning(t),
		10.0,
		"scene %s" % wanted_class
	)
	await t.frames(3)
	return t.tree.current_scene if ok else null


func _transitioning(t: E2EContext) -> bool:
	var ui := t.tree.root.get_node_or_null("Ui")
	return ui != null and ui.is_transitioning()
