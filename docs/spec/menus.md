# Menus

## Main Menu

### Device Selection

The main menu detects all available input devices (keyboard + connected gamepads) and displays them in a list. Players join devices, optionally split them, and hold to start.

### Controls

| Action | Keyboard | Gamepad |
|--------|----------|---------|
| Join / Unjoin | Enter | A or Start |
| Split toggle (2P per device) | Left / Right arrow | D-Pad Left / Right or Left Stick |
| Start game | Hold Enter (1.0s) | Hold A (1.0s) |

### Stick Input

- Threshold: 0.5 (axis must exceed to register direction)
- Reset: 0.3 (must drop below to allow new direction)
- Prevents repeated toggles from held stick

### Device States

Each device can be:
- **Not joined** — shown as "not joined"
- **Single** — one player per device
- **Split (2P)** — two players per device (keyboard: WASD + IJKL; gamepad: left stick + right stick)

### Start Flow

1. At least one device must be joined
2. Any joined device can hold their join button for 1.0 second
3. A progress bar fills during the hold
4. Releasing before 1.0s toggles the device back to unjoined
5. On completion, `GameConfig.players` is populated and the game scene loads

## Game Over Screen

Shown when all players are dead with no revival possible.

### Display
- Semi-transparent black overlay (75% opacity)
- "GAME OVER" title (red, 64pt)
- Final score (32pt)
- Two buttons: "Restart" and "Main Menu" (24pt)

### Controls
- "Restart" is focused by default
- D-Pad / Arrow Keys navigate between buttons
- A / Enter activates the focused button
- "Restart" reloads the game level with the same player config
- "Main Menu" returns to device selection
