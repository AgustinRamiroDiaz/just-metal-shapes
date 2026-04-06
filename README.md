# Just Metal Shapes

`Just Metal Shapes` is a cooperative bullet-hell arena game blending the chaotic survival feel of **Just Shapes & Beats** with team-based combat inspired by **Full Metal Furies**.

## How to Play

### Controls

- **Keyboard Player 1:** WASD or Arrow Keys
- **Keyboard Player 2:** IJKL
- **Gamepads:** Up to 8 gamepads supported, each can be split into 2 players (left stick + right stick)

### Main Menu

1. Press **Enter** (keyboard) or **A** (gamepad) to join
2. Press **Left/Right** to toggle between single and split (2-player) mode per device
3. **Hold Enter/A** for 1 second to start the game

### Gameplay

- Move to dodge enemy projectiles and contact damage
- Enemies inside your **range ring** (the circle around your player) take continuous **lightning damage**
- Your lightning color must **match the enemy's shield color** to deal damage — coordinate with teammates
- You have **3 lives**; taking damage grants 3 seconds of invincibility (blinking)
- When downed, a nearby alive teammate can **revive you** by staying within 60px for 2 seconds
- Game ends when **all players are dead** simultaneously

## Game Architecture

### Game Flow

```
Main Menu (device selection) → Main Level (gameplay) → Game Over (restart / main menu)
```

### Enemies

| Type | Spawn | Movement | Attack | Health | Shield |
|------|-------|----------|--------|--------|--------|
| **Static Shooter** | Inside viewport (with particle effect) | Stationary | Single projectile at nearest player every 2s | 3.0 | Random player color |
| **Shotgun** | Outside viewport (circle edge) | Chases nearest player at 30px/s | 3-projectile fan every 3s | 3.0 | Random player color |
| **Turret** | Inside viewport (with particle effect) | Stationary | Alternates cardinal/diagonal 4-shot patterns every 2s | 2.0 | Orange |

- Enemy health scales with player count (health x number of players)
- Spawn rate increases over time (intervals shrink to 30% of base over 5 minutes)
- Shields must be depleted before health can be damaged; shield color must match the attacking player's color

### Player

- **Speed:** 220 px/s, clamped to viewport
- **Damage:** 1.0 DPS continuous to all enemies in range (140px radius)
- **Lightning effect:** Animated Line2D with spark textures, tinted to player color, only shown when damage actually lands
- **Lives:** 3, with invincibility frames on hit
- **Revival:** Dead players can be revived by nearby teammates

### Components (Enemy)

Enemies are built from reusable components:

- **HealthComponent** — HP, shields, color-matching logic, visual rings
- **ShooterComponent** — Base projectile firing (aimed at nearest player)
  - **ShotgunShooterComponent** — Fan of projectiles
  - **TurretShooterComponent** — Cardinal/diagonal alternating pattern
- **ChaserComponent** — Moves toward nearest alive player
- **TurnComponent** — Rotates sprite toward nearest player
- **ContactDamageComponent** — Deals damage on body collision

### Project Structure

```
scripts/
  game_config.gd          # Singleton: player configs, input types, colors
  game_manager.gd         # Game loop: spawning, scoring, difficulty, game over
  main_menu.gd            # Device selection and player joining
  player.gd               # Player movement, damage, lightning, death
  spawn_effect.gd         # Particle burst before enemy spawn
  player/
    revival_component.gd   # Teammate revival mechanic
  enemies/
    static_shooter_enemy.gd
    shotgun_enemy.gd
    turret_enemy.gd
    projectile.gd
    components/
      health_component.gd
      shooter_component.gd
      shotgun_shooter_component.gd
      turret_shooter_component.gd
      chaser_component.gd
      turn_component.gd
      contact_damage_component.gd

scenes/
  main_menu.tscn
  player.tscn
  static_shooter_enemy.tscn
  shotgun_enemy.tscn
  turret_enemy.tscn
  projectile.tscn
  spawn_effect.tscn

main_level.tscn            # Main game scene
project.godot              # Godot project config (720x768, Jolt physics)
```

## Design Goals

- Keep the kinetic readability and pressure of JS&B
- Add tactical co-op interactions through shield color matching and proximity-based damage
- Support 1-8 players with automatic difficulty scaling
