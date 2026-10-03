# Game Overview

Just Metal Shapes is a cooperative rhythm bullet-hell for 1-8 local players, any of
whom can be AI bots. Each level is one song: hazards, enemy arrivals, arena pulses and
camera kicks are scheduled from an offline beat analysis of the track. Shielded enemies
need the matching player color to break, so teams coordinate positions.

## Game Flow

```
Title -> Level Select -> Lobby -> Level (countdown, song plays) -> Results
```

1. Pick a level and difficulty mode, then join devices and add bots in the lobby.
2. Survive the song. Hazards telegraph in the level's danger color before they hit.
3. Enemies inside a player's range ring take lightning damage once their shield color
   matches that player.
4. Downed players are revived by a teammate standing close for 2 seconds.
5. If every player is down, the song rewinds to the last checkpoint (Casual / Normal).
   Hardcore ends the run instead.
6. The level is cleared when the song ends; results show rank and score.

## Scoring

Score = 10 per second survived + 100 per enemy + 150 per revive + 5000 on clear
- 50 per hit taken - 1000 per rewind, scaled by level difficulty and mode
(`core/scoring.rs`).

| Rank | Requirement |
|------|-------------|
| S | Clear, no rewinds, under 1 hit per player per minute |
| A | Clear, no rewinds, under 3 hits per player per minute |
| B | Clear with at most one rewind |
| C | Any other clear |
| D | Not cleared |

## Display

- Resolution: 1280 x 720, stretch `canvas_items`, aspect `keep`
- Renderer: GL Compatibility (desktop and Web)
