## Assertion and waiting helpers handed to every e2e scenario as `t`.
##
## Checks record failures and keep going (a failed check never stops the run), so a
## scenario reports every broken expectation at once.
extends RefCounted

var tree: SceneTree
var scenario_name: String
var failures: PackedStringArray = []
var checks := 0
## Set by the runner when `run()` returns.
var finished := false
## Set by the runner on timeout; later checks from the abandoned coroutine are ignored.
var aborted := false


func _init(scene_tree: SceneTree, name: String) -> void:
	tree = scene_tree
	scenario_name = name


func check(condition: bool, message: String) -> bool:
	if aborted:
		return condition
	checks += 1
	if not condition:
		failures.append(message)
	return condition


func check_eq(actual: Variant, expected: Variant, message: String) -> bool:
	return check(actual == expected, "%s: expected %s, got %s" % [message, expected, actual])


func check_near(actual: float, expected: float, tolerance: float, message: String) -> bool:
	return check(
		absf(actual - expected) <= tolerance,
		"%s: expected %.3f +/- %.3f, got %.3f" % [message, expected, tolerance, actual]
	)


func fail(message: String) -> void:
	check(false, message)


func note(message: String) -> void:
	print("    %s" % message)


func frames(count: int) -> void:
	for i in count:
		await tree.process_frame


## Waits real (unscaled) seconds.
func seconds(duration: float) -> void:
	await tree.create_timer(duration, true, false, true).timeout


## Polls `predicate` every frame until it returns true or `timeout` real seconds pass.
## Records a failure on timeout.
func wait_until(predicate: Callable, timeout: float, what: String) -> bool:
	var started := Time.get_ticks_msec()
	while not predicate.call():
		if (Time.get_ticks_msec() - started) / 1000.0 > timeout:
			fail("timed out waiting for %s" % what)
			return false
		await tree.process_frame
	return true


func change_scene(path: String) -> Node:
	var err := tree.change_scene_to_file(path)
	check_eq(err, OK, "change_scene_to_file(%s)" % path)
	await frames(2)
	return tree.current_scene


func game_config() -> Node:
	return tree.root.get_node("GameConfig")


func players() -> Array[Node]:
	return tree.get_nodes_in_group("players")


func press_key(keycode: Key, pressed: bool) -> void:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.physical_keycode = keycode
	event.pressed = pressed
	Input.parse_input_event(event)


## Loads main_level.tscn for `level_id` and returns the GameManager once the
## countdown is running. Players get `god_mode` unless `vulnerable`.
func start_level(level_id: String, mode: int, time_scale: float, vulnerable := false) -> Node:
	var config := game_config()
	config.selected_level_id = level_id
	config.difficulty_mode = mode
	Engine.time_scale = time_scale
	var manager := await change_scene("res://main_level.tscn")
	if not check(manager != null and manager.is_class("GameManager"), "main level loaded"):
		return null
	manager.get_node("Conductor").use_clock = true
	for player in players():
		player.god_mode = not vulnerable
	return manager
