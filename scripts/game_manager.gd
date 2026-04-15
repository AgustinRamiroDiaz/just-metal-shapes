class_name GameManager
extends Node2D

@export var player_scene: PackedScene

var score: int = 0
var game_time: float = 0.0
var is_game_over: bool = false
var viewport_rect: Rect2

@onready var score_label: Label = $ScoreLabel
@onready var debug_label: Label = $DebugLabel
@onready var _spawner: EnemySpawner = $EnemySpawner
@onready var _midi_player: MidiPlayer = $MidiPlayer
@onready var _midi_synth: MidiSynth = $MidiSynth


func _ready() -> void:
	viewport_rect = get_viewport().get_visible_rect()
	player_scene = load("res://scenes/player.tscn")
	_spawner.enemy_died.connect(_on_enemy_died)
	_spawn_players()
	debug_label.visible = OS.is_debug_build()
	_midi_player.note.connect(_on_midi_note)
	_midi_player.play()


func _on_midi_note(event: Dictionary, _track: int) -> void:
	var note: int = event.get("note", 0)
	if event.get("subtype") == MIDI_MESSAGE_NOTE_ON:
		_midi_synth.note_on(note)
	elif event.get("subtype") == MIDI_MESSAGE_NOTE_OFF:
		_midi_synth.note_off(note)


func _process(delta: float) -> void:
	if is_game_over:
		return
	game_time += delta
	_spawner.update_difficulty(game_time)
	_update_ui()


func _spawn_players() -> void:
	if GameConfig.players.is_empty():
		GameConfig.players = [
			GameConfig.PlayerConfig.new(
				GameConfig.InputType.GAMEPAD_LEFT_0, GameConfig.PLAYER_COLORS[0]
			),
			GameConfig.PlayerConfig.new(
				GameConfig.InputType.GAMEPAD_RIGHT_0, GameConfig.PLAYER_COLORS[1]
			),
		]

	var r := viewport_rect
	var spawn_positions := [
		r.position + r.size * Vector2(0.333, 0.390),
		r.position + r.size * Vector2(0.333, 0.612),
		r.position + r.size * Vector2(0.667, 0.390),
		r.position + r.size * Vector2(0.667, 0.612),
		r.position + r.size * Vector2(0.500, 0.260),
		r.position + r.size * Vector2(0.500, 0.703),
		r.position + r.size * Vector2(0.167, 0.502),
		r.position + r.size * Vector2(0.833, 0.502),
	]
	var spawn_index := 0
	for cfg in GameConfig.players:
		var p: CharacterBody2D = player_scene.instantiate()
		p.position = spawn_positions[spawn_index]
		spawn_index += 1
		p.team_color = cfg.color
		p.input_type = cfg.input_type
		if cfg.input_type == GameConfig.InputType.KEYBOARD1:
			p.move_left_action = &"p1_left"
			p.move_right_action = &"p1_right"
			p.move_up_action = &"p1_up"
			p.move_down_action = &"p1_down"
		elif cfg.input_type == GameConfig.InputType.KEYBOARD2:
			p.move_left_action = &"p2_left"
			p.move_right_action = &"p2_right"
			p.move_up_action = &"p2_up"
			p.move_down_action = &"p2_down"
		add_child(p)
		p.died.connect(_on_player_died)


func _on_enemy_died() -> void:
	score += 10


func _on_player_died() -> void:
	var players := get_tree().get_nodes_in_group("players")
	var alive_count := 0
	for p in players:
		if is_instance_valid(p) and not p.is_dead:
			alive_count += 1

	if alive_count == 0:
		_game_over()


func _game_over() -> void:
	is_game_over = true
	_spawner.stop()
	_show_game_over_screen()


func _show_game_over_screen() -> void:
	var canvas := CanvasLayer.new()
	add_child(canvas)

	var overlay := ColorRect.new()
	overlay.color = Color(0.0, 0.0, 0.0, 0.75)
	overlay.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	canvas.add_child(overlay)

	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	canvas.add_child(center)

	var vbox := VBoxContainer.new()
	vbox.alignment = BoxContainer.ALIGNMENT_CENTER
	center.add_child(vbox)

	var title := Label.new()
	title.text = "GAME OVER"
	title.add_theme_color_override("font_color", Color(1.0, 0.2, 0.2, 1.0))
	title.add_theme_font_size_override("font_size", 64)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	vbox.add_child(title)

	var score_lbl := Label.new()
	score_lbl.text = "Score: %d" % score
	score_lbl.add_theme_font_size_override("font_size", 32)
	score_lbl.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	vbox.add_child(score_lbl)

	var spacer := Control.new()
	spacer.custom_minimum_size = Vector2(0.0, 32.0)
	vbox.add_child(spacer)

	var restart_btn := Button.new()
	restart_btn.text = "Restart"
	restart_btn.add_theme_font_size_override("font_size", 24)
	restart_btn.pressed.connect(
		func() -> void: get_tree().change_scene_to_file("res://main_level.tscn")
	)
	vbox.add_child(restart_btn)

	var menu_btn := Button.new()
	menu_btn.text = "Main Menu"
	menu_btn.add_theme_font_size_override("font_size", 24)
	menu_btn.pressed.connect(
		func() -> void: get_tree().change_scene_to_file("res://scenes/main_menu.tscn")
	)
	vbox.add_child(menu_btn)

	restart_btn.call_deferred(&"grab_focus")


func _update_ui() -> void:
	score_label.text = "Score: %d" % score
	_update_debug_label()


func _update_debug_label() -> void:
	if not OS.is_debug_build():
		return
	var lines := PackedStringArray()
	lines.append("─── DEBUG ───")
	lines.append("time:       %6.1fs" % game_time)
	lines.append("difficulty: %6.2f" % _spawner.difficulty_factor)
	lines.append("")
	lines.append("spawn intervals:")
	for cfg in _spawner.enemy_configs:
		var enemy_name := cfg.scene.resource_path.get_file().get_basename()
		lines.append("  %-22s %5.2fs" % [enemy_name, cfg.spawn_interval])
	debug_label.text = "\n".join(lines)
