## Headless e2e runner.
##
## Discovers res://tests/e2e/test_*.gd, runs each scenario's `run(t)` coroutine with a
## timeout, prints PASS/FAIL per scenario and exits non-zero if any failed.
##
## Usage:
##   godot --headless --path godot -s res://tests/run_e2e.gd
##   godot --headless --path godot -s res://tests/run_e2e.gd -- --only=rewind
##
## A scenario is a script extending RefCounted with `func run(t: E2EContext) -> void`
## (it may await) and an optional `const TIMEOUT_SECONDS := <real seconds>`.
extends SceneTree

const E2EContext = preload("res://tests/e2e_context.gd")
const SCENARIO_DIR := "res://tests/e2e"
const DEFAULT_TIMEOUT := 60.0


func _initialize() -> void:
	_run_all()


func _run_all() -> void:
	await process_frame
	_ensure_autoloads()
	var only := _only_filter()
	var scenarios := _discover(only)
	if scenarios.is_empty():
		push_error("e2e: no scenarios matched '%s'" % only)
		quit(1)
		return

	var failed: PackedStringArray = []
	var started := Time.get_ticks_msec()
	for path in scenarios:
		var passed: bool = await _run_scenario(path)
		if not passed:
			failed.append(_scenario_name(path))

	var seconds := (Time.get_ticks_msec() - started) / 1000.0
	print("")
	print(
		(
			"e2e: %d passed, %d failed (%.1fs)"
			% [scenarios.size() - failed.size(), failed.size(), seconds]
		)
	)
	if not failed.is_empty():
		print("e2e: failed: %s" % ", ".join(failed))
	quit(1 if not failed.is_empty() else 0)


func _only_filter() -> String:
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--only="):
			return arg.trim_prefix("--only=")
	return ""


func _scenario_name(path: String) -> String:
	return path.get_file().get_basename().trim_prefix("test_")


func _discover(only: String) -> PackedStringArray:
	var found: PackedStringArray = []
	for file in DirAccess.get_files_at(SCENARIO_DIR):
		# Exported/imported projects may list compiled scripts as *.gdc or *.remap.
		var file_name := file.trim_suffix(".remap").trim_suffix(".gdc")
		if not file_name.begins_with("test_") or not file_name.ends_with(".gd"):
			continue
		var path := SCENARIO_DIR.path_join(file_name)
		if only.is_empty() or _scenario_name(path) == only:
			found.append(path)
	found.sort()
	return found


func _ensure_autoloads() -> void:
	if root.get_node_or_null("GameConfig") == null:
		var config: Node = load("res://scenes/autoload/game_config.tscn").instantiate()
		config.name = "GameConfig"
		root.add_child(config)


func _run_scenario(path: String) -> bool:
	var scenario_name := _scenario_name(path)
	print("RUN  %s" % scenario_name)
	var script: GDScript = load(path)
	if script == null:
		print("FAIL %s: could not load %s" % [scenario_name, path])
		return false
	var timeout: float = script.get_script_constant_map().get("TIMEOUT_SECONDS", DEFAULT_TIMEOUT)
	var scenario: RefCounted = script.new()
	var t := E2EContext.new(self, scenario_name)
	var started := Time.get_ticks_msec()

	_drive(scenario, t)
	while not t.finished and (Time.get_ticks_msec() - started) / 1000.0 < timeout:
		await process_frame
	if not t.finished:
		t.aborted = true
		t.fail("timed out after %.0fs" % timeout)

	await _reset()
	var seconds := (Time.get_ticks_msec() - started) / 1000.0
	if t.failures.is_empty():
		print("PASS %s (%d checks, %.1fs)" % [scenario_name, t.checks, seconds])
		return true
	for failure in t.failures:
		print("  - %s" % failure)
	print("FAIL %s (%.1fs)" % [scenario_name, seconds])
	return false


func _drive(scenario: RefCounted, t: E2EContext) -> void:
	await scenario.run(t)
	t.finished = true


func _reset() -> void:
	Engine.time_scale = 1.0
	paused = false
	if current_scene != null:
		unload_current_scene()
	var config := root.get_node_or_null("GameConfig")
	if config != null:
		config.players.clear()
		config.selected_level_id = ""
		config.difficulty_mode = GameConfig.NORMAL
	for child in root.get_children():
		if child != config:
			child.queue_free()
	await process_frame
	await process_frame
