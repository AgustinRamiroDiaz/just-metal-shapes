## Lives and names live on the players: no HUD panels; life pips and name tags show at
## the start and fade, pips come back on a hit and stay faint while hurt, the idle face
## follows the lives left, and a downed player's name shows until revived.
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 40.0
const FACES := "res://assets/kenney_shape-characters/PNG/Default/"


func run(t: E2EContext) -> void:
	var config := t.game_config()
	config.players.clear()
	config.add_human(GameConfig.KEYBOARD1)
	config.add_bot(GameConfig.BOT_NORMAL)
	var manager := await t.start_level(LevelCatalog.first_level_id(), GameConfig.NORMAL, 1.0, true)
	if manager == null:
		return
	manager.skip_countdown()
	t.check(
		manager.find_children("*", "PlayerPanel", true, false).is_empty(), "no HUD player panels"
	)
	var players := t.players()
	var player: Node = players[0]
	var visual: Node = player.get_node("PlayerVisual")
	var face: Sprite2D = player.get_node("FaceSprite")
	t.check_eq(String(visual.get_display_name()), "P1", "name from the seat")
	t.check_eq(String(players[1].get_node("PlayerVisual").get_display_name()), "BOT 1", "bot name")
	t.check_near(visual.get_name_alpha(), 1.0, 0.01, "name shown at the start")
	t.check_near(visual.get_life_pip_alpha(), 1.0, 0.01, "pips shown at the start")
	t.check(face.texture.resource_path.ends_with("face_f.png"), "full-health face")

	await t.seconds(3.0)
	t.check_near(visual.get_name_alpha(), 0.0, 0.01, "name fades")
	t.check_near(visual.get_life_pip_alpha(), 0.0, 0.01, "pips hide at full health")

	for p in players:
		p.god_mode = false
	player.take_damage(1.0)
	await t.frames(2)
	t.check_near(visual.get_life_pip_alpha(), 1.0, 0.01, "a hit shows the pips")
	t.check(face.texture.resource_path.ends_with("face_h.png"), "hurt face at 2 lives")
	await t.seconds(3.0)
	t.check(
		visual.get_life_pip_alpha() > 0.3 and visual.get_life_pip_alpha() < 0.6,
		"pips stay faint while hurt"
	)

	# `kill` ignores the post-hit invincibility.
	player.kill()
	await t.frames(2)
	t.check(player.is_dead, "downed")
	t.check_near(visual.get_name_alpha(), 1.0, 0.01, "downed player's name shows")
	player.revive()
	await t.frames(2)
	t.check(face.texture.resource_path.ends_with("face_i.png"), "last-life face after a revive")
	await t.seconds(3.0)
	t.check_near(visual.get_name_alpha(), 0.0, 0.01, "name hides after the revive")
