extends StaticBody2D

@export var max_life: float = 3.0
@export var shield_color: Color = Color(1.0, 0.35, 0.35, 1.0)

@onready var sprite: Sprite2D = $Sprite2D
@onready var life_label: Label = $LifeLabel

var life: float = 0.0
var damage_flash_timer: float = 0.0

const DAMAGE_FLASH_DURATION: float = 0.4
const PULSE_SPEED: float = 4.0
const GLOW_INTENSITY: float = 0.4


func _ready() -> void:
	life = max_life
	_update_life_label()
	queue_redraw()


func _process(delta: float) -> void:
	if damage_flash_timer > 0.0:
		damage_flash_timer -= delta
		queue_redraw()


func take_damage(amount: float) -> void:
	if life <= 0:
		return

	life = max(life - amount, 0)
	damage_flash_timer = DAMAGE_FLASH_DURATION
	_update_life_label()
	queue_redraw()

	if life == 0:
		sprite.modulate = Color(0.35, 0.35, 0.35, 1.0)
		life_label.text = "Defeated"


func _draw() -> void:
	var ratio := float(life) / float(max(max_life, 1))
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
	draw_arc(Vector2.ZERO, 34.0, -PI / 2.0, -PI / 2.0 + (TAU * ratio), 64, inner_color, 6.0)
	draw_arc(Vector2.ZERO, 34.0, -PI / 2.0, -PI / 2.0 + (TAU * ratio), 64, outer_color, shield_width)

	if ratio > 0.0:
		var glow_color := Color(shield_color.r, shield_color.g, shield_color.b, base_alpha * 0.3)
		draw_arc(Vector2.ZERO, 38.0, -PI / 2.0, -PI / 2.0 + (TAU * ratio), 64, glow_color, 2.0)


func _update_life_label() -> void:
	life_label.text = "Life: %.1f" % life
