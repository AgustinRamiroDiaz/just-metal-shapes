# Spawning

## Enemy Spawning

### Base Interval

The base spawn interval is 7.0 seconds. Each enemy type has a multiplier applied to this base.

| Enemy | Multiplier | Initial Interval |
|-------|------------|------------------|
| Static Shooter | 1.0x | 7.0s |
| Runner | 1.2x | 8.4s |
| Shotgun | 1.5x | 10.5s |
| Turret | 2.0x | 14.0s |
| Mine Layer | 2.5x | 17.5s |

### Difficulty Scaling

Spawn intervals decrease over time:

```
difficulty_factor = 1.0 - min(game_time / 300.0, 0.7)
effective_interval = base_interval * multiplier * difficulty_factor
```

| Time | Factor | Static Shooter Interval |
|------|--------|-------------------------|
| 0s | 1.0 | 7.0s |
| 60s | 0.8 | 5.6s |
| 150s | 0.5 | 3.5s |
| 300s+ | 0.3 | 2.1s (cap) |

The difficulty caps at 30% of original intervals after 5 minutes.

### Spawn Locations

**Inside viewport** (Static Shooter, Turret):
- Random position within viewport, 50px margin from edges
- A particle burst effect plays for 0.8 seconds before the enemy appears

**Outside viewport** (Shotgun, Runner, Mine Layer):
- Random angle on a circle centered on the viewport
- Circle radius: half the viewport diagonal + 50px
- Enemy appears immediately (no spawn effect)

### Multiplayer Scaling

Enemy `max_life` is multiplied by the total number of players at spawn time. This applies to all enemy types. With 1 player, values are unchanged.

## Player Spawning

Players spawn at 8 preset positions distributed across the viewport:

| Index | Position (relative to viewport) |
|-------|-------------------------------|
| 0 | (33.3%, 39.0%) |
| 1 | (33.3%, 61.2%) |
| 2 | (66.7%, 39.0%) |
| 3 | (66.7%, 61.2%) |
| 4 | (50.0%, 26.0%) |
| 5 | (50.0%, 70.3%) |
| 6 | (16.7%, 50.2%) |
| 7 | (83.3%, 50.2%) |

## Spawn Effect

Inside-viewport enemies get a particle burst before appearing:
- Texture: dirt_03.png
- 16 particles, sphere emission (8px radius)
- Omnidirectional, velocity 40-80 px/s
- Particle lifetime: 0.6s
- Emission duration: 0.8s (enemy appears after this)
- Explosiveness: 0.8 (near-instant burst)
