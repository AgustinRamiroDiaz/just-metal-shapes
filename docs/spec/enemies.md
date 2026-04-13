# Enemies

All enemies use the same base script (`base_enemy.gd`) with behavior defined by attached components. Every enemy has a `HealthComponent` and `ContactDamageComponent` (1 damage on touch).

## Shield System

- Enemies can have a colored shield (depletes before HP)
- Player lightning must match the shield color to deal damage
- Shield is a 0-1 ratio; once depleted, health takes damage regardless of color
- Shield color is randomly assigned to one of the active player colors at spawn (except turret, which has a fixed orange shield)

## Enemy Types

### Static Shooter

Stationary enemy that fires aimed projectiles at the nearest player.

| Stat | Value |
|------|-------|
| HP | 3.0 |
| Shield | Yes (random player color) |
| Collision radius | 24 px |
| Sprite scale | 0.2x |
| Color | White |
| Spawn type | Inside viewport |
| Spawn interval multiplier | 1.0x |
| Shoot interval | 2.0s |
| Shoot pattern | Single aimed projectile |

### Shotgun Enemy

Chases players and fires a fan of projectiles in the direction it faces.

| Stat | Value |
|------|-------|
| HP | 3.0 |
| Shield | Yes (light blue) |
| Collision radius | 32 px |
| Sprite scale | 0.3x |
| Color | Red (0.8, 0.2, 0.2) |
| Spawn type | Outside viewport |
| Spawn interval multiplier | 1.5x |
| Chase speed | 30 px/s |
| Turn speed | 1.0 |
| Shoot interval | 3.0s |
| Shot count | 3 |
| Spread angle | 45 degrees |

### Turret Enemy

Stationary enemy that fires in alternating cross patterns.

| Stat | Value |
|------|-------|
| HP | 2.0 |
| Shield | Yes (orange, fixed) |
| Collision radius | 24 px |
| Sprite scale | 0.2x |
| Color | Purple (0.6, 0.2, 0.8) |
| Spawn type | Inside viewport |
| Spawn interval multiplier | 2.0x |
| Shoot interval | 2.0s |
| Pattern | Alternates cardinal (N/S/E/W) and diagonal |

### Runner Enemy

Fast enemy that chases players whose color doesn't match its shield, forcing the "wrong" player to dodge while the matching player must close in to deal damage.

| Stat | Value |
|------|-------|
| HP | 2.0 |
| Shield | No |
| Collision radius | 20 px |
| Sprite scale | 0.18x |
| Color | Green (0.2, 0.8, 0.2) |
| Spawn type | Outside viewport |
| Spawn interval multiplier | 1.2x |
| Chase speed | 60 px/s |
| Turn speed | 2.0 |
| Targeting | Nearest player with mismatched color |

### Mine Layer Enemy

Slow enemy that leaves a trail of mines as it chases players.

| Stat | Value |
|------|-------|
| HP | 4.0 |
| Shield | No |
| Collision radius | 26 px |
| Sprite scale | 0.22x |
| Color | Orange-yellow (1.0, 0.6, 0.1) |
| Spawn type | Outside viewport |
| Spawn interval multiplier | 2.5x |
| Chase speed | 20 px/s |
| Mine drop interval | 2.5s |

## Projectiles

- Speed: 100 px/s
- Lifetime: 30s (also despawns on screen exit)
- Collision radius: 12 px
- Damage: 1 per hit
- Visual: yellow circle with orange arc outline

## Mines

- Arm time: 0.5s (gray and inert until armed)
- Lifetime: 15s
- Collision radius: 10 px
- Damage: 1 per hit (only when armed)
- Visual: pulsing red-orange glow when armed (6 Hz)
