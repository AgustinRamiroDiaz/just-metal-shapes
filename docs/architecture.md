# Architecture: Composition Over Inheritance

This project uses a **composition-based architecture** where behavior is assembled from independent, reusable components attached to scene nodes. There are no inheritance hierarchies beyond what Godot requires (extending built-in node types).

## Core Principle

Every entity in the game is defined by **what components it has**, not by what class it extends. A single `BaseEnemy` script handles all enemies. The difference between a turret and a shotgun enemy is entirely determined by which component nodes are attached in the scene editor.

## Enemy System

### BaseEnemy

All enemies use `base_enemy.gd` directly. It provides the minimal shared contract:

- `died` signal
- Wires `HealthComponent.died` to `queue_free()`
- Exposes `take_damage()` that delegates to `HealthComponent`

There are no per-enemy scripts. Enemy behavior is defined purely by scene composition.

### Example: How Enemies Are Built

```
ShotgunEnemy (StaticBody2D + base_enemy.gd)
  ├── Sprite2D
  ├── CollisionShape2D
  ├── HealthComponent            (HP, shields, color matching)
  ├── ShotgunShooterComponent    (fires fan of projectiles on timer)
  ├── ChaserComponent            (moves toward nearest player)
  ├── TurnComponent              (rotates sprite toward target)
  └── ContactDamageComponent     (deals damage on body collision)
```

```
MineLayerEnemy (StaticBody2D + base_enemy.gd)
  ├── Sprite2D
  ├── CollisionShape2D
  ├── HealthComponent
  ├── ChaserComponent
  ├── MineDropperComponent       (drops mines on timer)
  └── ContactDamageComponent
```

To create a new enemy type, you create a new `.tscn` scene, attach `base_enemy.gd`, and add whichever components define its behavior. No code changes needed.

### Available Enemy Components

| Component | Responsibility |
|---|---|
| `HealthComponent` | HP, shield with color matching, damage flash, visual rings |
| `ShooterComponent` | Fires single aimed projectile at nearest player on timer |
| `ShotgunShooterComponent` | Fires fan of projectiles based on sprite rotation |
| `TurretShooterComponent` | Fires alternating cardinal/diagonal projectile patterns |
| `ChaserComponent` | Moves toward nearest alive player |
| `ColorChaserComponent` | Moves toward nearest player whose color doesn't match its shield |
| `TurnComponent` | Rotates sprite toward nearest player |
| `MineDropperComponent` | Drops mines at current position on timer |
| `ContactDamageComponent` | Deals damage to players on body contact |

### Component Independence

Components are designed to be self-contained:

- Each component manages its own state and timing (using `Timer` nodes)
- Components read from the scene tree (groups, parent position) rather than referencing siblings directly
- When a component needs optional context from a sibling (e.g., `ColorChaserComponent` reading shield color from `HealthComponent`), it uses `get_node_or_null()` in `_ready()` and falls back to defaults

## Player System

The player follows the same principle. The `Player` script handles core mechanics (movement, input, damage, range tiers), while extracted components handle specific subsystems:

```
Player (CharacterBody2D + player.gd)
  ├── Sprite2D
  ├── CollisionShape2D
  ├── RevivalComponent         (teammate revival mechanic)
  ├── LightningComponent       (visual lightning rays to damaged targets)
  └── RangeArea                (Area2D detecting enemies in range)
```

## Game Management

The game loop is also split by responsibility:

```
Main Level (Node2D + game_manager.gd)
  ├── ScoreLabel
  └── EnemySpawner             (spawn timing, difficulty scaling, placement)
```

`GameManager` handles game state (score, game over, player setup, UI). `EnemySpawner` handles all spawn logic independently and communicates via the `enemy_died` signal.

## Shared Utilities

Static utility classes avoid duplicating logic across components:

| Utility | Purpose |
|---|---|
| `Targeting` | `get_nearest_alive(tree, origin, group)` — shared nearest-target lookup with dead-player filtering |
| `ColorUtils` | `colors_match(c1, c2)` — RGB color comparison used by shield and chaser systems |

## Adding New Behavior

### New enemy type
1. Create a `.tscn` scene
2. Set the root to `StaticBody2D` with `base_enemy.gd`
3. Add a `HealthComponent` and whichever behavior components you need
4. Register it in `EnemySpawner`

### New enemy component
1. Create a script extending `Node` (or `Node2D` if it draws)
2. Use `Timer` nodes for periodic actions
3. Use `Targeting.get_nearest_alive()` for player lookups
4. Read parent position via `(get_parent() as Node2D).global_position`
5. Attach it to any enemy scene that needs the behavior

### New player component
1. Create a script in `scripts/player/`
2. Add the node to `scenes/player.tscn`
3. Access it from `player.gd` via `@onready`
