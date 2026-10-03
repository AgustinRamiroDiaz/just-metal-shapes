## Title -> Credits by keyboard; the credits carry the Kevin MacLeod CC BY attribution;
## Esc returns to the title.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 20.0


func run(t: E2EContext) -> void:
	await t.change_scene("res://scenes/ui/title.tscn")
	await t.frames(3)
	for key in [KEY_DOWN, KEY_DOWN, KEY_ENTER]:
		t.press_key(key, true)
		t.press_key(key, false)
		await t.frames(1)
	await t.wait_until(
		func() -> bool:
			var scene := t.tree.current_scene
			return scene != null and scene.is_class("CreditsScreen"),
		10.0,
		"credits screen"
	)
	var credits := t.tree.current_scene
	if credits == null or not credits.is_class("CreditsScreen"):
		return
	var text: String = credits.get_credits_text()
	t.check(text.contains("Kevin MacLeod"), "credits list Kevin MacLeod")
	t.check(text.contains("Licensed under Creative Commons: By Attribution 4.0"), "CC BY text")
	t.check(text.contains("Ouroboros") and text.contains("Voxel Revolution"), "both tracks")
	t.check(text.contains("Kenney"), "Kenney assets credited")

	await t.wait_until(func() -> bool: return not _transitioning(t), 5.0, "transition end")
	t.press_key(KEY_ESCAPE, true)
	t.press_key(KEY_ESCAPE, false)
	await t.wait_until(
		func() -> bool:
			var scene := t.tree.current_scene
			return scene != null and scene.is_class("TitleScreen"),
		10.0,
		"Esc back to the title"
	)


func _transitioning(t: E2EContext) -> bool:
	var ui := t.tree.root.get_node_or_null("Ui")
	return ui != null and ui.is_transitioning()
