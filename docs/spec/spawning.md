# Spawning

## Enemy Spawning

Enemies come from the level chart as `SpawnEnemy` events; nothing spawns on a timer.
`core/chart_gen.rs` places them on phrase boundaries (see `levels.md` for each level's
enemy pool):

| Section | Enemy spawns |
|---------|--------------|
| intro, outro, finale | none |
| breakdown | a wave (the co-op combat phase): 2 different enemies (3 from difficulty 4), one per bar; difficulty 1 sends one |
| main | single enemies, likelier when the current phrase is light on hazards |
| build | half as often as main |

Each event picks a scene from the level's `enemy_pool` (`params.variant`) and its warning
time is how long the spawn effect plays. `params.count` above 1 spawns a group: the
first at the position and the rest on a ring 46 px around it (outside spawners fan out
by 0.22 rad). `params.duration_beats` above 0 is each enemy's lifetime
(`BaseEnemy.lifetime_beats`): it blinks for its last 2 beats, then leaves without
counting as a kill. Scripted levels place their enemies from lyric cues instead (see
`levels.md`).

- **Introductions**: pool entries marked `intro` (new in that level) arrive first, one
  at a time and in pool order, before the pool mixes.
- **Support** entries (the Warden) never arrive alone: a companion from the rest of the
  pool spawns with them.
- **Thinning**: when arrivals come within 16 beats of each other (a wave, or singles
  close together), hazards hitting on every other hit beat are dropped from the first
  of those arrivals until 16 beats after the last (whole beats at a time, so mirrored
  groups stay whole), keeping the safe path open while several enemies are alive.

### Spawn Locations

`enemy_spawn.rs` places the enemy:

**Inside the arena** (every type but the outside chasers): at the event's arena
position. A particle burst plays for the event's telegraph time, then the enemy appears.

**Outside the arena** (shotgun, runner, mine layer): at `params.angle` on a circle around
the arena center, radius half the arena diagonal + 50px, and the enemy chases inward.

**Splitter pieces** appear where the Splitter died, 34 px apart, and inherit its death
listeners (kill counts).

### Multiplayer Scaling

Enemy `max_life` is multiplied by the number of players at spawn time.

### Rewinds

A checkpoint rewind clears enemies, enemy projectiles and mines; the chart respawns
enemies as the song replays. Enemy timing comes from the song beat, so respawned enemies
act on the same beats as the first time (see `enemies.md`).

## Player Spawning

Players spawn at 8 preset positions relative to the viewport, in seat order:

| Index | Position |
|-------|----------|
| 0 | (40%, 42%) |
| 1 | (40%, 58%) |
| 2 | (60%, 42%) |
| 3 | (60%, 58%) |
| 4 | (50%, 30%) |
| 5 | (50%, 70%) |
| 6 | (30%, 50%) |
| 7 | (70%, 50%) |
