extends Control

const SCENE_GAME := "res://main_level.tscn"
const HOLD_TIME: float = 1.0
const STICK_THRESHOLD: float = 0.5
const STICK_RESET: float = 0.3


class DeviceSlot:
	var display_name: String
	var is_keyboard: bool
	var device_index: int  # 0 for keyboard; gamepad index for gamepads
	var joined: bool = false
	var split: bool = false

	func get_input_types() -> Array:
		if is_keyboard:
			if split:
				return [GameConfig.InputType.KEYBOARD1, GameConfig.InputType.KEYBOARD2]
			return [GameConfig.InputType.KEYBOARD1]

		var left: int = GameConfig.InputType.GAMEPAD_LEFT_0 + device_index
		var right: int = GameConfig.InputType.GAMEPAD_RIGHT_0 + device_index
		if split:
			return [left, right]
		return [left]


var device_slots: Array = []
var device_rows: Array = []  # Array of {name_label, status_label}
var hold_timers: Dictionary = {}  # slot_idx -> float
var last_stick_dir: Dictionary = {}  # device_index -> int (-1, 0, 1)

@onready var stage1: Control = $Stage1
@onready var stage2: Control = $Stage2
@onready var device_list: VBoxContainer = $Stage1/VBox/DeviceList
@onready var player_list: VBoxContainer = $Stage2/VBox/PlayerList
@onready var start_button: Button = $Stage2/VBox/StartButton


func _ready() -> void:
	GameConfig.players.clear()
	start_button.focus_mode = Control.FOCUS_NONE
	start_button.pressed.connect(_on_start_pressed)
	Input.joy_connection_changed.connect(_on_joy_connection_changed)
	_rebuild_devices()
	stage1.visible = true
	stage2.visible = false


func _process(delta: float) -> void:
	if stage2.visible or hold_timers.is_empty():
		return
	var should_continue := false
	for idx in hold_timers.keys():
		hold_timers[idx] += delta
		if hold_timers[idx] >= HOLD_TIME:
			should_continue = true
	_refresh_stage1()
	if should_continue:
		hold_timers.clear()
		_on_continue_pressed()


func _rebuild_devices() -> void:
	for child in device_list.get_children():
		child.queue_free()
	device_slots.clear()
	device_rows.clear()

	# Keyboard is always available
	var kb := DeviceSlot.new()
	kb.display_name = "Keyboard"
	kb.is_keyboard = true
	kb.device_index = 0
	device_slots.append(kb)

	for idx in Input.get_connected_joypads():
		var gp := DeviceSlot.new()
		gp.display_name = "Gamepad %d" % (idx + 1)
		gp.is_keyboard = false
		gp.device_index = idx
		device_slots.append(gp)

	for slot in device_slots:
		var s: DeviceSlot = slot
		var row := HBoxContainer.new()
		row.custom_minimum_size = Vector2(420, 44)

		var name_lbl := Label.new()
		name_lbl.custom_minimum_size = Vector2(140, 0)
		name_lbl.text = s.display_name

		var hint_lbl := Label.new()
		hint_lbl.custom_minimum_size = Vector2(160, 0)
		hint_lbl.text = _hint_for(s)
		hint_lbl.modulate = Color(0.55, 0.55, 0.55)

		var status_lbl := Label.new()
		status_lbl.custom_minimum_size = Vector2(160, 0)
		status_lbl.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER

		row.add_child(name_lbl)
		row.add_child(hint_lbl)
		row.add_child(status_lbl)
		device_list.add_child(row)
		device_rows.append({"status": status_lbl})

	_refresh_stage1()


func _hint_for(slot: DeviceSlot) -> String:
	if slot.is_keyboard:
		return "Enter / ← →  / Hold Enter"
	return "A / D-Pad or L-Stick / Hold A"


func _on_joy_connection_changed(_device: int, _connected: bool) -> void:
	if stage1.visible:
		_rebuild_devices()


func _unhandled_input(event: InputEvent) -> void:
	if stage2.visible:
		return

	if event is InputEventKey and not event.echo:
		var idx := _keyboard_slot_index()
		if event.pressed:
			match event.physical_keycode:
				KEY_ENTER, KEY_KP_ENTER:
					_handle_join_down(idx)
				KEY_LEFT:
					_set_split(idx, false)
				KEY_RIGHT:
					_set_split(idx, true)
		else:
			match event.physical_keycode:
				KEY_ENTER, KEY_KP_ENTER:
					_handle_join_up(idx)

	elif event is InputEventJoypadButton:
		var idx := _gamepad_slot_index(event.device)
		if idx == -1:
			return
		if event.pressed:
			match event.button_index:
				JOY_BUTTON_A, JOY_BUTTON_START:
					_handle_join_down(idx)
				JOY_BUTTON_DPAD_LEFT:
					_set_split(idx, false)
				JOY_BUTTON_DPAD_RIGHT:
					_set_split(idx, true)
		else:
			match event.button_index:
				JOY_BUTTON_A, JOY_BUTTON_START:
					_handle_join_up(idx)

	elif event is InputEventJoypadMotion and event.axis == JOY_AXIS_LEFT_X:
		var idx := _gamepad_slot_index(event.device)
		if idx == -1:
			return
		var s: DeviceSlot = device_slots[idx]
		if not s.joined:
			return
		var prev_dir: int = last_stick_dir.get(event.device, 0)
		var new_dir: int = 0
		if event.axis_value > STICK_THRESHOLD:
			new_dir = 1
		elif event.axis_value < -STICK_THRESHOLD:
			new_dir = -1
		elif absf(event.axis_value) < STICK_RESET:
			new_dir = 0
		else:
			return  # in deadzone transition, ignore
		if new_dir != prev_dir:
			last_stick_dir[event.device] = new_dir
			if new_dir == 1:
				_set_split(idx, true)
			elif new_dir == -1:
				_set_split(idx, false)


func _handle_join_down(slot_idx: int) -> void:
	if slot_idx == -1:
		return
	var s: DeviceSlot = device_slots[slot_idx]
	if not s.joined:
		s.joined = true
		_refresh_stage1()
	else:
		hold_timers[slot_idx] = 0.0


func _handle_join_up(slot_idx: int) -> void:
	if slot_idx == -1 or not hold_timers.has(slot_idx):
		return
	var held: float = hold_timers[slot_idx]
	hold_timers.erase(slot_idx)
	if held < HOLD_TIME:
		var s: DeviceSlot = device_slots[slot_idx]
		s.joined = false
		s.split = false
	_refresh_stage1()


func _keyboard_slot_index() -> int:
	var i := 0
	for slot: DeviceSlot in device_slots:
		if slot.is_keyboard:
			return i
		i += 1
	return -1


func _gamepad_slot_index(device_index: int) -> int:
	var i := 0
	for slot: DeviceSlot in device_slots:
		if not slot.is_keyboard and slot.device_index == device_index:
			return i
		i += 1
	return -1


func _set_split(slot_idx: int, enable_split: bool) -> void:
	if slot_idx == -1:
		return
	var s: DeviceSlot = device_slots[slot_idx]
	if not s.joined:
		return
	s.split = enable_split
	_refresh_stage1()


func _refresh_stage1() -> void:
	var i := 0
	for slot: DeviceSlot in device_slots:
		var status_lbl: Label = device_rows[i]["status"]
		i += 1
		if hold_timers.has(i - 1):
			var progress := minf(hold_timers[i - 1] / HOLD_TIME, 1.0)
			var filled := roundi(progress * 8)
			status_lbl.text = "█".repeat(filled) + "░".repeat(8 - filled)
			status_lbl.modulate = Color(1.0, 0.88, 0.35)
		elif not slot.joined:
			status_lbl.text = "— not joined —"
			status_lbl.modulate = Color(0.45, 0.45, 0.45)
		elif slot.split:
			status_lbl.text = "◀ SPLIT (2P) ▶"
			status_lbl.modulate = Color(1.0, 0.88, 0.35)
		else:
			status_lbl.text = "◀ SINGLE ▶"
			status_lbl.modulate = Color(0.4, 0.95, 0.5)


func _on_continue_pressed() -> void:
	GameConfig.players.clear()
	var color_index := 0
	for slot in device_slots:
		var s: DeviceSlot = slot
		if not s.joined:
			continue
		for input_type in s.get_input_types():
			var color: Color = GameConfig.PLAYER_COLORS[
				color_index % GameConfig.PLAYER_COLORS.size()
			]
			GameConfig.players.append(GameConfig.PlayerConfig.new(input_type, color))
			color_index += 1
	_build_stage2()
	stage1.visible = false
	stage2.visible = true


func _build_stage2() -> void:
	for child in player_list.get_children():
		child.queue_free()
	var player_number := 1
	for cfg in GameConfig.players:
		var row := HBoxContainer.new()
		row.custom_minimum_size = Vector2(0, 36)

		var dot := ColorRect.new()
		dot.custom_minimum_size = Vector2(24, 24)
		dot.color = cfg.color

		var lbl := Label.new()
		lbl.text = "  Player %d  —  %s" % [player_number, GameConfig.INPUT_LABELS[cfg.input_type]]
		lbl.modulate = cfg.color

		row.add_child(dot)
		row.add_child(lbl)
		player_list.add_child(row)
		player_number += 1


func _on_start_pressed() -> void:
	get_tree().change_scene_to_file(SCENE_GAME)
