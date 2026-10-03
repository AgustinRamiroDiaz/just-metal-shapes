# Music credits

All tracks are from [Pixabay Music](https://pixabay.com/music/) and are used under the
[Pixabay Content License](https://pixabay.com/service/license-summary/) (free for
commercial and non-commercial use, no attribution required; credited here anyway).
Files were re-encoded from the original MP3 to Ogg Vorbis (`ffmpeg -c:a libvorbis -q:a 5`)
to keep the web build small.

| Song id | Title | Artist | Pixabay id | Source | License |
|---|---|---|---|---|---|
| `wonders-of-the-earth` | Wonders of the Earth | Grand Project (`grand_project`) | 550792 | `https://pixabay.com/music/...-wonders-of-the-earth-550792/` | Pixabay Content License |
| `celtic` | Celtic | Alex Morgan (`alex-morgan`) | 591333 | `https://pixabay.com/music/...-celtic-591333/` | Pixabay Content License |
| `surf-rock` | Surf Rock | Alex Morgan (`alex-morgan`) | 591326 | `https://pixabay.com/music/...-surf-rock-591326/` | Pixabay Content License |

The `...` in each URL is Pixabay's genre prefix, which the original download filenames
(`<artist>-<slug>-<id>.mp3`) do not record; searching Pixabay Music for the title and
artist finds the page with the matching id.

## Adding a track

1. Drop `godot/music/<song-id>.ogg` here (Ogg Vorbis, loop disabled on import).
2. Run `uv run --project devtools devtools/analyze_all.py` from the repository root to
   write `<song-id>.analysis.json`. If the BPM is a harmonic of the real tempo, pin it in
   `devtools/music_overrides.json` and rerun.
3. Add a row to the table above and a `LevelSpec` in `rust/src/level_catalog.rs`.
