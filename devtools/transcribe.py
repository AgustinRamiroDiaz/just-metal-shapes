#!/usr/bin/env python3
"""Transcribe a song into per-instrument notes with MuScriptor, on the song's beat grid.

Needs the `ml` extra, a GPU for reasonable speed, and a HuggingFace login with the
MuScriptor license accepted (the weights are gated, CC BY-NC 4.0):
    uvx hf auth login
    uv run --project devtools --extra ml devtools/transcribe.py              # every song without notes
    uv run --project devtools --extra ml devtools/transcribe.py godot/music/surf-rock.ogg

Writes `godot/music/<id>.notes.json` next to the audio. Note times are mapped onto the
beats of `<id>.analysis.json` (run analyze_all.py first), so `beat` and `quantizedBeat`
line up with the beats the game already uses.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
from pathlib import Path

import numpy as np

from analyze_beats import time_to_beat

MUSIC_DIR = Path(__file__).resolve().parents[1] / "godot" / "music"
AUDIO_EXTENSIONS = {".ogg", ".mp3", ".wav", ".flac"}
QUANTIZE_STEPS_PER_BEAT = 4  # sixteenth notes
# General MIDI percussion pitches the game is likely to care about.
DRUM_NAMES = {
    35: "kick", 36: "kick",
    37: "snare", 38: "snare", 40: "snare",
    42: "hihat", 44: "hihat", 46: "hihat-open",
    41: "tom", 43: "tom", 45: "tom", 47: "tom", 48: "tom", 50: "tom",
    49: "crash", 57: "crash", 51: "ride", 59: "ride", 53: "ride",
}


def pair_events(events: list[dict]) -> list[dict]:
    """Join MuScriptor's start/end event stream into notes with a duration."""
    starts = {event["index"]: event for event in events if event["type"] == "start"}
    notes = []
    for event in events:
        if event["type"] != "end":
            continue
        start = starts[event["start_event_index"]]
        notes.append({
            "instrument": start["instrument"],
            "pitch": start["pitch"],
            "start": start["start_time"],
            "end": event["end_time"],
        })
    return sorted(notes, key=lambda note: (note["start"], note["pitch"]))


def notes_on_grid(notes: list[dict], beat_times: np.ndarray) -> dict[str, list[dict]]:
    beat_indices = np.arange(len(beat_times), dtype=float)
    by_instrument: dict[str, list[dict]] = {}
    for note in notes:
        beat = time_to_beat(note["start"], beat_times, beat_indices)
        row = {
            "timeSeconds": round(note["start"], 4),
            "durationSeconds": round(note["end"] - note["start"], 4),
            "pitch": note["pitch"],
            "beat": round(beat, 4),
            "quantizedBeat": round(beat * QUANTIZE_STEPS_PER_BEAT) / QUANTIZE_STEPS_PER_BEAT,
        }
        if note["instrument"] == "drums":
            row["drum"] = DRUM_NAMES.get(note["pitch"], "other")
        by_instrument.setdefault(note["instrument"], []).append(row)
    return by_instrument


def load_muscriptor(size: str, dtype: str):
    import torch
    from muscriptor.transcription_model import TranscriptionModel

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    # load_model builds fp32 on the target device before casting, which needs 2x the fp32
    # weights (11 GB for large). Loading on CPU and moving the cast model fits a 6 GB GPU.
    model = TranscriptionModel.load_model(size, device="cpu", dtype=dtype)
    model._model.to(device)
    model._device = device
    for module in model._model.modules():  # conditioners keep their device as a plain attribute
        if isinstance(getattr(module, "device", None), (torch.device, str)):
            module.device = device
    return model


def run_muscriptor(model, audio: Path, instruments: str | None) -> list[dict]:
    from muscriptor.events import NoteEndEvent, NoteStartEvent

    events = []
    for event in model.transcribe(str(audio), instruments=instruments.split(",") if instruments else None):
        if isinstance(event, NoteStartEvent):
            events.append({"type": "start", **dataclasses.asdict(event)})
        elif isinstance(event, NoteEndEvent):
            events.append({"type": "end", "end_time": event.end_time, "start_event_index": event.start_event_index})
    return events


def self_check() -> None:
    events = [
        {"type": "start", "pitch": 36, "start_time": 1.0, "index": 0, "instrument": "drums"},
        {"type": "start", "pitch": 40, "start_time": 1.26, "index": 1, "instrument": "electric_bass"},
        {"type": "end", "end_time": 1.0, "start_event_index": 0},
        {"type": "end", "end_time": 1.5, "start_event_index": 1},
    ]
    grid = notes_on_grid(pair_events(events), np.array([0.0, 0.5, 1.0, 1.5, 2.0]))
    assert grid["drums"] == [{"timeSeconds": 1.0, "durationSeconds": 0.0, "pitch": 36,
                              "beat": 2.0, "quantizedBeat": 2.0, "drum": "kick"}], grid
    assert grid["electric_bass"][0]["quantizedBeat"] == 2.5, grid
    assert grid["electric_bass"][0]["durationSeconds"] == 0.24, grid


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("audio", type=Path, nargs="*", help="Audio files (default: every analyzed song without notes)")
    parser.add_argument("--force", action="store_true", help="With no audio given, also redo songs that have notes")
    parser.add_argument("--model", default="large", help="small (~100M), medium (~300M) or large (~1.3B)")
    parser.add_argument("--dtype", default="float16", help="float16 (3.3 GB VRAM for large) or float32")
    parser.add_argument("--instruments", help="Restrict to these instrument groups, e.g. 'drums,elec,dist'")
    parser.add_argument("--self-check", action="store_true", help="Run the offline self-check and exit")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        print("self-check ok")
        return
    songs = args.audio or [
        audio
        for audio in sorted(MUSIC_DIR.iterdir())
        if audio.suffix in AUDIO_EXTENSIONS
        and audio.with_name(f"{audio.stem}.analysis.json").exists()
        and (args.force or not audio.with_name(f"{audio.stem}.notes.json").exists())
    ]
    if not songs:
        print("Every analyzed song already has notes (use --force to redo them)")
        return

    model = load_muscriptor(args.model, args.dtype)
    for audio in songs:
        analysis_path = audio.with_name(f"{audio.stem}.analysis.json")
        beat_times = np.array(json.loads(analysis_path.read_text(encoding="utf-8"))["beatTimesSeconds"])
        instruments = notes_on_grid(pair_events(run_muscriptor(model, audio, args.instruments)), beat_times)

        output = audio.with_name(f"{audio.stem}.notes.json")
        output.write_text(json.dumps({
            "source": audio.name,
            "model": f"muscriptor-{args.model}-{args.dtype}",
            "instruments": instruments,
        }, indent=1) + "\n", encoding="utf-8")
        summary = ", ".join(f"{name}={len(rows)}" for name, rows in instruments.items())
        print(f"Wrote {output}: {summary}", flush=True)


if __name__ == "__main__":
    main()
