# Enemies

Every enemy is a `BaseEnemy` scene (`rust/src/enemies/`) built from components. Each has
a `HealthComponent`, a `ContactDamageComponent` (1 damage on touch) and an `EnemyVisual`.
Each type follows one rule you can learn in a few seconds; the challenge comes from
stacking several at once.

## Shields

- Enemies carry colored shield layers that deplete before life.
- Lightning breaks a layer only if the player's color matches it. Once every layer is
  gone, any color damages life.
- `auto_shield_layers` picks each layer's color at random from the players in the level
  (the Splitter's pieces and the Chameleon set theirs explicitly).
- A **ward** is an extra outer layer a Warden lends in its own color. It is drawn with
  six wide plates (ordinary layers have twelve) and drops when the Warden dies.
- Enemy life is multiplied by the number of seats at spawn.

## On the beat

Enemies act on the music, Crypt of the NecroDancer style. Every timed component reads
the song beat from the level's `Conductor` each frame (a 120 BPM clock when there is no
Conductor, for standalone tests) and acts on its **cadence**: beats
`offset_beats + k * every_beats`. Before each action it winds up for `windup_beats`:
the body coils, an amber ring closes in on it and flashes white at the end, and its
attack reports pending `danger_shapes()` (`activates_in > 0`). On the action beat the
body pops.

The cadence is computed from the song beat, never by counting signals
(`core::beat_motion::CadenceTracker`):

- a new enemy waits 1 beat plus its wind-up before its first action;
- a backward jump (checkpoint rewind, seek) restarts the tracker at the new beat;
- a forward jump skips actions more than half a beat old instead of firing a burst.

Components that act emit `acted(action_beat, song_beat)` and expose `get_cadence()`
(`every`, `offset`, `windup`) and `get_windup()`.

## Movement

- **Beat surges** (chasers): speed peaks right on each beat (about 3x) and glides below
  average until the next (`surge`, averages 1).
- **Inertia**: velocity steers toward the desired heading with limited acceleration, so
  turns become arcs. Chasers approach on an arc (`arc` radians at range, straight up
  close), sway slightly with the beat and keep apart from other enemies (separation).
- **Hops** land on the beat: a crouch (flatten, lean back), airtime with a stretched
  body and a ground shadow, then a squash and a small overshoot on landing.
- **Turning lags**: `EnemyVisual` turns the body toward its aim or travel direction at a
  limited rate.

## Enemy types

Cadences in beats (`every` / `offset`, wind-up). Silhouettes are Kenney simple-space
sprites through the metal body shader, with the amber core ring.

| Enemy | Rule | Cadence | Silhouette | Introduced |
|---|---|---|---|---|
| Static shooter | One aimed shot at the nearest player each bar | 4 / 0, 1 | `enemy_C` | level 1 pool |
| Turret | Four shots, alternating cardinal and diagonal | 2 / 0, 0.75 | `enemy_E` | level 1 pool |
| **Pulser** | A ring of 14 shots with a 3-shot gap every bar; the gap turns a quarter each bar | 4 / 0, 1 | `enemy_D` | level 1 |
| **Hopper** | Hops toward the nearest player, landing just short of them with a shockwave (52 px) | 2 / 0, 1 | `enemy_B` | level 1 |
| Runner | Surges every beat toward the nearest player whose color does not match its shield | 1 / 0 | `ship_E` | level 2 pool |
| **Bouncer** | Steps one diagonal cell (56 px) per beat, bouncing off the arena edges | 1 / 0, 0.3 | `meteor_squareDetailedLarge` | level 2 |
| **Splitter** | Slow chaser; on death splits into two pieces, each with one shield in a different player's color | surges 1 / 0 | `enemy_A` (pieces `star_large`, hop every beat) | level 2 |
| Shotgun | Chases; fires a 3-shot fan where it faces on the off-bar | 4 / 2, 1 | `ship_sidesB` | level 3 pool |
| **Dasher** | Locks a lane toward a player, shows it for a beat, then dashes along it | 4 / 0, 1 | `ship_G` | level 3 |
| **Chameleon** | Slow chaser whose shield colors rotate through the players' colors every 2 bars | 8 / 0, 1 | `ship_J` | level 3 |
| Mine layer | Chases slowly, drops a mine each bar | 4 / 0, 0.5 | `ship_sidesA` | level 4 pool |
| **Lancer** | Aims a thin beam at a player for 2 beats (aim locked), then fires it for half a beat | 8 / 4, 2 | `ship_L` | level 4 |
| **Warden** | Each bar, every enemy inside its ring (220 px) gains a ward in its color; wards drop when it dies | 4 / 0, 1 | `ship_sidesD` | level 4 (always with a companion) |

The pressure rules (Pulser, Hopper, Bouncer, Dasher, Lancer) ask for dodging; the
shield rules (Splitter, Chameleon, Warden) ask the team to coordinate who attacks what.

### Las Huevas enemies

One per lyric reference, all in the `las-huevas` bonus level and the gym. Silhouettes
are game-icons.net glyphs (`godot/assets/game-icons/`, white body with a dark rim)
through the same metal shader; themed shots use the firing scene's `projectile_skin`
metadata (potatoes, pills, juice drops). Most are fragile (1-4 life, 0-1 shields) since
the level gives them lifetimes and sends many.

| Enemy | Lyric | Rule | Built from |
|---|---|---|---|
| Foco | "prendo el foco" | A light ring with two gaps each bar (to 230 px); gaps turn 45° | `ShockwaveComponent` 4 / 0, 1 |
| Papa | "patatas en tu cara" | Three potatoes every 2 beats | `ShotgunShooterComponent`, `TurnComponent` |
| Sable | "la fuerza de un Jedi" | Slow chaser; two blades (92 px) snap 45° round on every beat | `ChaserComponent`, `SpinBladeComponent` |
| Birra | "una birra" | Bounces diagonally; foam four ways every 2 beats | `BounceComponent`, `TurretShooterComponent` |
| Caja | "a la caja le pego" | Snare rings on beats 2 and 4 (150 px, three gaps) | `ShockwaveComponent` 2 / 1, 0.5 |
| Corazón | "mi corazón" | Chaser with a heartbeat ring (82 px) every 2 beats | `ChaserComponent`, `ShockwaveComponent` |
| Fiera | "dientes de una fiera" | Long lunging bite (64 px) every 2 beats | `HopComponent` |
| Pastilla | "pasta de Morfeo" | Shield swaps color every bar; aimed pill each bar | `ChaserComponent`, `ChameleonComponent`, `ShooterComponent` |
| Maestro | "me siento Yoda... enano y verde" | Tiny, green; hops every beat with a small bite | `HopComponent` (green metal) |
| Globo | "como Julio Verne" | Orbits the arena (a turn per 32 beats), sandbag mine each bar | `OrbitComponent`, `MineDropperComponent` |
| Pelota | "el Diego", "vine a jugar" | Long diagonal dribble step every beat | `BounceComponent` |
| Mano de Dios | "por el cielo" | Rises and falls onto you every bar (92 px slam) | `HopComponent` |
| Huevo | "los huevos" | Hatches three chicks (hop every beat) 8 beats in unless cracked | `FuseComponent` |
| Jeringa | "tráiganme suero, me inyectan" | Aims a lane, then injects along it | `DashComponent` |
| Oveja | "ser oveja" | Slow flock chaser | `ChaserComponent` |
| Abeja | "el enjambre" | Fast swarm chaser, one hit pops it | `ChaserComponent` |
| Hermanos | "dándole la mano" | Wards nearby enemies in its color | `ChaserComponent`, `WardComponent` |
| Cohete | "espacio sideral" | Blasts in from outside at a mismatched player | `ColorChaserComponent` |
| Micrófono | "algo de melodía" | Ring of 16 words with a turning gap each bar | `RingEmitterComponent` |
| Gota | "cual líquido" | Splits into three droplets | `ChaserComponent`, `SplitComponent` |
| Limón | "todos los cítricos" | Bursts into 12 juice shots 6 beats in unless squeezed | `ChaserComponent`, `FuseComponent` |

### Telegraphs

| Enemy | Wind-up |
|---|---|
| Shooter, Shotgun, Turret | Body coils and turns to the shot; pending shots reported |
| Pulser | A spoke per coming shot grows outward; the gap stays dark between two brackets |
| Hopper | Crouch; a dashed amber ring marks the landing spot |
| Bouncer | A small dashed ring marks the next cell |
| Dasher | A lane fills toward the dash beat with chevrons and a target ring |
| Lancer | A thin line brightens and flickers; firing is a white-hot beam in amber |
| Chameleon | A ring in the next color flickers in the beat before the change |
| Warden | Its dashed ring brightens; tethers link it to warded enemies |

## Components

| Component | Responsibility |
|---|---|
| `HealthComponent` | Life, shield layers, wards, damage flash, shield rings |
| `ContactDamageComponent` | 1 damage to players on body contact |
| `ChaserComponent` | Surge toward the nearest player (inertia, arc, sway, separation) |
| `ColorChaserComponent` | Same, toward the nearest player whose color mismatches the active shield |
| `TurnComponent` | Turns the sprite pivot toward the nearest player with lag |
| `HopComponent` | Beat hops with optional landing shockwave |
| `BounceComponent` | Diagonal cell steps with edge bounces (closed form in the beat) |
| `DashComponent` | Telegraphed lane dash; drifts between dashes |
| `ShooterComponent` | One aimed shot |
| `ShotgunShooterComponent` | Fan along the facing |
| `TurretShooterComponent` | Four-way, alternating cardinal and diagonal by volley index |
| `MineDropperComponent` | Drops a mine |
| `RingEmitterComponent` | Gapped ring whose gap turns each volley |
| `LanceComponent` | Telegraphed beam |
| `SplitComponent` | Spawns single-shield pieces on death (they inherit death listeners) |
| `ChameleonComponent` | Cycles shield colors by song beat |
| `WardComponent` | Grants and revokes wards |
| `ShockwaveComponent` | Expanding gapped ring on its cadence (drawn by itself) |
| `SpinBladeComponent` | Blades that turn a step on every beat (drawn by itself) |
| `FuseComponent` | Pops `fuse_beats` after spawning: a ring of shots and/or released enemies, then leaves |
| `OrbitComponent` | Circles the arena center |

`BaseEnemy.danger_shapes()` reports the contact circle plus every component's
`component_danger_shapes()` (landing circles, dash lanes, beams, pending shots).

## Leaving

An enemy with a `lifetime_beats` (set from the chart) blinks through its last 2 beats
and then leaves: a metal puff and the `left` signal, never `died`, so it is not a kill.
Fuses leave the same way after they pop.

## Projectiles and mines

- Projectile speed 100 px/s by default (the Pulser fires at 130), radius 12, 1 damage,
  freed off screen or after 30 s. Amber orb with a white-hot core and trail, or the
  firing enemy's `projectile_skin` texture spinning over an amber disc.
- Mines arm after 0.5 s, last 15 s, radius 10, 1 damage once armed.

## Feel (`EnemyVisual`)

Spawn pop (elastic scale-in), damage flash, recoil and muzzle flash per shot, wind-up
ring and coil, action pop, hop lift with ground shadow, squash/stretch from the
components' poses, shield-plate break animation, death explosion. The core ring is
steady: with many enemies alive, per-beat pulses on every body were noise, so the beat
shows through actions and wind-ups instead.
