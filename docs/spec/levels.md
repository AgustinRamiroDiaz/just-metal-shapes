# Levels

Five levels, one song each, in `rust/src/level_catalog.rs`. Each level has its own
palette, signature phrases (the highest `phrases` weight), hazard pool and enemy mix.
Difficulty rises with the hardest stretch of each chart (peak 8-bar hazard pressure,
checked by `every_level_generates_a_playable_chart`).

| # | Level | Song (BPM) | Identity | Hazards | Enemies |
|---|---|---|---|---|---|
| 1 | `wonders-of-the-earth` (tutorial) | Grand Project (140) | Deep blue and cyan. Pulse grids, a few mirrored lasers, gapped rings; on-screen hints | Pulse, Laser, BulletRing | Static shooter, turret |
| 2 | `voxel-revolution` | Kevin MacLeod (122) | Violet and lime, blocky. Spikes from alternating sides, walls with gaps | Pulse, Laser, Spikes, Wall, BulletRing | Static shooter, runner, turret |
| 3 | `celtic` | Alex Morgan (126) | Forest green. A reel: rings on every kick, lasers trading sides, sweeps | Pulse, Laser, LaserSweep, BulletRing, Wall, Spikes, Spiral | Shooter, shotgun, runner, turret |
| 4 | `ouroboros` | Kevin MacLeod (107) | Teal-black and gold, the serpent. Spiral risers, rotating sweep crosses, bomb pairs | All but Barrage | Turret, mine layer, shotgun, runner |
| 5 | `surf-rock` (final) | Alex Morgan (147) | Wine red and orange. Snare barrages, bomb pairs, walls, then the finale | All nine | All five |

## Finale

`surf-rock` sets `finale`: the last 12 bars of main/build music before the outro become
the boss set piece. At its downbeat it fires a full-strength `Flash`, `PaletteShift` and
`CameraKick`. Each phrase inside layers two phrases at high heat, in order: sweep cross +
rings on kicks, sweeping walls + spike sides, laser call and response + snare barrages,
bomb pairs + spiral risers. No enemies spawn during it. The safe-path caps still apply.

## Tempo notes

Analysis comes from `make analyze-music` (`FFMPEG=/usr/bin/ffmpeg` on machines whose
default ffmpeg lacks Vorbis). Per-song decisions live in `devtools/music_overrides.json`:

- `wonders-of-the-earth`: pinned to 140 (the estimate locks onto 2:3).
- `ouroboros`: kept at 107.17. Its grid holds 54% of strong-onset weight (4.3x chance)
  and its eighth-note grid 80%. 130 BPM scores at chance, and the 71.25 and 142.5
  peaks are 2:3 and 4:3 of 107.
- `voxel-revolution`: 122.2 as estimated (61 is the half-time peak).

## Adding a level

1. Add the track and analysis (see `godot/music/CREDITS.md`, "Adding a track").
2. Add a `LevelSpec` in `level_catalog.rs` with a distinct palette (`bg`, `accent`,
   `danger`), a `pattern_pool`, an `enemy_pool` and `phrases` listing the phrases it
   plays (weights above 1 make them signatures).
3. Add the analysis to `REAL_ANALYSES` in `core/test_support.rs` and run `cargo test`.
   `level_stats` (ignored test) prints the result for tuning.
