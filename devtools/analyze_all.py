#!/usr/bin/env python3
"""Analyze every audio file in godot/music/.

Run from the repository root after adding or replacing a track:
    uv run --project devtools devtools/analyze_all.py           # only stale/missing
    uv run --project devtools devtools/analyze_all.py --force   # everything

Each `godot/music/<id>.<ext>` gets a `godot/music/<id>.analysis.json` next to it.
The song id is the file stem, which is what `LevelSpec.analysis_path` points at.
Extra arguments after `--` are forwarded to analyze_beats.py (e.g. `-- --bpm 128`).

Per-track fixes live in devtools/music_overrides.json, keyed by song id:
    {"<id>": {"bpm": 140, "offset": 0.0, "minBpm": 70, "maxBpm": 180}}
Use them when the tempo estimate locks onto a harmonic of the real tempo.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MUSIC_DIR = ROOT / "godot" / "music"
ANALYZER = Path(__file__).resolve().parent / "analyze_beats.py"
OVERRIDES = Path(__file__).resolve().parent / "music_overrides.json"
OVERRIDE_FLAGS = {"bpm": "--bpm", "offset": "--offset", "minBpm": "--min-bpm", "maxBpm": "--max-bpm"}
AUDIO_EXTENSIONS = {".ogg", ".mp3", ".wav", ".flac"}


def load_overrides() -> dict[str, dict[str, object]]:
    if not OVERRIDES.exists():
        return {}
    return json.loads(OVERRIDES.read_text(encoding="utf-8"))


def override_args(song_overrides: dict[str, object]) -> list[str]:
    args: list[str] = []
    for key, flag in OVERRIDE_FLAGS.items():
        if key in song_overrides:
            args += [flag, str(song_overrides[key])]
    return args


def is_stale(audio_path: Path, output_path: Path) -> bool:
    if not output_path.exists():
        return True
    inputs = [audio_path, ANALYZER] + ([OVERRIDES] if OVERRIDES.exists() else [])
    return output_path.stat().st_mtime < max(path.stat().st_mtime for path in inputs)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--force", action="store_true", help="Re-analyze every track")
    parser.add_argument("extra", nargs="*", help="Arguments forwarded to analyze_beats.py")
    args = parser.parse_args()

    tracks = sorted(
        path for path in MUSIC_DIR.iterdir() if path.suffix.lower() in AUDIO_EXTENSIONS
    )
    if not tracks:
        print(f"No audio files in {MUSIC_DIR.relative_to(ROOT)}")
        return

    overrides = load_overrides()
    for audio_path in tracks:
        output_path = audio_path.with_name(f"{audio_path.stem}.analysis.json")
        if not args.force and not is_stale(audio_path, output_path):
            print(f"Up to date: {output_path.relative_to(ROOT)}")
            continue
        subprocess.run(
            [
                sys.executable,
                str(ANALYZER),
                str(audio_path),
                "--output",
                str(output_path),
                *override_args(overrides.get(audio_path.stem, {})),
                *args.extra,
            ],
            check=True,
        )
        data = json.loads(output_path.read_text(encoding="utf-8"))
        section_types = " ".join(section["type"] for section in data["sections"])
        print(
            f"Wrote {output_path.relative_to(ROOT)}: "
            f"bpm={data['bpm']:.2f} confidence={data['confidence']:.3f} "
            f"duration={data['durationSeconds']:.1f}s beats={len(data['beats'])} "
            f"sections=[{section_types}]"
        )


if __name__ == "__main__":
    main()
