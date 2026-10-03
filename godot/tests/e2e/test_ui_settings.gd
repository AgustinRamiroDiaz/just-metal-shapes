## Settings persist across a SaveData reload, apply to the audio buses, and the
## settings screen edits them with left/right.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 20.0


func run(t: E2EContext) -> void:
	var save: Node = t.tree.root.get_node("SaveData")
	save.music_volume = 0.3
	save.screen_shake = false
	save.latency_offset_ms = 45
	save.show_fps = true
	t.check(save.save(), "save writes")
	t.check(FileAccess.file_exists(save.save_path), "save file exists")

	save.music_volume = 1.0
	save.screen_shake = true
	save.latency_offset_ms = 0
	t.check(save.reload(), "reload parses the file")
	t.check_near(save.music_volume, 0.3, 0.001, "music volume restored")
	t.check_eq(save.screen_shake, false, "screen shake restored")
	t.check_eq(save.latency_offset_ms, 45, "latency restored")
	t.check_eq(save.show_fps, true, "show fps restored")
	var music_bus := AudioServer.get_bus_index("Music")
	t.check(music_bus >= 0, "Music bus exists")
	t.check(AudioServer.get_bus_index("SFX") >= 0, "SFX bus exists")
	t.check_near(
		AudioServer.get_bus_volume_db(music_bus), linear_to_db(0.3), 0.05, "Music bus volume"
	)

	# A corrupt file falls back to the backup written by the previous save.
	save.save()
	var file := FileAccess.open(save.save_path, FileAccess.WRITE)
	file.store_string("{ not json")
	file.close()
	save.music_volume = 0.9
	save.reload()
	t.check_near(save.music_volume, 0.3, 0.001, "corrupt save recovers from the backup")

	var screen := await t.change_scene("res://scenes/ui/settings.tscn")
	await t.frames(3)
	var panel: Node = screen.find_child("SettingsPanel", true, false)
	if not t.check(panel != null, "settings panel"):
		return
	var row: Node = panel.get_row("master_volume")
	var before: float = save.master_volume
	row.grab_focus()
	await t.frames(1)
	t.press_key(KEY_LEFT, true)
	t.press_key(KEY_LEFT, false)
	await t.frames(2)
	t.check_near(save.master_volume, before - 0.05, 0.001, "Left lowers master volume")
	var toggle: Node = panel.get_row("screen_shake")
	toggle.grab_focus()
	await t.frames(1)
	t.press_key(KEY_ENTER, true)
	t.press_key(KEY_ENTER, false)
	await t.frames(2)
	t.check_eq(save.screen_shake, true, "Enter toggles screen shake")
