## Every enemy scene, spawned standalone against a clock-driven Conductor (120 BPM):
## - it spawns, joins `enemies` and `danger`, and has a HealthComponent and EnemyVisual;
## - each beat-locked component acts on its cadence beats (within tolerance), after a
##   wind-up, and attacks report pending danger shapes before they land;
## - the wrong color cannot hurt it; the matching colors kill it;
## - it frees cleanly (and a Splitter or Gota leaves single-shield pieces).
## Plus the Warden's ward, the Chameleon's color cycle, the fuses (Huevo hatches chicks,
## Limón bursts) and lifetimes (an enemy leaves on time without counting as a kill).
extends RefCounted

const E2EContext = preload("res://tests/e2e_context.gd")
const TIMEOUT_SECONDS := 150.0
const TIME_SCALE := 2.0
const BPM := 120.0
## Beats an action may land late (one frame at this time scale is ~0.07 beats).
const BEAT_TOLERANCE := 0.15
const RECORD_LEN := 10
const COLORS := [Color(0.25, 0.6, 1.0), Color(1.0, 0.35, 0.35)]
const WRONG_COLOR := Color(0.123, 0.456, 0.789)

## Scene -> beats to watch it, and whether it has a telegraphed attack shape.
const CASES := {
	"res://scenes/static_shooter_enemy.tscn": {"beats": 10.0, "telegraph": true},
	"res://scenes/shotgun_enemy.tscn": {"beats": 12.0, "telegraph": true},
	"res://scenes/turret_enemy.tscn": {"beats": 8.0, "telegraph": true},
	"res://scenes/runner_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/mine_layer_enemy.tscn": {"beats": 10.0, "telegraph": false},
	"res://scenes/hopper_enemy.tscn": {"beats": 8.0, "telegraph": true},
	"res://scenes/pulser_enemy.tscn": {"beats": 10.0, "telegraph": true},
	"res://scenes/bouncer_enemy.tscn": {"beats": 6.0, "telegraph": true},
	"res://scenes/dasher_enemy.tscn": {"beats": 10.0, "telegraph": true},
	"res://scenes/lancer_enemy.tscn": {"beats": 16.0, "telegraph": true},
	"res://scenes/splitter_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/splitter_mini_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/chameleon_enemy.tscn": {"beats": 18.0, "telegraph": false},
	"res://scenes/warden_enemy.tscn": {"beats": 10.0, "telegraph": false},
	"res://scenes/foco_enemy.tscn": {"beats": 10.0, "telegraph": true},
	"res://scenes/papa_enemy.tscn": {"beats": 8.0, "telegraph": true},
	"res://scenes/sable_enemy.tscn": {"beats": 6.0, "telegraph": false},
	"res://scenes/birra_enemy.tscn": {"beats": 8.0, "telegraph": true},
	"res://scenes/caja_enemy.tscn": {"beats": 8.0, "telegraph": true},
	"res://scenes/corazon_enemy.tscn": {"beats": 8.0, "telegraph": true},
	"res://scenes/fiera_enemy.tscn": {"beats": 8.0, "telegraph": true},
	"res://scenes/pastilla_enemy.tscn": {"beats": 10.0, "telegraph": true},
	"res://scenes/maestro_enemy.tscn": {"beats": 6.0, "telegraph": true},
	"res://scenes/globo_enemy.tscn": {"beats": 10.0, "telegraph": false},
	"res://scenes/pelota_enemy.tscn": {"beats": 6.0, "telegraph": true},
	"res://scenes/mano_de_dios_enemy.tscn": {"beats": 12.0, "telegraph": true},
	"res://scenes/pollito_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/jeringa_enemy.tscn": {"beats": 10.0, "telegraph": true},
	"res://scenes/oveja_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/abeja_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/hermanos_enemy.tscn": {"beats": 10.0, "telegraph": false},
	"res://scenes/cohete_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/microfono_enemy.tscn": {"beats": 10.0, "telegraph": true},
	"res://scenes/gota_enemy.tscn": {"beats": 5.0, "telegraph": false},
	"res://scenes/gotita_enemy.tscn": {"beats": 5.0, "telegraph": false},
}
## Scenes whose pieces appear on death, and how many.
const PIECES := {"splitter_enemy": 2, "gota_enemy": 3}


## Stands in for a Player: has a team color and counts hits instead of losing lives.
class DummyPlayer:
	extends Node2D
	var is_dead := false
	var hits := 0
	var team_color := Color.WHITE

	func take_damage(_amount: float, _color := Color.WHITE) -> bool:
		hits += 1
		return true


var _conductor: Node
var _arena: Node2D
var _players: Array[DummyPlayer] = []


func run(t: E2EContext) -> void:
	Engine.time_scale = TIME_SCALE
	_conductor = Conductor.new()
	_conductor.use_clock = true
	t.tree.root.add_child(_conductor)
	_conductor.setup(null, BPM, 0.0, 600.0)
	_conductor.play(0.0)
	_arena = Node2D.new()
	t.tree.root.add_child(_arena)
	for i in COLORS.size():
		var player := DummyPlayer.new()
		player.team_color = COLORS[i]
		player.position = Vector2(400 + 480 * i, 200 + 320 * i)
		player.add_to_group("players")
		_arena.add_child(player)
		_players.append(player)
	await t.frames(2)

	for path in CASES:
		await _check_scene(t, path, CASES[path])
	await _check_warden(t)
	await _check_chameleon(t)
	await _check_hatch(t)
	await _check_burst(t)
	await _check_lifetime(t)

	_arena.queue_free()
	_conductor.queue_free()
	await t.frames(2)
	t.check_eq(t.tree.get_nodes_in_group("enemies").size(), 0, "no enemies left")
	Engine.time_scale = 1.0


func _spawn(path: String, at: Vector2) -> Node2D:
	var enemy: Node2D = load(path).instantiate()
	enemy.position = at
	_arena.add_child(enemy)
	return enemy


## Components with a cadence (they emit `acted`).
func _actors(enemy: Node) -> Array[Node]:
	var found: Array[Node] = []
	for child in enemy.get_children():
		if child.has_method("get_cadence") and child.has_signal("acted"):
			found.append(child)
	return found


func _check_scene(t: E2EContext, path: String, case: Dictionary) -> void:
	var label := path.get_file().get_basename()
	var enemy := _spawn(path, Vector2(640, 360))
	await t.frames(1)
	t.check(enemy.is_in_group("enemies"), "%s joins enemies" % label)
	t.check(enemy.is_in_group("danger"), "%s joins danger" % label)
	t.check(enemy.get_node_or_null("HealthComponent") != null, "%s has health" % label)
	t.check(enemy.get_node_or_null("EnemyVisual") != null, "%s has EnemyVisual" % label)

	var actors := _actors(enemy)
	var acts: Array = []
	for actor in actors:
		actor.acted.connect(
			func(action_beat: float, song_beat: float) -> void:
				acts.append([actor, action_beat, song_beat])
		)
	var spawn_beat: float = _conductor.song_beat()
	var end_beat: float = spawn_beat + case.beats
	var pending_seen := false
	var shapes_seen := false
	var windup_seen := false
	while is_instance_valid(enemy) and _conductor.song_beat() < end_beat:
		var records: PackedFloat32Array = enemy.danger_shapes()
		shapes_seen = shapes_seen or not records.is_empty()
		for i in range(0, records.size(), RECORD_LEN):
			if _activates_in(records, i) > 0.0:
				pending_seen = true
		windup_seen = windup_seen or enemy.get_node("EnemyVisual").get_windup() > 0.3
		await t.frames(1)
	if not t.check(is_instance_valid(enemy), "%s survived its watch" % label):
		return

	t.check(shapes_seen, "%s reports danger shapes" % label)
	if case.telegraph:
		t.check(pending_seen, "%s telegraphs its attack as pending danger" % label)
		t.check(windup_seen, "%s shows a wind-up" % label)
	for actor in actors:
		var cadence: Vector3 = actor.get_cadence()
		var mine := acts.filter(func(a: Array) -> bool: return a[0] == actor)
		t.check(mine.size() > 0, "%s/%s acted" % [label, actor.name])
		for act in mine:
			var action_beat: float = act[1]
			var song_beat: float = act[2]
			var k := (action_beat - cadence.y) / cadence.x
			t.check_near(k, roundf(k), 0.001, "%s/%s acts on its cadence" % [label, actor.name])
			t.check(
				song_beat >= action_beat - 0.001 and song_beat - action_beat <= BEAT_TOLERANCE,
				"%s/%s acted at %.3f for beat %.3f" % [label, actor.name, song_beat, action_beat]
			)
			t.check(
				action_beat >= spawn_beat + 1.0 + cadence.z - 0.001,
				"%s/%s waits out its spawn and wind-up" % [label, actor.name]
			)
	t.note("%s: %d actions, pending danger %s" % [label, acts.size(), pending_seen])

	await _kill(t, enemy, label)
	if PIECES.has(label):
		await _check_pieces(t, PIECES[label])
	_clear_bullets()


## The wrong color does nothing; the active colors break every layer and kill it.
func _kill(t: E2EContext, enemy: Node, label: String) -> void:
	var health: Node = enemy.get_node("HealthComponent")
	var layer: int = health.get_active_layer()
	var life: float = health.life
	# Without shields any color hurts, so only shielded enemies can ignore one.
	if layer >= 0:
		t.check(not enemy.take_damage(1000.0, WRONG_COLOR), "%s ignores the wrong color" % label)
		t.check_eq(
			health.get_active_layer(), layer, "%s keeps its shield against the wrong color" % label
		)
		t.check_eq(health.life, life, "%s keeps its life against the wrong color" % label)
	var died := {"count": 0}
	enemy.died.connect(func() -> void: died.count += 1)
	var id := enemy.get_instance_id()
	for i in 12:
		if not is_instance_id_valid(id) or died.count > 0:
			break
		t.check(
			enemy.take_damage(1000.0, health.get_active_color()),
			"%s takes its matching color" % label
		)
		await t.frames(1)
	t.check_eq(died.count, 1, "%s died once" % label)
	await t.frames(2)
	t.check(not is_instance_id_valid(id), "%s freed" % label)


func _check_pieces(t: E2EContext, count: int) -> void:
	await t.frames(2)
	var pieces := t.tree.get_nodes_in_group("enemies")
	t.check_eq(pieces.size(), count, "splitting leaves %d pieces" % count)
	var colors: Array[Color] = []
	for piece in pieces:
		var health: Node = piece.get_node("HealthComponent")
		t.check_eq(health.get_layer_count(), 1, "piece has one shield")
		colors.append(health.get_active_color())
	if colors.size() >= 2:
		t.check(colors[0] != colors[1], "pieces wear different colors")
	for piece in pieces:
		await _kill(t, piece, "splitter piece")


func _check_warden(t: E2EContext) -> void:
	var warden := _spawn("res://scenes/warden_enemy.tscn", Vector2(640, 360))
	var ally := _spawn("res://scenes/pulser_enemy.tscn", Vector2(760, 360))
	var far := _spawn("res://scenes/pulser_enemy.tscn", Vector2(100, 650))
	await t.frames(1)
	var ward: Node = warden.get_node("WardComponent")
	var ally_health: Node = ally.get_node("HealthComponent")
	var layers: int = ally_health.get_layer_count()
	await t.wait_until(func() -> bool: return ally_health.has_ward(), 10.0, "the Warden to ward")
	t.check_eq(ally_health.get_layer_count(), layers + 1, "a ward adds one layer")
	t.check_eq(
		ally_health.get_active_color(), ward.get_ward_color(), "the ward wears the Warden's color"
	)
	t.check(not far.get_node("HealthComponent").has_ward(), "enemies outside the ring stay bare")
	await _kill(t, warden, "warden")
	t.check(not ally_health.has_ward(), "the ward drops with the Warden")
	t.check_eq(ally_health.get_layer_count(), layers, "the ally is back to its own layers")
	await _kill(t, ally, "warded ally")
	await _kill(t, far, "far ally")
	_clear_bullets()


func _check_chameleon(t: E2EContext) -> void:
	var enemy := _spawn("res://scenes/chameleon_enemy.tscn", Vector2(640, 360))
	await t.frames(1)
	var health: Node = enemy.get_node("HealthComponent")
	var cycler: Node = enemy.get_node("ChameleonComponent")
	var changes: Array = []
	cycler.acted.connect(
		func(_action: float, _song: float) -> void: changes.append(health.get_active_color())
	)
	var before: Color = health.get_active_color()
	t.check(COLORS.has(before), "chameleon wears a player color")
	await t.wait_until(func() -> bool: return changes.size() >= 1, 15.0, "a color change")
	if changes.size() >= 1:
		t.check(changes[0] != before, "chameleon changed color on its beat")
		t.check(COLORS.has(changes[0]), "chameleon changes to a player color")
	await _kill(t, enemy, "chameleon")
	_clear_bullets()


## A Huevo left alone hatches three chicks on its fuse beat and leaves (not a kill);
## the chicks inherit its death listeners.
func _check_hatch(t: E2EContext) -> void:
	var egg := _spawn("res://scenes/huevo_enemy.tscn", Vector2(640, 360))
	await t.frames(1)
	var fuse: Node = egg.get_node("FuseComponent")
	var outcome := {"died": 0, "left": 0, "kills": 0}
	egg.died.connect(func() -> void: outcome.died += 1)
	egg.left.connect(func() -> void: outcome.left += 1)
	egg.died.connect(func() -> void: outcome.kills += 1)
	var popped := {"beat": -1.0, "song": -1.0}
	fuse.acted.connect(
		func(action: float, song: float) -> void:
			popped.beat = action
			popped.song = song
	)
	var spawn_beat: float = _conductor.song_beat()
	var windup_seen := false
	while popped.beat < 0.0 and _conductor.song_beat() < spawn_beat + 12.0:
		windup_seen = windup_seen or egg.get_node("EnemyVisual").get_windup() > 0.3
		await t.frames(1)
	t.check(windup_seen, "the egg winds up before hatching")
	t.check_near(popped.beat, ceilf(spawn_beat) + 8.0, 1.01, "the egg hatches 8 beats in")
	t.check_near(popped.song, popped.beat, BEAT_TOLERANCE, "the egg hatches on its beat")
	await t.frames(3)
	t.check_eq(outcome.left, 1, "the hatched egg leaves")
	t.check_eq(outcome.died, 0, "hatching is not a kill")
	var chicks := t.tree.get_nodes_in_group("enemies")
	t.check_eq(chicks.size(), 3, "three chicks hatch")
	for chick in chicks:
		t.check(chick.scene_file_path.ends_with("pollito_enemy.tscn"), "a chick hatched")
		t.check_near(chick.lifetime_beats, 24.0, 0.001, "chicks have a lifetime")
		await _kill(t, chick, "chick")
	t.check_eq(outcome.kills, 3, "chick kills reach the egg's listeners")
	_clear_bullets()


## A Limón bursts into a ring of shots on its fuse beat, reported as pending first.
func _check_burst(t: E2EContext) -> void:
	var lemon := _spawn("res://scenes/limon_enemy.tscn", Vector2(640, 360))
	await t.frames(1)
	var fuse: Node = lemon.get_node("FuseComponent")
	var popped := {"count": 0}
	fuse.acted.connect(func(_a: float, _s: float) -> void: popped.count += 1)
	var pending_seen := false
	while popped.count == 0 and is_instance_valid(lemon):
		var records: PackedFloat32Array = lemon.danger_shapes()
		for i in range(0, records.size(), RECORD_LEN):
			if _activates_in(records, i) > 0.0:
				pending_seen = true
		await t.frames(1)
	t.check(pending_seen, "the burst is pending danger before it fires")
	await t.frames(2)
	var shots := t.tree.get_nodes_in_group("enemy_projectiles")
	t.check_eq(shots.size(), 12, "the lemon bursts into 12 shots")
	t.check(shots.size() > 0 and shots[0].skin != null, "the shots wear the juice skin")
	t.check(not is_instance_valid(lemon), "the burst lemon is gone")
	_clear_bullets()


## An enemy given a lifetime blinks at the end, then leaves on time without dying.
func _check_lifetime(t: E2EContext) -> void:
	var enemy: Node2D = load("res://scenes/oveja_enemy.tscn").instantiate()
	enemy.position = Vector2(640, 360)
	enemy.lifetime_beats = 6.0
	_arena.add_child(enemy)
	var outcome := {"died": 0, "left": -1.0}
	enemy.died.connect(func() -> void: outcome.died += 1)
	enemy.left.connect(func() -> void: outcome.left = _conductor.song_beat())
	var spawn_beat: float = _conductor.song_beat()
	var blinked := false
	while outcome.left < 0.0 and _conductor.song_beat() < spawn_beat + 10.0:
		blinked = blinked or enemy.modulate.a < 0.9
		await t.frames(1)
	t.check(blinked, "the enemy blinks before leaving")
	t.check_near(outcome.left - spawn_beat, 6.0, BEAT_TOLERANCE, "it leaves after its lifetime")
	t.check_eq(outcome.died, 0, "leaving is not a kill")
	await t.frames(2)
	t.check_eq(t.tree.get_nodes_in_group("enemies").size(), 0, "the enemy is gone")


func _clear_bullets() -> void:
	for group in ["enemy_projectiles", "mines"]:
		for node in _arena.get_tree().get_nodes_in_group(group):
			node.queue_free()


func _activates_in(records: PackedFloat32Array, i: int) -> float:
	match int(records[i]):
		0, 1:
			return records[i + 6]
		_:
			return records[i + 8]
