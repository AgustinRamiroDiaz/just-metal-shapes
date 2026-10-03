# Hazards and phrases

Chart hazards are Rust nodes in `rust/src/hazards/`, one file per `EventKind`. They follow
the hazard contract in `docs/design/complete-game-plan.md`: harmless while telegraphing,
damaging once active, `danger_shapes()` for bots, free themselves when done.

## Shared look

Every hazard uses `HazardCore` and `paint.rs`, so they read as one family:

- **Warning**: translucent fill plus outline in the level's danger color
  (`Palette::danger`, neon pink by default). The fill grows toward the hit and flickers
  in the last quarter of the telegraph.
- **Hit**: a white flash (`IMPACT_FLASH`, 0.07 s) and a scale pop that settles in about
  0.2 s.
- **Active**: solid danger color with a white core, brightened on every beat
  (`beat_pulse`). Bullet hazards fade over the last 12% (`FADE_START`) and stop hurting
  while they fade.

All geometry is a closed-form function of song time (`geometry.rs`, unit tested), so
hazards stay correct after frame drops, pauses and seeks. Bullet hazards draw and test
every bullet from a single node.

## Kinds

Units follow `PatternParams`: positions `0..1` of the arena, `size` as a fraction of
arena height, `speed` in arena heights per second.

| Kind | Telegraph | Active | Params | Danger shapes |
|---|---|---|---|---|
| Laser | Strip widening toward the hit | Full-length beam, narrows away after 70% | `x`,`y` point, `angle`, `size` width | Capsule |
| LaserSweep | Start beam plus a rotation arc or push chevrons | Rotation: quarter turn around (`x`,`y`). Translation (`variant` bit 0): pushes toward the center, at most 45% of the arena | `variant` bit 1 = counter-clockwise, `speed` (translation) | Capsule at the current pose |
| BulletRing | Swelling emitter with spokes | Shockwave, `count` bullets fly outward | `angle` first bullet, `variant` 1 = gap around `angle` | Emitter plus pre-shifted bullets, then live bullets with velocity |
| Spiral | Emitter with slowly turning arm indicators | `count` arms fire every quarter beat for 55% of the duration, turning a quarter turn per beat | `variant` 1 = counter-clockwise | Emitter, live bullets |
| Wall | Entry-edge band with the gap left dark and bracketed, chevrons | Slab pair slides edge to edge over the active phase | `angle` snapped to a cardinal direction; gap center `y` (horizontal travel) or `x` (vertical); gap is 30% of arena height | Two Rects with velocity |
| Pulse | Ring plus fill growing to the edge, closing outer ring | Burst with shockwave, shrinks away after 70% | `size` radius | Circle |
| Bomb | Bomb falls onto the target, blast circle fills, fuse blinks faster | 0.3 s blast plus `count` shrapnel bullets | `size` blast radius, `speed` shrapnel | Blast circle, shrapnel |
| Spikes | Shaded reach band, spike outlines, tips peeking | Thrust (overshoot), hold, retract | `variant % 4` side (left, top, right, bottom), `variant & 4` every other spike, `count`, `size` depth | One capsule per spike |
| Barrage | Dashed aim lines and tightening crosshairs on living players (captured at spawn) | `count` shots, an eighth beat apart, cycling through targets with a slight fan | `x`,`y` emitter (usually an edge) | Aim-lane capsules, then bullets (unfired ones pending) |

Moving shapes that have not started yet are reported shifted back along their velocity
by `activates_in`, so `DangerShape::advanced(activates_in)` lands exactly where they will
start.

## Phrases (choreography)

`core::chart_gen` cuts each section into phrases of `PHRASE_BARS` (4) bars, aligned to
the section start. A novelty spike at least two bars in starts the next phrase early.
Intros longer than `INTRO_MAX_BARS` (8) continue as a build. Each phrase plays one
`Phrase`, picked by section type (`Phrase::default_weight`), the level's `phrases`
emphasis and which kinds the level's pool allows. The same phrase rarely plays twice in
a row. Main sections favor opening with a wall and builds with a spiral riser.

| Phrase | Kinds | Choreography |
|---|---|---|
| Breather | Pulse/Laser | One hit on strong accents, at most one per bar (breakdowns, intros, outros) |
| PulseGrid | Pulse | Corners-then-center, a march across alternating rows, or point-mirrored pairs |
| LaserCallResponse | Laser | Alternating sides, the response mirrors the call, closing in toward the center |
| RingsOnKicks | BulletRing | A ring on every kick or downbeat, alternating between two mirrored origins; successive rings interleave; gapped early on |
| SweepingWall | Wall | A wall at the phrase start; a mirrored wall answers from the other side two bars later |
| SpiralRiser | Spiral | A spiral every two bars, alternating spin and gaining an arm; counter-rotating pairs late in hard builds |
| BarrageSnares | Barrage | Aimed volleys on snare hits from alternating edges |
| BombPairs | Bomb | Point-symmetric bomb pairs on downbeats, keeping the middle lane open |
| SpikeSides | Spikes | Spikes from opposite sides in turn; comb layout early in a section |
| SweepCross | LaserSweep | A rotating sweep from the center (a counter-rotating cross on hard levels), then a beam pushing in from an edge |
| Scatter | any | Weighted random picks from the pool on the phrase grid |
| Finale | any | Boss set piece: layers two phrases per phrase (`FINALE_LAYERS`) |

Phrases that center on one long hazard (wall, spiral, sweep) add accent fills: pulses or
lasers on strong accents, and on downbeats once the phrase runs hot.

**Heat** sets how often a phrase hits (every bar, half bar or beat):
`density x section shape x (0.7 + 0.6 x bar intensity)`. Builds rise through the
section, outros fall, mains and finales run hot. `density` combines difficulty, the
level's `density` and the difficulty mode.

## Safe path and pacing rules

- Proposals are committed in time order. Summed `coverage` of active hazards stays at or
  below `coverage_cap(difficulty)` (0.30 to 0.50) and at most
  `max_active_hazards(difficulty)` (3 to 7) are active at once. Mirrored groups commit
  all-or-nothing, so symmetry never breaks.
- No hazard hits before beat 4 or on a silent beat; every hazard ends 0.5 s before the
  song does.
- The first hazard of each kind gets one extra beat of telegraph.
- Enemies arrive on phrase boundaries: pairs (mirrored) during breakdowns; in main
  sections more often when the phrase is light on hazards (at least one per main
  section); half as often in builds; never in intros, outros or the finale.

## Tests

- `cargo test`: per-phrase invariants (`chart_gen` tests), safe path and concurrency
  per difficulty, the finale, enemy placement, determinism; geometry in
  `hazards/geometry.rs`; per-level safe path in every mode (`level_catalog`).
- `cargo test level_stats -- --ignored --nocapture` prints each level's hazards per
  kind, pressure and phrase plan.
- `make e2e ONLY=hazard_kinds`: each kind spawned on its own via `spawn_event`. Its
  telegraph is harmless, its active phase damages, it reports shapes, and it frees itself.
- `make e2e ONLY=levels_run`: every level's chart plays to the end at 12x with
  god-mode players and reports events per kind.
