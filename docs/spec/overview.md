# Game Overview

Just Metal Shapes is a cooperative bullet-hell arena game for 1-8 players. Players dodge enemy projectiles and use proximity-based lightning damage to destroy enemies. Shields on enemies require color-matching coordination between teammates.

## Game Flow

```
Main Menu (device selection) --> Game (arena survival) --> Game Over (restart / menu)
```

1. Players join from the main menu by pressing Enter (keyboard) or A (gamepad)
2. The game spawns players in preset positions and begins spawning enemies
3. Difficulty escalates over time (enemies spawn faster)
4. Game ends when all players are dead simultaneously
5. Players can restart with the same config or return to the main menu

## Scoring

- 10 points per enemy killed
- Score displayed at top-left of screen

## Display

- Resolution: 720 x 768
- Window mode: maximized
- Stretch mode: canvas_items (expand aspect)
- Physics engine: Jolt
