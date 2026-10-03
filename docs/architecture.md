# Architecture: Composition Over Inheritance

This project uses a **composition-based architecture** where behavior is assembled from independent, reusable components attached to scene nodes. There are no inheritance hierarchies beyond what Godot requires (extending built-in node types).

## Core Principle

Every entity in the game is defined by **what components it has**, not by what class it extends. A single `BaseEnemy` script handles all enemies. The difference between a turret and a shotgun enemy is entirely determined by which component nodes are attached in the scene editor.

## Enemy System

### BaseEnemy

All enemies use `base_enemy.gd` directly. It provides the minimal shared contract:

- `died` signal
- Wires `HealthComponent.died` to `queue_free()`
- Exposes `take_damage()` that delegates to `HealthComponent`

There are no per-enemy scripts. Enemy behavior is defined purely by scene composition.

### Example: How Enemies Are Built

```
ShotgunEnemy (StaticBody2D + base_enemy.gd)
  ├── Sprite2D
  ├── CollisionShape2D
  ├── HealthComponent            (HP, shields, color matching)
  ├── ShotgunShooterComponent    (fires fan of projectiles on timer)
  ├── ChaserComponent            (moves toward nearest player)
  ├── TurnComponent              (rotates sprite toward target)
  └── ContactDamageComponent     (deals damage on body collision)
```

```
MineLayerEnemy (StaticBody2D + base_enemy.gd)
  ├── Sprite2D
  ├── CollisionShape2D
  ├── HealthComponent
  ├── ChaserComponent
  ├── MineDropperComponent       (drops mines on timer)
  └── ContactDamageComponent
```

To create a new enemy type, you create a new `.tscn` scene, attach `base_enemy.gd`, and add whichever components define its behavior. No code changes needed.

### Available Enemy Components

| Component | Responsibility |
|---|---|
| `HealthComponent` | HP, shield with color matching, damage flash, visual rings |
| `ShooterComponent` | Fires single aimed projectile at nearest player on timer |
| `ShotgunShooterComponent` | Fires fan of projectiles based on sprite rotation |
| `TurretShooterComponent` | Fires alternating cardinal/diagonal projectile patterns |
| `ChaserComponent` | Moves toward nearest alive player |
| `ColorChaserComponent` | Moves toward nearest player whose color doesn't match its shield |
| `TurnComponent` | Rotates sprite toward nearest player |
| `MineDropperComponent` | Drops mines at current position on timer |
| `ContactDamageComponent` | Deals damage to players on body contact |

### Component Independence

Components are designed to be self-contained:

- Each component manages its own state and timing (using `Timer` nodes)
- Components read from the scene tree (groups, parent position) rather than referencing siblings directly
- When a component needs optional context from a sibling (e.g., `ColorChaserComponent` reading shield color from `HealthComponent`), it uses `get_node_or_null()` in `_ready()` and falls back to defaults

## Player System

The player follows the same principle. The `Player` script handles core mechanics (movement, input, damage, range tiers), while extracted components handle specific subsystems:

```
Player (CharacterBody2D + player.gd)
  ├── Sprite2D
  ├── CollisionShape2D
  ├── RevivalComponent         (teammate revival mechanic)
  ├── LightningComponent       (visual lightning rays to damaged targets)
  └── RangeArea                (Area2D detecting enemies in range)
```

## Game Management

A level is one song. The game loop is split by responsibility:

```
Main Level (Node2D + GameManager)      main_level.tscn
  ├── Arena            (background shader; routes director presentation signals to Fx)
  ├── Camera2D         (fixed on the arena; Fx shakes its offset)
  ├── Conductor        (song time, beat/bar/section signals, seek/pause)
  ├── LevelDirector    (analysis -> chart -> spawns events on time)
  ├── DangerField      (per-physics-frame DangerSnapshot of every hazard)
  ├── ScoreLabel / DebugLabel
  └── players, enemies (added at runtime)
```

`GameManager` spawns players from the `GameConfig` autoload, asks the director to load
`GameConfig.selected_level_id`, runs a 3-2-1 countdown and starts the Conductor. The level
is cleared on `Conductor.song_finished`. When all players are down it calls
`LevelDirector.rewind_to_checkpoint()` and respawns players (or ends the run in hardcore).
It tracks run stats and exposes `get_run_stats()` (score/rank from `core::scoring`).

### Music-driven pipeline

```
godot/music/<id>.ogg --devtools/analyze_all.py--> <id>.analysis.json
   -> core::analysis::SongAnalysis
   -> core::chart_gen::generate_chart(analysis, LevelSpec, seed) -> core::chart::Chart
   -> LevelDirector: registry EventKind -> SpawnFn
        hazards/*         (one node per hazard, group "hazards")
        enemy_spawn.rs    (SpawnEnemy: existing enemy scenes + spawn effect)
        signals           (ArenaPulse, CameraKick, Flash, PaletteShift, Checkpoint, ShowHint)
```

- `rust/src/core/` is pure Rust (no `godot`, threads or system clock) and holds every
  rule worth unit testing: analysis model, timing math, PRNG, chart generation, danger
  shapes, scoring, difficulty modes and bot decisions.
- `LevelCatalog` (`level_catalog.rs`) is the static level list. Each `LevelSpec` carries a
  data-driven `pattern_pool` (`PatternEntry`: kind, weight, sections, telegraph, duration,
  coverage, size, speed, count) that hazards are tuned through.
- The director spawns each event at `beat - telegraph_beats`. Hazards receive absolute
  song times (`HazardTiming`) and read the Conductor every frame (`HazardClock`), so they
  stay in sync through frame drops, pauses and seeks.

### Hazards and danger

Every hazard joins `hazards` and `danger`, is harmless while telegraphing, damages
players through `hazards::hit_players` once active, frees itself when done, and implements
`danger_shapes() -> PackedFloat32Array` (records from `core::danger`). `Projectile`,
`Mine` and enemies with a `ContactDamageComponent` report shapes too. `DangerField` decodes
all records once per physics frame into a `DangerSnapshot` for bots. `hazards/pulse.rs` is
the reference hazard; `hazards/mod.rs` documents the steps to add one.

Groups (`groups.rs`): `players`, `enemies`, `hazards`, `danger`, `enemy_projectiles`,
`mines`, `spawn_effects`. A rewind frees everything in the last five.

## Testing

- `make test`: `cargo test` over `core` (plus catalog and hazard timing tests).
- `make e2e`: builds the extension, imports the project and runs
  `godot/tests/run_e2e.gd`, which runs every `godot/tests/e2e/test_*.gd` scenario with a
  timeout (`make e2e ONLY=rewind` for one). Scenarios get an `e2e_context.gd` helper
  (`check*`, `wait_until`, `change_scene`, `start_level`, input injection) and typically
  set `Conductor.use_clock` and `Engine.time_scale`.
- `make test-all`: both.

### Presentation

The `Fx` autoload (`fx.rs`) owns shake, hit-stop, pooled particle bursts, flashes, rings,
the checkpoint sweep, the rewind overlay and gameplay SFX. Visual-only component nodes
in `rust/src/visuals/` sit in the entity scenes and read gameplay state without changing
it: `PlayerVisual` (squash/stretch, beat bounce, trail, range ring, hit/down/revive) and
`EnemyVisual` (spawn pop, damage flash, aim/recoil/muzzle flash, shield-break plates,
death explosion). Enemy bodies are a `Sprite2D` pivot (rotated by `TurnComponent`) with
a `Body` child sprite. Tunable feel math (trauma, hit-stop, squash, beat envelopes) is in
`core/feel.rs`. `godot/tests/capture_screens.gd` stages gameplay moments and saves
screenshots plus a frame-time sample for visual checks (needs a display).

## Shared Utilities

Static utility classes avoid duplicating logic across components:

| Utility | Purpose |
|---|---|
| `Targeting` | `get_nearest_alive(tree, origin, group)` — shared nearest-target lookup with dead-player filtering |
| `ColorUtils` | `colors_match(c1, c2)` — RGB color comparison used by shield and chaser systems |

## Adding New Behavior

### New enemy type
1. Create a `.tscn` scene
2. Set the root to `StaticBody2D` with `base_enemy.gd`
3. Add a `HealthComponent` and whichever behavior components you need
4. Add an `EnemyEntry` for it to the levels' `enemy_pool` in `level_catalog.rs`

### New hazard
Follow the steps at the top of `rust/src/hazards/mod.rs`, using `hazards/pulse.rs` as
the template, then add a `PatternEntry` for its `EventKind` to a level's `pattern_pool`.

### New song / level
Drop `godot/music/<id>.ogg`, run `make analyze-music`, credit it in
`godot/music/CREDITS.md`, and add a `LevelSpec` in `level_catalog.rs`.

### New enemy component
1. Create a script extending `Node` (or `Node2D` if it draws)
2. Use `Timer` nodes for periodic actions
3. Use `Targeting.get_nearest_alive()` for player lookups
4. Read parent position via `(get_parent() as Node2D).global_position`
5. Attach it to any enemy scene that needs the behavior

### New player component
1. Create a script in `scripts/player/`
2. Add the node to `scenes/player.tscn`
3. Access it from `player.gd` via `@onready`
