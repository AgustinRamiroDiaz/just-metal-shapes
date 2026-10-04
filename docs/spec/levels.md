# Levels

Five campaign levels plus a bonus level, one song each, in `rust/src/level_catalog.rs`.
Each level has its own palette, signature phrases (the highest `phrases` weight), hazard
pool and enemy mix. Through the campaign, difficulty rises with the hardest stretch of
each chart (peak 8-bar hazard pressure, checked by
`every_level_generates_a_playable_chart`).

| # | Level | Song (BPM) | Identity | Hazards | Enemies (**new**) |
|---|---|---|---|---|---|
| 1 | `wonders-of-the-earth` (tutorial) | Grand Project (140) | Deep blue and cyan. Pulse grids, a few mirrored lasers, gapped rings; on-screen hints | Pulse, Laser, BulletRing | **Pulser**, **Hopper**, static shooter, turret |
| 2 | `voxel-revolution` | Kevin MacLeod (122) | Violet and lime, blocky. Spikes from alternating sides, walls with gaps | Pulse, Laser, Spikes, Wall, BulletRing | **Bouncer**, **Splitter**, static shooter, runner, turret, hopper, pulser |
| 3 | `celtic` | Alex Morgan (126) | Forest green. A reel: rings on every kick, lasers trading sides, sweeps | Pulse, Laser, LaserSweep, BulletRing, Wall, Spikes, Spiral | **Dasher**, **Chameleon**, shooter, shotgun, runner, turret, hopper, bouncer, splitter |
| 4 | `ouroboros` | Kevin MacLeod (107) | Teal-black and gold, the serpent. Spiral risers, rotating sweep crosses, bomb pairs | All but Barrage | **Lancer**, **Warden**, turret, mine layer, shotgun, runner, pulser, dasher, chameleon |
| 5 | `surf-rock` (final) | Alex Morgan (147) | Wine red and orange. Snare barrages, bomb pairs, walls, then the finale | All nine | All thirteen |
| Bonus | `las-huevas` | Banzai FC ft. Wos (~96, live, tempo map) | Navy and celeste. A 9-minute live freestyle: every enemy is a lyric reference arriving on its word, with the word as a caption; the band's jams carry the hazards; "se vienen los climas" opens the finale | All but Spiral | The 21 lyric enemies (see `enemies.md`) |

Each level's new enemies (`EnemyEntry::introduced`) arrive first and alone, so players
meet one rule at a time before breakdown waves stack them (see `spawning.md`).

## Las Huevas: lyric cues

`las-huevas` is scripted from `godot/music/las-huevas.cues.json` (loaded by
`level_catalog::apply_cue_sheet`). Each cue has a song time `t` (the sung word, from a
Whisper large-v3 transcription of the Demucs-isolated vocals), a position, an optional
`caption` (a few words of the line) and one action:

- `enemy` (a kind id), with `count` (a group around the position) and `life` (beats
  before it leaves on its own, blinking for the last two);
- `hazard` (a pattern-pool kind) with `variant`/`angle` overrides;
- neither: the caption alone.

`chart_gen` places cues on the nearest half beat of the tempo map. A cue enemy spawns
`ENEMY_TELEGRAPH_BEATS` early so it appears on its word, and its `Caption` event pops the
caption over it (`Hud._on_caption`). Cue hazards are committed before the phrases' and
never thinned (the safe-path caps still apply). Generated enemies only arrive in phrases
at least `CUE_CLEARANCE_BEATS` from a cue enemy, so the band's jams (158-305 s and
445-530 s) bring a few from the same pool and the verses stay scripted.

The level is difficulty 5 and also sets `finale`. Lines about real people and politics
(Santiago Maldonado, an election) and the swearing get no cues.

## Finale

`surf-rock` and `las-huevas` set `finale`: the last 12 bars of main/build music before
the outro become the boss set piece. At its downbeat it fires a full-strength `Flash`, `PaletteShift` and
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
- `las-huevas`: a live band drifting between ~93 and ~99 BPM, so `trackTempo` tracks
  every beat (dynamic programming over the onset envelope, smoothed over 8 beats, beat 0
  on the strongest kick phase). The analysis sets `variableTempo` and the chart carries
  the beat times; `core::timing::Timing` interpolates between them. Tracked beats line
  up with onsets at 2.07 vs 1.24 off-beat; a fixed 96.3 BPM grid scored 0.76 vs 0.73
  (chance).

## Adding a level

1. Add the track and analysis (see `godot/music/CREDITS.md`, "Adding a track").
2. Add a `LevelSpec` in `level_catalog.rs` with a distinct palette (`bg`, `accent`,
   `danger`), a `pattern_pool`, an `enemy_pool` and `phrases` listing the phrases it
   plays (weights above 1 make them signatures).
3. Add the analysis to `REAL_ANALYSES` in `core/test_support.rs` and run `cargo test`.
   `level_stats` (ignored test) prints the result for tuning.
