# Complete Game Plan

Shared contract for taking Just Metal Shapes from prototype to a complete game. Every
workstream builds against the interfaces here. If an interface has to change, update this
file in the same commit.

## Pillars

1. **Music drives everything.** Each level is one song. Hazards, enemy spawns, arena pulses
   and camera kicks are scheduled on beats derived from offline beat analysis
   (Just Shapes & Beats style). Nothing spawns on a random timer.
2. **Co-op color combat stays.** Shielded enemies still need the matching player color to
   break shields (Full Metal Furies influence). Enemies are scheduled by the chart too.
3. **Any seat can be a bot.** AI players fill co-op seats and are also how e2e tests play
   full levels.
4. **Readable danger.** Every hazard telegraphs before it can hurt (JSB rule: warning shape
   first, then the hit), and the arena reacts to the beat.

## Runtime stack

- Godot 4.6 API (runs on 4.7), GL Compatibility renderer, Web export without threads.
- Gameplay, UI and tooling nodes in Rust (`rust/`, gdext 0.5). GDScript is used only for
  tests under `godot/tests/`.
- Viewport 1280x720, stretch `canvas_items`, aspect `keep`.

## Rust layout

Pure logic with no Godot types lives in `rust/src/core/` so `cargo test` covers it without
a Godot runtime. Godot node classes wrap it.

```
rust/src/
  core/                 # no `godot` imports allowed
    mod.rs
    analysis.rs         # SongAnalysis serde model (output of devtools/analyze_beats.py)
    chart.rs            # Chart, ChartEvent, EventKind, PatternParams
    chart_gen.rs        # generate_chart(&SongAnalysis, &LevelSpec, seed) -> Chart
    timing.rs           # beat<->seconds conversion, latency math
    danger.rs           # DangerShape + encode/decode to flat f32 records
    bot.rs              # bot decision logic over a DangerSnapshot
    scoring.rs          # score, rank (S/A/B/C/D) from run stats
    rng.rs              # small deterministic PRNG (no global randomness in core)
    mode.rs             # DifficultyMode (casual/normal/hardcore rules)
  conductor.rs          # Conductor node
  level_catalog.rs      # LevelCatalog (static level list)
  director.rs           # LevelDirector: plays a Chart against the Conductor
  hazards/              # JSB-style hazard nodes (one file per hazard; mod.rs = shared helpers)
  enemy_spawn.rs        # SpawnEnemy handler
  groups.rs             # scene-tree group names
  danger_field.rs       # DangerField node: per-frame snapshot of all hazards
  bot_brain.rs          # BotBrain node: drives a Player from core::bot
  fx.rs                 # Fx autoload: shake, hitstop, bursts, flashes
  ui/                   # menus, HUD, pause, results, settings
  save.rs               # SaveData autoload (user://save.json)
  ...existing modules (player, enemy, projectile, manager, menu, game_config)
```

`Cargo.toml` uses `crate-type = ["cdylib", "rlib"]`. `serde`/`serde_json` are allowed.

## Song analysis and levels

- `devtools/analyze_beats.py` (ported from webcam-motion-games, numpy + ffmpeg, run with
  `uv`) writes `godot/music/<song-id>.analysis.json`: `bpm`, `beatOffsetSeconds`,
  `durationSeconds`, `beats[]` (loudness, onsetStrength, low/mid/highEnergy, accent,
  novelty, silent), `onsets[]` (quantizedBeat, strength), `bars[]` (intensity,
  onsetDensity, novelty, silent), `sections[]` (startBar, endBar, startBeat, endBeat,
  type ∈ intro|build|main|breakdown|outro, intensity).
- Audio lives in `godot/music/*.ogg` (or `.mp3`), with `godot/music/CREDITS.md` listing
  source, author and license for every track.
- `LevelSpec` (core) per level: `id`, `title`, `artist`, `music_path`, `analysis_path`,
  `difficulty` (1-5), `seed`, `palette` (bg, accent, danger), `pattern_pool` (which
  EventKinds are allowed), `enemy_pool`, `phrases` (choreography emphasis), `density`
  multiplier, `tutorial` (show hints), `finale` (end with the layered set piece).
  Interface detail: `pattern_pool` is `Vec<PatternEntry>` (kind plus weight, allowed
  sections, telegraph/duration beats, safe-path `coverage`, size, speed, count range,
  `min_accent`) so hazards are tuned as data; `enemy_pool` is `Vec<EnemyEntry>` (scene,
  `spawn_outside`, weight); `phrases` is `Vec<PhraseEntry>` (`Phrase` plus weight; empty
  means every phrase at its default weight). See `docs/spec/hazards.md` and
  `docs/spec/levels.md`.
- Tempo estimates that lock onto a harmonic are pinned per song in
  `devtools/music_overrides.json` (`analyze_all.py` applies them).
- `LevelCatalog` exposes the ordered list. Level 1 is the gentlest and acts as the
  tutorial (sparse patterns, on-screen hints during the intro section).

## Chart (schedule)

```rust
pub struct ChartEvent {
    pub beat: f64,          // when the hazard becomes dangerous (the "hit" beat)
    pub telegraph_beats: f64, // warning lead time before `beat`
    pub kind: EventKind,
    pub params: PatternParams, // position/angle/count/speed/size/color index, etc.
}
pub enum EventKind {
    // hazards (JSB-like)
    Laser, LaserSweep, BulletRing, Spiral, Wall, Pulse, Bomb, Spikes, Barrage,
    // co-op enemies (existing scenes)
    SpawnEnemy,
    // presentation
    ArenaPulse, CameraKick, Flash, PaletteShift, Checkpoint, ShowHint,
}
pub struct Chart { pub bpm: f64, pub offset_seconds: f64, pub duration_seconds: f64, pub events: Vec<ChartEvent> }
```

`PatternParams` is one flat struct for every kind: `x`, `y` (normalized 0..1 arena),
`angle` (radians), `count`, `speed` (arena heights/s), `size` (fraction of arena height),
`duration_beats`, `color_index`, `variant` (enemy pool index for `SpawnEnemy`, section
index for `Checkpoint`, `HINTS` index for `ShowHint`), `intensity` (0..1 musical
strength). Hazards read the fields they need.

`chart_gen` maps analysis to events deterministically:

- Sections are cut into bar-aligned phrases; each plays a `Phrase` (authored
  choreography: laser call and response, rings on kicks, sweeping walls, spiral risers,
  snare barrages, bomb pairs, spike sides, sweep crosses...) chosen by section type and
  the level's `phrases` (intro: sparse pulses; build: risers, rising density; main: full
  patterns + enemy spawns; breakdown: breathers, enemy phase; outro: wind-down).
- Kicks, snares and strong accents place hits; bar intensity and difficulty set how
  often; novelty spikes start a new phrase; `silent` beats get nothing (the structural
  `Checkpoint` and `ShowHint` events are exempt).
- Every bar gets an `ArenaPulse` on the downbeat; strong accents add `CameraKick`.
- Each section start gets a `Checkpoint`.
- A difficulty-aware cap guarantees a safe path: limit simultaneous screen coverage and
  never schedule two hazards that together cover the whole arena.

## Conductor

`Conductor` (Node, child `AudioStreamPlayer`), single source of song time.

- `#[func] song_time() -> f64` seconds; `song_beat() -> f64`.
- Time source: audio playback position corrected with
  `AudioServer.get_time_since_last_mix() - get_output_latency()` plus user latency offset;
  falls back to an internal clock (`delta * Engine.time_scale`) when `use_clock = true` or
  when audio is unavailable (headless/dummy driver). Tests set `use_clock`.
- Signals: `beat(index: i64)`, `bar(index: i64)`, `section_started(index: i64, type: GString)`,
  `song_finished()`.
- `seek(seconds)` for checkpoints; `pause()`/`resume()`.
- Also: `play(from_seconds)`, `stop()`, signals `seeked(seconds)` and
  `paused_changed(paused)`, `get_seek_count()` (lets the director resync after jumps).
  Frame `delta` is already scaled by `Engine.time_scale`. The user `latency_offset` is
  subtracted: positive means audio is heard later than reported.

## LevelDirector

Reads the `Chart` for the selected level, keeps a cursor, and when
`song_beat >= event.beat - event.telegraph_beats` spawns the hazard via a registry
(`EventKind -> fn(&mut Director, &ChartEvent)`). Hazards receive their absolute hit time
and query the Conductor themselves, so they stay synced if frames drop.
Presentation kinds are signals on the director: `arena_pulse(beat, intensity)`,
`camera_kick(strength)`, `flash(color, duration_seconds)`,
`palette_shift(section_type, intensity)`, `checkpoint_reached(index, beat)`,
`show_hint(text, duration_seconds)`; plus `event_spawned(kind, beat, song_beat)`,
`enemy_spawned(enemy)`, `enemy_died()`, `rewound(beat)`.
`spawn_event(kind, beat, telegraph_beats, params)` dispatches one event outside the
chart (tests, previews); hazards draw in the level's `danger_color()`.

## Hazard contract

Every hazard node:

- joins groups `"hazards"` and `"danger"` (`LevelDirector::add_hazard` does both;
  `DangerField` gathers the `"danger"` group);
- is harmless while telegraphing, damages players (`take_damage(1.0)`) once active;
- frees itself when done;
- implements `#[func] fn danger_shapes(&self) -> PackedFloat32Array` returning records
  encoded by `core::danger` (both telegraphing and active shapes, with `activates_in`
  seconds so bots can plan ahead). Enemy projectiles, mines and contact-damage enemies
  implement it too.

```rust
pub enum DangerShape {
    Circle  { center: V2, radius: f32, velocity: V2, activates_in: f32 },
    Capsule { a: V2, b: V2, radius: f32, activates_in: f32 },    // lasers, beams
    Rect    { center: V2, half: V2, angle: f32, velocity: V2, activates_in: f32 }, // walls
}
```

`DangerField` (node `"DangerField"` in the level) gathers all records once per physics
frame into a `core::danger::DangerSnapshot` that bots query in pure Rust.

## Players and bots

- `InputType::Bot = 100` (constant `GameConfig::BOT`). A Player with this input type reads
  its move vector from its `BotBrain` child instead of `Input`.
- `core::bot::decide(&BotInput) -> V2`: sample candidate directions, score each by
  predicted danger over a short horizon, attraction to enemies whose active shield
  matches the bot's color (stay inside range ring but outside contact radius), attraction
  to downed teammates (revive), mild cohesion, and arena-bounds penalty. Pure and unit
  tested with hand-built snapshots.
  Interface detail: candidates are *plans* (heading x speed x stop time, then hold) scored
  over `horizon + hold` seconds with swept sampling, so bots pre-empt telegraphs
  (`activates_in`), lead moving shapes and wait in a wall's gap. Bots treat any
  `danger_shapes()` record generically; a new hazard needs nothing bot-specific.
  `BotTuning::for_skill(skill)` holds every knob (reaction delay, decision interval,
  horizon, hold, plans, directions, noise, turn rate, term weights).
- Lobby: any joined device can add/remove bots (keyboard: `B`/`Backspace`; gamepad: `X`/`Y`).
  Starting with zero humans is allowed (attract/demo mode).

### Lobby API (`GameConfig` autoload)

`GameConfig.players` is the seat list the level spawns, in order (seat index = spawn
slot). Use these helpers instead of building `PlayerConfig`s by hand so colors stay unique
across humans and bots:

| Call | Effect |
|---|---|
| `add_human(input_type) -> bool` | Seat for that input with the first free color, named `P<n>`. False when full or that input already has a seat. |
| `remove_human(input_type) -> bool` | Frees that input's seat. |
| `find_human(input_type) -> int` | Seat index or -1. |
| `add_bot(skill) -> bool` | Bot seat (`BOT_EASY`/`BOT_NORMAL`/`BOT_HARD`) with the first free color, named `BOT <n>`. False when full. |
| `remove_last_bot() -> bool` | Removes the most recently added bot. |
| `bot_count()`, `human_count()` | Seat counts. |
| `next_free_color() -> Color` | First `get_player_colors()` entry no seat uses. |
| `bot_skill_name(skill) -> String` | `easy` / `normal` / `hard`. |

Constants: `BOT = 100` (input type), `BOT_EASY = 0`, `BOT_NORMAL = 1`, `BOT_HARD = 2`,
`MAX_PLAYERS = 8`. `PlayerConfig` carries `input_type`, `color`, `bot_skill`,
`display_name` and `is_bot()`; `PlayerConfig.new_bot(skill, color, name)` exists for
custom setups. With no seats configured the level still falls back to two keyboard
players.

At spawn, `GameManager` gives each bot seat a `BotBrain` child (`skill`, `seed` = seat
index) and stores every seat's `display_name` as the Player's `display_name` meta.
`BotBrain` exposes `get_move_direction()`, `skill` (settable at runtime), `enabled`
(false = stand still) and `get_stats()` (`decisions`, `avg_decide_usec`,
`max_decide_usec`, `hit_predictions`, `last_danger`, `skill`). A downed bot stops deciding
and stays put until revived; bots revive humans and bots alike.

| Skill | Reaction | Decisions | Horizon + hold | Notes |
|---|---|---|---|---|
| easy | 0.25 s | 12 Hz | 0.6 + 0.3 s | noisy, slow turns; takes hits |
| normal | 0.12 s | 20 Hz | 1.2 + 1.0 s | |
| hard | 0.04 s | 30 Hz | 1.6 + 1.5 s | no noise |

Cost (release build): about 26 us per decision in a level, about 5 ms of CPU per game
second for 8 bots; about 170 us per decision against 160 bullets on screen.

## Game flow and UX

```
Boot -> Title -> Level Select -> Lobby (join / bots) -> Level (countdown 3-2-1)
     -> Results (rank, stats, retry / next / menu)
Pause overlay (Esc / Start): resume, restart, settings, quit to menu.
Settings: master/music/sfx volume, screen shake on/off, audio latency offset, fullscreen.
```

- `SaveData` autoload: best rank/score per level, settings, levels unlocked
  (level N+1 unlocks when N is cleared; all levels unlocked in debug builds).
- HUD: song progress bar with section ticks, per-player life pips in player color,
  score, combo-free (team score only), checkpoint toast.
- On all players down: rewind to the last checkpoint (song seeks back, hazards, enemies,
  enemy projectiles and mines cleared so the chart can respawn them, players revived
  with full lives) with a counter; results show rewinds used. Hardcore toggle disables
  it. `GameConfig.difficulty_mode` is CASUAL/NORMAL/HARDCORE (`core::mode`: chart
  density scale, rewind allowed, score multiplier).
- Consistent theme: Kenney fonts and UI art, one accent color per level palette.

## Feel and animation

`Fx` autoload: `shake(strength)`, `hitstop(seconds)`, `burst(pos, color, amount)`,
`flash(color, seconds)`. Respect the screen-shake setting. Player squash/stretch on move,
hit flash + knockback ring, death shatter, revive burst; enemy spawn scale-in and
death explosion; arena background shader pulsing on beats; UI tweens on every screen
transition.

Interface detail (`rust/src/fx.rs`, autoload `/root/Fx`):

- `shake(strength)` adds trauma (0.15 small, 0.35 medium, 0.7 large) to the viewport
  `Camera2D` offset/roll, scaled by `SaveData.screen_shake` (bool or 0..1 float; on
  without SaveData). `hitstop(seconds)` dips `Engine.time_scale` relative to the current
  scale for at most 0.12 real seconds, with a cooldown; pausing or another time-scale
  change cancels it. `burst(pos, color, amount)` and `burst_style(pos, color, amount,
  style, speed)` (`Fx.BURST_SPARKS|DOTS|SHARDS`) use a pool of CPUParticles2D;
  `flash(color, seconds)`, `ring(pos, color, radius, seconds)`, `sweep(color, seconds)`,
  `rewind_effect(seconds)`; `play_sfx(name, pos = screen center, pitch_jitter = 0.06,
  volume_db = 0)` plays `assets/sfx/<name>.ogg` on bus `SFX` (else `Master`) with
  per-sound voice limits and retrigger throttling. Rust callers use `fx::with_fx`.
- Overlays (flash, rewind) are on CanvasLayer 5; HUD and menus should use layer 10+.
  World effects draw at z 50 (above players, enemies, hazards); the `Arena` background
  sits at z -1000.
- Level scene: `Arena` (background + director/conductor signal glue) and a `Camera2D`
  centered on the arena. Presentation nodes live in `rust/src/visuals/`
  (`PlayerVisual`, `EnemyVisual`) and never change gameplay state.

### Visual language and palette

Color says who owns a shape. Keep these families apart so danger reads at a glance:

| Owner | Color | Shape language |
|---|---|---|
| Arena | level `bg` + `accent` at low alpha (grid, hexagon motif, pulses) | thin lines, never solid fills |
| Hazards | level `accent` (and `hazard_color` variants, hot pink `(1, 0.25, 0.45)`) | telegraph outline/fill, then a solid white-flash hit |
| Enemies | metal bodies (cool gray-white ramp) with an amber core ring `(1, 0.55, 0.15)` | Kenney simple-space silhouettes, one per type |
| Enemy projectiles and mines | amber family: white-hot core, amber `(1, 0.55, 0.15)` glow and trail | round orbs with trails; mines are dark cores with amber spikes |
| Shields | the matching player's team color | segmented plate rings around enemies |
| Players | team color (`GameConfig.get_player_colors`) | rounded bodies with faces, range ring, lightning |

Rules: hazards never use amber; enemy-owned things never use the level accent; white is
reserved for impacts (hit frames, flashes, sparks). No level accent should be close to
amber (current accents: cyan, lime, pink).

## Testing

- Unit: `cargo test` over `core::*` (analysis parsing, chart generation invariants
  — sorted, within song bounds, safe path, deterministic for a seed — timing math,
  danger encode/decode + queries, bot decisions, scoring).
- E2E: headless Godot runner `godot/tests/run_e2e.gd`, executed by
  `make e2e` (`godot --headless --path godot -s res://tests/run_e2e.gd`). Scenarios use
  `Conductor.use_clock = true` and raised `Engine.time_scale` for speed:
  1. boot reaches Title; navigating to Level Select and Lobby works with injected input;
  2. a level with 2 bots runs to the end and shows Results;
  3. pause/resume freezes song time;
  4. all players down triggers checkpoint rewind; hardcore triggers game over;
  5. chart events spawn at the expected song beats (director timing tolerance);
  6. every level's analysis + chart loads.
  Exit code is non-zero on any failure. CI runs `cargo test` and `make e2e`.
- Make targets: `make test` (cargo test), `make e2e`, `make test-all`.
