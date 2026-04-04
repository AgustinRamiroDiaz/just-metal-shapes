extends Control

const SCENE_GAME := "res://main_level.tscn"


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
		else:
			var left: int = GameConfig.InputType.GAMEPAD_LEFT_0 + device_index
			var right: int = GameConfig.InputType.GAMEPAD_RIGHT_0 + device_index
			if split:
				return [left, right]
			return [left]


var device_slots: Array = []
var device_rows: Array = []  # Array of {name_label, status_label}

@onready var stage1: Control = $Stage1
@onready var stage2: Control = $Stage2
@onready var device_list: VBoxContainer = $Stage1/VBox/DeviceList
@onready var continue_button: Button = $Stage1/VBox/ContinueButton
@onready var player_list: VBoxContainer = $Stage2/VBox/PlayerList
@onready var start_button: Button = $Stage2/VBox/StartButton


func _ready() -> void:
	GameConfig.players.clear()
	continue_button.focus_mode = Control.FOCUS_NONE
	start_button.focus_mode = Control.FOCUS_NONE
	continue_button.pressed.connect(_on_continue_pressed)
	start_button.pressed.connect(_on_start_pressed)
	Input.joy_connection_changed.connect(_on_joy_connection_changed)
	_rebuild_devices()
	stage1.visible = true
	stage2.visible = false


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
		return "Enter / ← →"
	return "A/Start / D-Pad ← →"


func _on_joy_connection_changed(_device: int, _connected: bool) -> void:
	if stage1.visible:
		_rebuild_devices()


func _unhandled_input(event: InputEvent) -> void:
	if stage2.visible:
		return

	if event is InputEventKey and event.pressed and not event.echo:
		var idx := _keyboard_slot_index()
		match event.physical_keycode:
			KEY_ENTER, KEY_KP_ENTER:
				_toggle_join(idx)
			KEY_LEFT:
				_set_split(idx, false)
			KEY_RIGHT:
				_set_split(idx, true)

	elif event is InputEventJoypadButton and event.pressed:
		var idx := _gamepad_slot_index(event.device)
		if idx == -1:
			return
		match event.button_index:
			JOY_BUTTON_A, JOY_BUTTON_START:
				_toggle_join(idx)
			JOY_BUTTON_DPAD_LEFT:
				_set_split(idx, false)
			JOY_BUTTON_DPAD_RIGHT:
				_set_split(idx, true)


func _keyboard_slot_index() -> int:
	for i in device_slots.size():
		if (device_slots[i] as DeviceSlot).is_keyboard:
			return i
	return -1


func _gamepad_slot_index(device_index: int) -> int:
	for i in device_slots.size():
		var s: DeviceSlot = device_slots[i]
		if not s.is_keyboard and s.device_index == device_index:
			return i
	return -1


func _toggle_join(slot_idx: int) -> void:
	if slot_idx == -1:
		return
	var s: DeviceSlot = device_slots[slot_idx]
	s.joined = not s.joined
	if not s.joined:
		s.split = false
	_refresh_stage1()


func _set_split(slot_idx: int, enable_split: bool) -> void:
	if slot_idx == -1:
		return
	var s: DeviceSlot = device_slots[slot_idx]
	if not s.joined:
		return
	s.split = enable_split
	_refresh_stage1()


func _refresh_stage1() -> void:
	for i in device_slots.size():
		var s: DeviceSlot = device_slots[i]
		var status_lbl: Label = device_rows[i]["status"]
		if not s.joined:
			status_lbl.text = "— not joined —"
			status_lbl.modulate = Color(0.45, 0.45, 0.45)
		elif s.split:
			status_lbl.text = "◀ SPLIT (2P) ▶"
			status_lbl.modulate = Color(1.0, 0.88, 0.35)
		else:
			status_lbl.text = "◀ SINGLE ▶"
			status_lbl.modulate = Color(0.4, 0.95, 0.5)

	var any_joined := false
	for slot in device_slots:
		if (slot as DeviceSlot).joined:
			any_joined = true
			break
	continue_button.disabled = not any_joined


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
	for i in GameConfig.players.size():
		var cfg: Variant = GameConfig.players[i]
		var row := HBoxContainer.new()
		row.custom_minimum_size = Vector2(0, 36)

		var dot := ColorRect.new()
		dot.custom_minimum_size = Vector2(24, 24)
		dot.color = cfg.color

		var lbl := Label.new()
		lbl.text = "  Player %d  —  %s" % [i + 1, GameConfig.INPUT_LABELS[cfg.input_type]]
		lbl.modulate = cfg.color

		row.add_child(dot)
		row.add_child(lbl)
		player_list.add_child(row)


func _on_start_pressed() -> void:
	get_tree().change_scene_to_file(SCENE_GAME)
