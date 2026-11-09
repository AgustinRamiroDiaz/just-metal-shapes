#!/usr/bin/env python3
"""Render a `<id>.notes.json` transcription to audio, to check it by ear.

    uv run --project devtools --extra ml devtools/render_notes.py godot/music/surf-rock.notes.json
    uv run --project devtools --extra ml devtools/render_notes.py godot/music/surf-rock.notes.json --only drums

Writes `build/<id>.notes.wav` (outside godot/ so Godot doesn't import it): the original song
on the left channel and the synthesized notes on the right, level-matched. Pan or mute one
side to compare. Needs `fluidsynth` on PATH; the MuseScore General SoundFont is downloaded
once on first use.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from muscriptor.tokenizer.mt3 import MT3_FULL_PLUS_GROUP_NAMES, MT3Tokenizer
from muscriptor.tokenizer.notes import DRUM_PROGRAM, Note, trim_overlapping_notes, validate_notes
from muscriptor.utils.auralization import auralize
from muscriptor.utils.midi import notes_to_midi

ROOT = Path(__file__).resolve().parents[1]
AUDIO_EXTENSIONS = {".ogg", ".mp3", ".wav", ".flac"}


def instrument_programs() -> dict[str, int]:
    """Instrument group name -> the General MIDI program MuScriptor uses for it."""
    groups = MT3Tokenizer(instrument_vocabulary="MT3_FULL_PLUS", max_shift_steps=1001).group_program_map
    programs = {name: groups[gid][0] for name, gid in MT3_FULL_PLUS_GROUP_NAMES.items() if groups.get(gid)}
    return programs | {"drums": DRUM_PROGRAM}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("notes", type=Path)
    parser.add_argument("--only", help="Comma-separated instruments to keep, e.g. 'drums,electric_bass'")
    args = parser.parse_args()

    data = json.loads(args.notes.read_text(encoding="utf-8"))
    keep = set(args.only.split(",")) if args.only else None
    programs = instrument_programs()
    notes = [
        Note(
            is_drum=instrument == "drums",
            program=programs[instrument],
            onset=row["timeSeconds"],
            offset=row["timeSeconds"] + row["durationSeconds"],
            pitch=row["pitch"],
        )
        for instrument, rows in data["instruments"].items()
        if keep is None or instrument in keep
        for row in rows
    ]
    if not notes:
        parser.error(f"no notes left; instruments in file: {', '.join(data['instruments'])}")

    song = args.notes.name.removesuffix(".notes.json")
    audio = next(path for path in args.notes.parent.glob(f"{song}.*") if path.suffix in AUDIO_EXTENSIONS)
    out_dir = ROOT / "build"
    out_dir.mkdir(exist_ok=True)
    suffix = f".{args.only.replace(',', '+')}" if args.only else ""
    midi_path = out_dir / f"{song}{suffix}.notes.mid"
    wav_path = out_dir / f"{song}{suffix}.notes.wav"
    notes = trim_overlapping_notes(validate_notes(notes, fix=True), sort=True)
    program_names = {programs[name]: name.replace("_", " ") for name in data["instruments"]}
    notes_to_midi(notes, program_names=program_names).save(str(midi_path))
    auralize(midi_path, audio, wav_path)
    print(f"Wrote {wav_path.relative_to(ROOT)} (left: original, right: transcription) and {midi_path.name}")


if __name__ == "__main__":
    main()
