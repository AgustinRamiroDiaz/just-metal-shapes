## Clearing a level shows the results screen, records the best score and unlocks the
## next level; a hardcore wipe shows the game-over variant.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 40.0


func run(t: E2EContext) -> void:
	var save: Node = t.tree.root.get_node("SaveData")
	save.debug_unlocks_all = false
	var first_id: String = LevelCatalog.get_level(0).id
	var second_id: String = LevelCatalog.get_level(1).id
	t.check(not save.is_level_unlocked(second_id), "second level starts locked")

	var manager := await t.start_level(first_id, GameConfig.NORMAL, 4.0)
	if manager == null:
		return
	var conductor: Node = manager.get_node("Conductor")
	manager.skip_countdown()
	await t.frames(5)
	conductor.seek(conductor.get_duration() - 0.5)
	await t.wait_until(
		func() -> bool: return manager.get_node_or_null("EndScreen") != null,
		10.0,
		"results screen after level_cleared"
	)
	var results: Node = manager.get_node_or_null("EndScreen")
	if not t.check(results != null and results.is_class("ResultsScreen"), "results screen"):
		return
	t.check(results.cleared, "results show a clear")
	var buttons: PackedStringArray = results.get_button_names()
	t.check(buttons.has("Retry") and buttons.has("Level select"), "retry and level select")
	t.check(buttons.has("Next level"), "next level offered after the first clear")
	var record: Dictionary = save.get_record(first_id, GameConfig.NORMAL)
	t.check(record.best_score > 0, "best score recorded")
	t.check(record.best_rank != "", "best rank recorded")
	t.check(save.is_level_unlocked(second_id), "clearing level 1 unlocks level 2")
	t.check(results.outcome.get("new_best_score", false), "first clear is a new best")

	# Level select from the results.
	t.press_key(KEY_ESCAPE, true)
	t.press_key(KEY_ESCAPE, false)
	await t.wait_until(
		func() -> bool:
			var scene := t.tree.current_scene
			return scene != null and scene.is_class("LevelSelect"),
		10.0,
		"Esc on results goes to level select"
	)

	manager = await t.start_level(second_id, GameConfig.HARDCORE, 4.0, true)
	if manager == null:
		return
	manager.skip_countdown()
	await t.frames(5)
	for player in t.players():
		player.kill()
	await t.wait_until(
		func() -> bool: return manager.get_node_or_null("EndScreen") != null,
		5.0,
		"game over screen"
	)
	results = manager.get_node_or_null("EndScreen")
	if results != null:
		t.check(not results.cleared, "game over variant")
		t.check(not results.get_button_names().has("Next level"), "no next level after a loss")
