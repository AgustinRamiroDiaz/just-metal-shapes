# Music credits

| Level | Song id | Title | Artist | License |
|---|---|---|---|---|
| 1 | `wonders-of-the-earth` | Wonders of the Earth | Grand Project | Pixabay Content License |
| 2 | `voxel-revolution` | Voxel Revolution | Kevin MacLeod | CC BY 4.0 |
| 3 | `celtic` | Celtic | Alex Morgan | Pixabay Content License |
| 4 | `ouroboros` | Ouroboros | Kevin MacLeod | CC BY 4.0 |
| 5 | `surf-rock` | Surf Rock | Alex Morgan | Pixabay Content License |
| Bonus | `las-huevas` | Las Huevas (en vivo) | Banzai FC ft. Wos | Used with permission |
| Hidden | `las-huevas-full` | Las Huevas (en vivo) | Banzai FC ft. Wos | Used with permission |

## Kevin MacLeod (incompetech.com), CC BY 4.0

| Song id | Title | Source |
|---|---|---|
| `voxel-revolution` | Voxel Revolution | https://incompetech.com/music/royalty-free/index.html?isrc=USUAN2000025 |
| `ouroboros` | Ouroboros | https://incompetech.com/music/royalty-free/index.html?isrc=USUAN1400007 |

Both were re-encoded from the incompetech MP3 to Ogg Vorbis without other edits.
Attribution is required and must be shown in the game's credits:

> "Voxel Revolution" Kevin MacLeod (incompetech.com)
> Licensed under Creative Commons: By Attribution 4.0 License
> http://creativecommons.org/licenses/by/4.0/

> "Ouroboros" Kevin MacLeod (incompetech.com)
> Licensed under Creative Commons: By Attribution 4.0 License
> http://creativecommons.org/licenses/by/4.0/

## Pixabay Music

These tracks are from [Pixabay Music](https://pixabay.com/music/) and are used under the
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

## Las Huevas (bonus level)

"Las Huevas" by Banzai FC featuring Wos, recorded live at Centro Cultural Konex (2017),
is used with permission. Both files are re-encoded from the supplied MP3 to Ogg Vorbis
(`-q:a 4`):

- `las-huevas-full`: the whole 9-minute recording (the hidden level).
- `las-huevas`: its first 3:27 with a 2.5 s fade-out (`-t 207 -af
  "afade=t=out:st=204.5:d=2.5"`): the intro, the first verse and the start of the jam.

The band's tempo drifts (live), so both analyses use a tracked tempo map (`trackTempo`
in `devtools/music_overrides.json`); the cut also sets `outroBars`. Lyric cues for both
levels live in `las-huevas.cues.json`.

## Adding a track

1. Drop `godot/music/<song-id>.ogg` here (Ogg Vorbis, loop disabled on import).
2. Run `uv run --project devtools devtools/analyze_all.py` from the repository root to
   write `<song-id>.analysis.json`. If the BPM is a harmonic of the real tempo, pin it in
   `devtools/music_overrides.json` and rerun; if the tempo drifts (live recordings), set
   `"trackTempo": true` there.
3. Add a row to the table above and a `LevelSpec` in `rust/src/level_catalog.rs`.
