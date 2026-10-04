# Menus

All screens are Rust `Control` classes in `rust/src/ui/`, styled by one project theme,
and fully usable with keyboard or any gamepad (mouse also works). Back is Esc / B on
every screen.

```
Title -> Level Select -> Lobby -> Level (3-2-1 countdown) -> Results
Title -> Gym -> Lobby -> Gym arena
Title -> Settings | Credits
Pause (in level) -> Resume | Restart | Settings | Quit to level select
```

## Title

Play, Gym, Settings, Credits, Quit (desktop only). The menu track loops and the title pulses
on its beat.

## Level Select

One card per catalog level: title, artist, difficulty pips, BPM, best rank and score,
lock state. A focused card previews its song. Difficulty mode (Casual / Normal /
Hardcore) is chosen here. Level N+1 unlocks when N is cleared; debug builds and the
"Unlock all levels" setting unlock everything.

## Lobby

Seats for up to 8 players, any mix of humans and bots.

| Action | Keyboard | Gamepad |
|--------|----------|---------|
| Join | Enter | A |
| Split device into 2 players | Left / Right | D-pad or left stick |
| Add bot | B | X |
| Remove bot | Backspace | Y |
| Cycle bot skill (easy / normal / hard) | Tab | RB |
| Leave (or back when not joined) | Esc | B |
| Start | Hold Space or Enter | Hold Start or A |

Entered from Gym, the lobby says so and Back returns to the title.

A bots-only run can be started from an unjoined device. Seats are named `P1`, `P2`, ...
and `BOT 1`, `BOT 2`, ..., each with a unique color.

## In Level

HUD: song progress bar with section and checkpoint ticks, team score, checkpoint and
rewind toasts, tutorial hints on the first level. Lives and names are drawn on the
players (see `player.md`), leaving the bottom of the arena free. Esc / Start
pauses; resuming runs a short countdown.

## Gym

A sandbox for trying enemies (`ui/gym.rs`), with the lobby's players and bots:

- The song of the last selected level (first level by default) plays and loops. No
  chart: no hazards, checkpoints, hints or countdown, and no score or results.
- A sidebar on the right lists every enemy type (`level_catalog::ENEMY_KINDS`) with
  its sprite and one-line rule, in a scrolling column. Drag a card into the arena with
  the mouse; the enemy spawns at the drop point after a one-beat spawn effect, with
  health scaled to the player count as in levels.
- Tools: **Clear** (removes enemies, shots and mines), **God** (players take no
  damage; off by default), **Hide** or H (collapses the sidebar; a "Gym (H)" tab brings
  it back), **Song** (switches to the next level's song, so cadences can be tried at
  other tempos).
- When every player is down they all respawn in place.
- Pause works as in levels; Quit goes to the title.

## Results

Rank (S/A/B/C/D), score, time, enemies destroyed, revives, hits taken, rewinds, and a
new-best badge. Buttons: Next level (after a clear), Retry, Level select. A Hardcore wipe
shows the game-over variant.

## Settings

Persisted in `user://save.json` by the `SaveData` autoload: master / music / effects
volume, screen shake, reduce flashing, audio offset (with calibration), fullscreen
(desktop), show FPS (also shows the debug overlay in debug builds), unlock all levels.

## Credits

Lists every asset and music credit, including the CC BY attributions the Kevin MacLeod
tracks require (`rust/src/core/credits.rs`, kept in sync with `docs/CREDITS.md`).
