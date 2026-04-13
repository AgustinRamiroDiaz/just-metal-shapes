# Player

## Movement

- Speed: 220 px/s
- Clamped to viewport bounds
- Joystick deadzone: 0.2

## Combat

### Range Tiers

The player's attack range (140px radius) is divided into 3 equal tiers. Enemies closer to the player take more damage, visualized by multiple lightning rays.

| Tier | Radius | Ray Count | Effective DPS |
|------|--------|-----------|---------------|
| Inner | 0 - 46.7 px | 3 | 3.0 |
| Middle | 46.7 - 93.3 px | 2 | 2.0 |
| Outer | 93.3 - 140 px | 1 | 1.0 |

- Base damage: 1.0 DPS per ray
- Damage is continuous (applied every frame via delta)
- Lightning only appears when damage actually lands (shield color must match)

### Lightning Visual

- Animated Line2D with spark textures cycling at 12 FPS
- Width: 80px, tinted to player's team color at 0.8 alpha
- Multiple rays are spaced 10px apart perpendicular to the target direction

### Range Visual

- 3 concentric filled circles, dimmer toward the outside
- Inner tier alpha: 0.12, middle: 0.08, outer: 0.04
- No outlines

## Health

- Max lives: 3
- Invincibility after hit: 3.0 seconds (visual blink at 6 Hz)
- Death: grayed out sprite, emits `died` signal

## Revival

- A nearby alive player within 60px can revive a dead player
- Revival takes 2.0 seconds of continuous proximity
- Revived player gets 1 life and 3 seconds of invincibility
- Progress shown as white arc around dead player

## Input

### Keyboard Player 1
- WASD or Arrow Keys

### Keyboard Player 2
- IJKL

### Gamepad
- Up to 8 gamepads, each can be split into left stick + right stick (2 players per pad)
- Deadzone: 0.2 with linear rescaling

## Team Colors

Colors are assigned in order as players join:

1. Light Blue (0.35, 0.75, 1.0)
2. Orange (1.0, 0.6, 0.2)
3. Light Green (0.4, 0.9, 0.3)
4. Magenta (0.9, 0.3, 0.9)
5. Yellow (1.0, 0.9, 0.15)
6. Red (0.9, 0.3, 0.3)
7. Cyan (0.4, 0.9, 0.85)
8. Pink (1.0, 0.6, 0.75)
