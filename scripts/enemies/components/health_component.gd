class_name HealthComponent
extends Node2D

signal damaged(amount: float)
signal died

const DAMAGE_FLASH_DURATION: float = 0.4
const PULSE_SPEED: float = 4.0
const GLOW_INTENSITY: float = 0.4

@export var max_life: float = 3.0
@export var shield_color: Color = Color(1.0, 0.35, 0.35, 1.0)
@export var has_shield: bool = true

var life: float = 0.0
var shield: float = 1.0
var damage_flash_timer: float = 0.0


func _ready() -> void:
	life = max_life
	if has_shield:
		var players := get_tree().get_nodes_in_group("players")
		if players.size() > 0:
			shield_color = players.pick_random().team_color
	queue_redraw()


func _process(delta: float) -> void:
	if damage_flash_timer > 0.0:
		damage_flash_timer -= delta
		queue_redraw()


func take_damage(amount: float, damage_color: Color = Color.WHITE) -> void:
	if life <= 0:
		return

	if has_shield and shield > 0.0 and _colors_match(shield_color, damage_color):
		shield = max(shield - amount, 0.0)
		damage_flash_timer = DAMAGE_FLASH_DURATION
		damaged.emit(amount)
		queue_redraw()
		return

	if has_shield and shield > 0.0:
		return

	life = max(life - amount, 0.0)
	damage_flash_timer = DAMAGE_FLASH_DURATION
	damaged.emit(amount)
	queue_redraw()
	if life == 0:
		died.emit()


func _colors_match(c1: Color, c2: Color) -> bool:
	return c1.r == c2.r and c1.g == c2.g and c1.b == c2.b


func _draw() -> void:
	var pulse := sin(Time.get_ticks_msec() / 1000.0 * PULSE_SPEED) * 0.5 + 0.5
	var base_alpha := 0.5 + pulse * GLOW_INTENSITY

	var bg_color := Color(0.2, 0.2, 0.2, base_alpha * 0.6)
	draw_arc(Vector2.ZERO, 34.0, 0.0, TAU, 64, bg_color, 6.0)

	var shield_alpha := 1.0
	var shield_width := 6.0
	if damage_flash_timer > 0.0:
		var flash_intensity := damage_flash_timer / DAMAGE_FLASH_DURATION
		shield_alpha = 1.0 + flash_intensity * 0.8
		shield_width = 6.0 + flash_intensity * 4.0

	var inner_color := Color(shield_color.r, shield_color.g, shield_color.b, base_alpha)
	var outer_color := Color(shield_color.r, shield_color.g, shield_color.b, shield_alpha)
	draw_arc(Vector2.ZERO, 34.0, -PI / 2.0, -PI / 2.0 + (TAU * shield), 64, inner_color, 6.0)
	draw_arc(
		Vector2.ZERO, 34.0, -PI / 2.0, -PI / 2.0 + (TAU * shield), 64, outer_color, shield_width
	)

	if shield > 0.0:
		var glow_color := Color(shield_color.r, shield_color.g, shield_color.b, base_alpha * 0.3)
		draw_arc(Vector2.ZERO, 38.0, -PI / 2.0, -PI / 2.0 + (TAU * shield), 64, glow_color, 2.0)

	var health_ratio := float(life) / float(max(max_life, 1))
	if health_ratio > 0.0:
		var health_color := Color(0.3, 0.8, 0.3, base_alpha)
		draw_arc(
			Vector2.ZERO, 28.0, -PI / 2.0, -PI / 2.0 + (TAU * health_ratio), 32, health_color, 4.0
		)
