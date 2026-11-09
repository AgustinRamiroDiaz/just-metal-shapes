#!/usr/bin/env python3
"""Compare each song's beat grid (analysis.json) with the Beat This! neural tracker.

    uv run --project devtools --extra ml devtools/compare_beats.py [song-id ...]

For each grid, `on/off` is the mean onset strength on the beats divided by the strength
halfway between them: above ~1.5 the grid sits on the pulse, below 1 it is on the
off-beats. The metric uses analyze_beats' own onset envelope, which our grid's phase was
fitted to, so a Beat This! win is the stronger signal. `downbeats` is the share of Beat
This! downbeats that land on one of our bar starts.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
from beat_this.inference import File2Beats

import analyze_beats as ab

MUSIC_DIR = Path(__file__).resolve().parents[1] / "godot" / "music"
FRAME_RATE = ab.SAMPLE_RATE / ab.HOP_LENGTH
AUDIO_EXTENSIONS = {".ogg", ".mp3", ".wav", ".flac"}


def on_off_ratio(envelope: np.ndarray, times: np.ndarray) -> float:
    def peak_near(points: np.ndarray) -> np.ndarray:
        frames = (points * FRAME_RATE).astype(int)
        return np.array([envelope[max(0, f - 2) : f + 3].max() for f in frames if f + 3 < len(envelope)])

    return float(peak_near(times).mean() / peak_near((times[:-1] + times[1:]) / 2).mean())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("songs", nargs="*", help="Song ids (default: every analyzed song)")
    args = parser.parse_args()
    ids = args.songs or sorted(path.name.removesuffix(".analysis.json") for path in MUSIC_DIR.glob("*.analysis.json"))

    tracker = File2Beats(checkpoint_path="final0", dbn=False)
    print(f"{'song':24} {'bpm ours':>8} {'bpm BT':>7} {'on/off ours':>11} {'on/off BT':>9} {'downbeats':>9}")
    for song in ids:
        audio = next(path for path in MUSIC_DIR.glob(f"{song}.*") if path.suffix in AUDIO_EXTENSIONS)
        analysis = json.loads((MUSIC_DIR / f"{song}.analysis.json").read_text(encoding="utf-8"))
        ours = np.array(analysis["beatTimesSeconds"])
        beats, downbeats = tracker(str(audio))
        envelope = ab.onset_envelope(ab.decode_audio(audio))
        bt_bpm = 60 / np.polyfit(np.arange(len(beats)), beats, 1)[0]
        bar_starts = ours[:: ab.BEATS_PER_BAR]
        agree = np.mean([np.abs(bar_starts - t).min() < 0.07 for t in downbeats]) if len(downbeats) else 0.0
        print(
            f"{song:24} {analysis['bpm']:8.2f} {bt_bpm:7.2f} "
            f"{on_off_ratio(envelope, ours):11.2f} {on_off_ratio(envelope, beats):9.2f} {agree:9.0%}"
        )


if __name__ == "__main__":
    main()
