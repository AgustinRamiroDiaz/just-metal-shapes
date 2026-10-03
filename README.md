# Just Metal Shapes

A cooperative rhythm bullet-hell for 1-8 local players, blending the music-driven
dodging of **Just Shapes & Beats** with the color-coordinated team combat of
**Full Metal Furies**. Any seat can be filled by an AI bot.

Every level is one song. Lasers, bullet rings, sweeping walls, bombs and spikes are
scheduled from a beat analysis of the track and telegraph before they hit; shielded
enemies arrive on phrase boundaries and only break to the player whose color matches
their shield.

## How to Play

1. **Title → Level Select**: pick a level (later levels unlock as you clear earlier
   ones) and a mode: Casual, Normal or Hardcore.
2. **Lobby**: join with keyboard or gamepads, split a device into two players, and add
   bots.
3. **Survive the song**. Enemies inside your range ring take lightning damage when your
   color matches their shield. Revive downed teammates by standing next to them for
   2 seconds. If everyone goes down, the song rewinds to the last checkpoint (Hardcore:
   game over).

### Controls

| | Keyboard | Gamepad |
|---|---|---|
| Move | WASD / Arrows (P1), IJKL (P2 when split) | Left stick (right stick when split) |
| Pause | Esc | Start |
| Menus | Arrows, Enter, Esc | D-pad / stick, A, B |

Lobby: Enter / A join, Left / Right split, B / X add bot, Backspace / Y remove bot,
Tab / RB bot skill, hold Space / Start to begin. Full details in `docs/spec/menus.md`.

### Levels

| # | Level | Music |
|---|---|---|
| 1 | Wonders of the Earth (tutorial) | Grand Project (Pixabay) |
| 2 | Voxel Revolution | Kevin MacLeod (CC BY 4.0) |
| 3 | Celtic | Alex Morgan (Pixabay) |
| 4 | Ouroboros | Kevin MacLeod (CC BY 4.0) |
| 5 | Surf Rock (finale) | Alex Morgan (Pixabay) |

## Development

Godot 4.6+ with all gameplay, UI and tooling in a Rust GDExtension (`rust/`, gdext).
See `CONTRIBUTING.md` for toolchain setup and Web export.

| Command | What it does |
|---|---|
| `make build` | Build the native Rust extension |
| `make test` | Rust unit tests (`rust/src/core/`: chart generation, timing, danger geometry, bots, scoring, saves) |
| `make e2e` | Headless Godot end-to-end scenarios (`godot/tests/e2e/`), silent; fails on panics or leaks |
| `make test-all` | Both |
| `make screenshots` | Capture every UI screen and staged gameplay moments off-screen (`xvfb-run`, no audio) to `/tmp/jms_shots` |
| `make analyze-music` | Re-run beat analysis for every track in `godot/music/` |

Bots play whole levels in e2e: `godot --headless --path godot -s res://tests/run_e2e.gd -- --only=bot_full_level --bot-level=surf-rock`.

### Docs

- `docs/design/complete-game-plan.md`: system contracts (Conductor, chart, hazards, danger records, bots, UX flow)
- `docs/architecture.md`: code layout and how the pieces connect
- `docs/spec/`: gameplay specs (overview, player, enemies, hazards, levels, spawning, menus)
- `docs/CREDITS.md` and `godot/music/CREDITS.md`: asset and music licenses

### Adding a song

Drop an `.ogg` into `godot/music/`, run `make analyze-music`, credit it in
`godot/music/CREDITS.md`, and add a `LevelSpec` to `rust/src/level_catalog.rs`
(see "Adding a level" in `docs/spec/levels.md`).
