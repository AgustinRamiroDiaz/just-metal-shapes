# Spawning

## Enemy Spawning

Enemies come from the level chart as `SpawnEnemy` events; nothing spawns on a timer.
`core/chart_gen.rs` places them on phrase boundaries (see `levels.md` for each level's
enemy pool):

| Section | Enemy spawns |
|---------|--------------|
| intro, outro, finale | none |
| breakdown | mirrored pairs (the co-op combat phase) |
| main | likelier when the current phrase is light on hazards |
| build | half as often as main |

Each event picks a scene from the level's `enemy_pool` (`params.variant`) and its warning
time is how long the spawn effect plays.

### Spawn Locations

`enemy_spawn.rs` places the enemy:

**Inside the arena** (static shooter, turret): at the event's arena position. A particle
burst plays for the event's telegraph time, then the enemy appears.

**Outside the arena** (shotgun, runner, mine layer): at `params.angle` on a circle around
the arena center, radius half the arena diagonal + 50px, and the enemy chases inward.

### Multiplayer Scaling

Enemy `max_life` is multiplied by the number of players at spawn time.

### Rewinds

A checkpoint rewind clears enemies, enemy projectiles and mines; the chart respawns
enemies as the song replays.

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
