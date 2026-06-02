class_name HealthComponent
extends Node2D

signal damaged(amount: float)
signal died

const DAMAGE_FLASH_DURATION: float = 0.4
const PULSE_SPEED: float = 4.0
const GLOW_INTENSITY: float = 0.4
const SHIELD_BASE_RADIUS: float = 34.0
const SHIELD_LAYER_SPACING: float = 8.0

@export var max_life: float = 3.0
@export var shield_colors: PackedColorArray = PackedColorArray()
@export var auto_shield_layers: int = 0

var life: float = 0.0
var shield_fills: Array[float] = []
var damage_flash_timer: float = 0.0


func _ready() -> void:
	life = max_life
	_init_shields()
	queue_redraw()


func _init_shields() -> void:
	shield_fills.clear()
	if auto_shield_layers > 0 and shield_colors.is_empty():
		var players := get_tree().get_nodes_in_group("players")
		if players.size() > 0:
			var colors := PackedColorArray()
			for _i in auto_shield_layers:
				colors.append(players.pick_random().team_color)
			shield_colors = colors
	for _i in shield_colors.size():
		shield_fills.append(1.0)


func _process(delta: float) -> void:
	if damage_flash_timer > 0.0:
		damage_flash_timer -= delta
		queue_redraw()


func get_active_layer() -> int:
	for i in shield_fills.size():
		if shield_fills[i] > 0.0:
			return i
	return -1


func get_active_color() -> Color:
	var idx := get_active_layer()
	if idx >= 0:
		return shield_colors[idx]
	return Color.WHITE


func _get_shield_radius(layer_idx: int) -> float:
	var n := shield_fills.size()
	return SHIELD_BASE_RADIUS + (n - 1 - layer_idx) * SHIELD_LAYER_SPACING


func take_damage(amount: float, damage_color: Color = Color.WHITE) -> bool:
	if life <= 0:
		return false

	var active := get_active_layer()

	if active >= 0:
		if ColorUtils.colors_match(shield_colors[active], damage_color):
			shield_fills[active] = max(shield_fills[active] - amount, 0.0)
			damage_flash_timer = DAMAGE_FLASH_DURATION
			damaged.emit(amount)
			queue_redraw()
			return true
		return false

	life = max(life - amount, 0.0)
	damage_flash_timer = DAMAGE_FLASH_DURATION
	damaged.emit(amount)
	queue_redraw()
	if life == 0.0:
		died.emit()
	return true


func _draw() -> void:
	var pulse := sin(Time.get_ticks_msec() / 1000.0 * PULSE_SPEED) * 0.5 + 0.5
	var base_alpha := 0.5 + pulse * GLOW_INTENSITY

	var bg_color := Color(0.2, 0.2, 0.2, base_alpha * 0.6)
	draw_arc(Vector2.ZERO, SHIELD_BASE_RADIUS, 0.0, TAU, 64, bg_color, 6.0)

	var active := get_active_layer()

	for i in range(shield_fills.size() - 1, -1, -1):
		if shield_fills[i] <= 0.0:
			continue
		var radius := _get_shield_radius(i)
		var color := shield_colors[i]

		var shield_alpha := 1.0
		var shield_width := 6.0
		if i == active and damage_flash_timer > 0.0:
			var t := damage_flash_timer / DAMAGE_FLASH_DURATION
			shield_alpha = 1.0 + t * 0.8
			shield_width = 6.0 + t * 4.0

		var inner_color := Color(color.r, color.g, color.b, base_alpha)
		var outer_color := Color(color.r, color.g, color.b, shield_alpha)
		draw_arc(
			Vector2.ZERO, radius, -PI / 2.0, -PI / 2.0 + TAU * shield_fills[i], 64, inner_color, 6.0
		)
		draw_arc(
			Vector2.ZERO,
			radius,
			-PI / 2.0,
			-PI / 2.0 + TAU * shield_fills[i],
			64,
			outer_color,
			shield_width
		)

		if i == active:
			var glow_color := Color(color.r, color.g, color.b, base_alpha * 0.3)
			draw_arc(
				Vector2.ZERO,
				radius + 4.0,
				-PI / 2.0,
				-PI / 2.0 + TAU * shield_fills[i],
				64,
				glow_color,
				2.0
			)

	var health_ratio := life / maxf(max_life, 1.0)
	if health_ratio > 0.0:
		var health_color := Color(0.3, 0.8, 0.3, base_alpha)
		draw_arc(
			Vector2.ZERO, 28.0, -PI / 2.0, -PI / 2.0 + TAU * health_ratio, 32, health_color, 4.0
		)
