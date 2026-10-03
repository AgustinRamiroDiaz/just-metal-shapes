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
  conductor.rs          # Conductor node
  level_catalog.rs      # LevelCatalog (static level list)
  director.rs           # LevelDirector: plays a Chart against the Conductor
  hazards/              # JSB-style hazard nodes (one file per hazard)
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
  `difficulty` (1-5), `seed`, `palette` (bg, accent), `pattern_pool` (which EventKinds
  are allowed), `enemy_pool`, `density` multiplier.
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

`chart_gen` maps analysis to events deterministically:

- Section type chooses the pattern family (intro: sparse lasers/pulses; build: rising
  density, sweeps; main: full patterns + enemy spawns; breakdown: few hazards, enemy
  phase; outro: wind-down).
- Strong onsets/accents trigger hits; bar intensity scales density; novelty spikes
  start a new phrase; `silent` beats get nothing.
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

## LevelDirector

Reads the `Chart` for the selected level, keeps a cursor, and when
`song_beat >= event.beat - event.telegraph_beats` spawns the hazard via a registry
(`EventKind -> fn(&mut Director, &ChartEvent)`). Hazards receive their absolute hit time
and query the Conductor themselves, so they stay synced if frames drop.

## Hazard contract

Every hazard node:

- joins group `"hazards"`;
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
- Lobby: any joined device can add/remove bots (keyboard: `B`/`Backspace`; gamepad: `X`/`Y`).
  Starting with zero humans is allowed (attract/demo mode).

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
- On all players down: rewind to the last checkpoint (song seeks back, hazards cleared,
  players revived) with a counter; results show rewinds used. Hardcore toggle disables it.
- Consistent theme: Kenney fonts and UI art, one accent color per level palette.

## Feel and animation

`Fx` autoload: `shake(strength)`, `hitstop(seconds)`, `burst(pos, color, amount)`,
`flash(color, seconds)`. Respect the screen-shake setting. Player squash/stretch on move,
hit flash + knockback ring, death shatter, revive burst; enemy spawn scale-in and
death explosion; arena background shader pulsing on beats; UI tweens on every screen
transition.

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
