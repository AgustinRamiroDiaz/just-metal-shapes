#!/usr/bin/env python3
"""Extract a fixed-tempo musical feature map from an audio file.

Depends only on Python, NumPy, and FFmpeg. The JSON output contains beat timing,
loudness, onsets, frequency-band energy, novelty, bars, and sections, and is the
input format of `rust/src/core/analysis.rs` (`SongAnalysis`).

Usage (from the repository root):
    uv run --project devtools devtools/analyze_beats.py godot/music/celtic.ogg \
        --output godot/music/celtic.analysis.json

Set FFMPEG=/path/to/ffmpeg to pick a specific FFmpeg binary.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path

import numpy as np


SAMPLE_RATE = 22_050
HOP_LENGTH = 256
FRAME_LENGTH = 1_024
BEATS_PER_BAR = 4


def robust_normalize(values: np.ndarray) -> np.ndarray:
    low, high = np.percentile(values, [5, 95])
    if high <= low:
        return np.zeros_like(values)
    return np.clip((values - low) / (high - low), 0, 1)


def decode_audio(path: Path) -> np.ndarray:
    result = subprocess.run(
        [
            os.environ.get("FFMPEG", "ffmpeg"),
            "-v",
            "error",
            "-i",
            str(path),
            "-ac",
            "1",
            "-ar",
            str(SAMPLE_RATE),
            "-f",
            "f32le",
            "pipe:1",
        ],
        check=True,
        stdout=subprocess.PIPE,
    )
    return np.frombuffer(result.stdout, dtype="<f4")


def onset_envelope(audio: np.ndarray) -> np.ndarray:
    if len(audio) < FRAME_LENGTH:
        raise ValueError("Audio is too short to analyze")

    frame_count = 1 + (len(audio) - FRAME_LENGTH) // HOP_LENGTH
    frames = np.lib.stride_tricks.as_strided(
        audio,
        shape=(frame_count, FRAME_LENGTH),
        strides=(audio.strides[0] * HOP_LENGTH, audio.strides[0]),
        writeable=False,
    )
    spectrum = np.abs(np.fft.rfft(frames * np.hanning(FRAME_LENGTH), axis=1))
    spectrum = np.log1p(10 * spectrum)
    flux = np.maximum(0, np.diff(spectrum, axis=0)).mean(axis=1)
    flux = np.concatenate(([0.0], flux))

    # Subtract a local mean so sustained loud sections do not dominate beats.
    local_window = max(3, round(0.35 * SAMPLE_RATE / HOP_LENGTH))
    local_mean = np.convolve(flux, np.ones(local_window) / local_window, mode="same")
    envelope = np.maximum(0, flux - local_mean)
    scale = np.percentile(envelope, 95)
    return envelope / scale if scale > 0 else envelope


def frame_features(audio: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    frame_count = 1 + (len(audio) - FRAME_LENGTH) // HOP_LENGTH
    frames = np.lib.stride_tricks.as_strided(
        audio,
        shape=(frame_count, FRAME_LENGTH),
        strides=(audio.strides[0] * HOP_LENGTH, audio.strides[0]),
        writeable=False,
    )
    windowed = frames * np.hanning(FRAME_LENGTH)
    power = np.abs(np.fft.rfft(windowed, axis=1)) ** 2
    frequencies = np.fft.rfftfreq(FRAME_LENGTH, 1 / SAMPLE_RATE)
    low = np.log1p(power[:, frequencies < 180].mean(axis=1))
    mid = np.log1p(power[:, (frequencies >= 180) & (frequencies < 2_000)].mean(axis=1))
    high = np.log1p(power[:, frequencies >= 2_000].mean(axis=1))
    rms_db = 20 * np.log10(np.maximum(np.sqrt(np.mean(frames**2, axis=1)), 1e-8))
    return rms_db, low, mid, high


def aggregate_at_beats(
    audio: np.ndarray,
    envelope: np.ndarray,
    beat_times: np.ndarray,
    seconds_per_beat: float,
) -> list[dict[str, object]]:
    frame_rate = SAMPLE_RATE / HOP_LENGTH
    rms_db, low, mid, high = frame_features(audio)
    frame_times = (np.arange(len(rms_db)) * HOP_LENGTH + FRAME_LENGTH / 2) / SAMPLE_RATE
    rows: list[dict[str, float]] = []

    for beat, beat_time in enumerate(beat_times):
        end_time = min(len(audio) / SAMPLE_RATE, beat_time + seconds_per_beat)
        interval = (frame_times >= beat_time) & (frame_times < end_time)
        onset_window = np.abs(np.arange(len(envelope)) / frame_rate - beat_time) <= seconds_per_beat * 0.4
        if not np.any(interval):
            continue
        rows.append({
            "beat": float(beat),
            "timeSeconds": float(beat_time),
            "loudnessDb": float(np.mean(rms_db[interval])),
            "onsetRaw": float(np.max(envelope[onset_window])) if np.any(onset_window) else 0.0,
            "lowRaw": float(np.mean(low[interval])),
            "midRaw": float(np.mean(mid[interval])),
            "highRaw": float(np.mean(high[interval])),
        })

    loudness = robust_normalize(np.array([row["loudnessDb"] for row in rows]))
    onset = robust_normalize(np.array([row["onsetRaw"] for row in rows]))
    low_energy = robust_normalize(np.array([row["lowRaw"] for row in rows]))
    mid_energy = robust_normalize(np.array([row["midRaw"] for row in rows]))
    high_energy = robust_normalize(np.array([row["highRaw"] for row in rows]))
    feature_matrix = np.column_stack((loudness, onset, low_energy, mid_energy, high_energy))
    novelty_raw = np.linalg.norm(np.diff(feature_matrix, axis=0, prepend=feature_matrix[:1]), axis=1)
    novelty = robust_normalize(novelty_raw)
    silence_db = max(float(np.percentile([row["loudnessDb"] for row in rows], 10) + 2), -38.0)

    beats: list[dict[str, object]] = []
    for index, row in enumerate(rows):
        band_mean = (low_energy[index] + mid_energy[index] + high_energy[index]) / 3
        accent = np.clip(onset[index] * 0.55 + loudness[index] * 0.25 + band_mean * 0.2, 0, 1)
        beats.append({
            "beat": int(row["beat"]),
            "timeSeconds": round(row["timeSeconds"], 6),
            "loudnessDb": round(row["loudnessDb"], 3),
            "loudness": round(float(loudness[index]), 4),
            "onsetStrength": round(float(onset[index]), 4),
            "lowEnergy": round(float(low_energy[index]), 4),
            "midEnergy": round(float(mid_energy[index]), 4),
            "highEnergy": round(float(high_energy[index]), 4),
            "novelty": round(float(novelty[index]), 4),
            "accent": round(float(accent), 4),
            "silent": bool(row["loudnessDb"] <= silence_db and loudness[index] < 0.16),
        })
    return beats


def extract_quantized_onsets(
    envelope: np.ndarray,
    bpm: float,
    beat_offset: float,
    duration: float,
) -> list[dict[str, object]]:
    frame_rate = SAMPLE_RATE / HOP_LENGTH
    threshold = float(np.percentile(envelope, 88))
    events: dict[float, tuple[float, float]] = {}
    for index in range(1, len(envelope) - 1):
        strength = float(envelope[index])
        if strength < threshold or strength < envelope[index - 1] or strength < envelope[index + 1]:
            continue
        time_seconds = index / frame_rate
        beat = (time_seconds - beat_offset) * bpm / 60
        quantized_beat = round(beat * 2) / 2
        quantized_time = beat_offset + quantized_beat * 60 / bpm
        if quantized_beat < 0 or quantized_time >= duration:
            continue
        current = events.get(quantized_beat)
        if current is None or strength > current[1]:
            events[quantized_beat] = (time_seconds, strength)

    strengths = np.array([event[1] for event in events.values()])
    normalized = robust_normalize(strengths) if len(strengths) else strengths
    return [
        {
            "timeSeconds": round(events[beat][0], 6),
            "quantizedBeat": beat,
            "strength": round(float(normalized[index]), 4),
        }
        for index, beat in enumerate(sorted(events))
    ]


def aggregate_bars(
    beats: list[dict[str, object]],
    onsets: list[dict[str, object]],
) -> list[dict[str, object]]:
    bars: list[dict[str, object]] = []
    for start in range(0, len(beats), BEATS_PER_BAR):
        bar_beats = beats[start : start + BEATS_PER_BAR]
        if len(bar_beats) < BEATS_PER_BAR:
            break
        start_beat = int(bar_beats[0]["beat"])
        end_beat = start_beat + BEATS_PER_BAR
        bar_onsets = [event for event in onsets if start_beat <= float(event["quantizedBeat"]) < end_beat]
        intensity = float(np.mean([
            float(beat["loudness"]) * 0.5 + float(beat["onsetStrength"]) * 0.3 + float(beat["accent"]) * 0.2
            for beat in bar_beats
        ]))
        strongest = max(bar_beats, key=lambda beat: float(beat["accent"]))
        bars.append({
            "bar": len(bars),
            "startBeat": start_beat,
            "timeSeconds": bar_beats[0]["timeSeconds"],
            "intensity": round(intensity, 4),
            "onsetCount": len(bar_onsets),
            "strongestBeat": strongest["beat"],
            "silent": all(bool(beat["silent"]) for beat in bar_beats),
        })

    onset_density = robust_normalize(np.array([float(bar["onsetCount"]) for bar in bars]))
    for index, bar in enumerate(bars):
        bar["onsetDensity"] = round(float(onset_density[index]), 4)
    feature_matrix = np.array([[float(bar["intensity"]), float(bar["onsetDensity"])] for bar in bars])
    novelty = robust_normalize(np.linalg.norm(np.diff(feature_matrix, axis=0, prepend=feature_matrix[:1]), axis=1))
    for index, bar in enumerate(bars):
        bar["novelty"] = round(float(novelty[index]), 4)
    return bars


def infer_sections(bars: list[dict[str, object]]) -> list[dict[str, object]]:
    if not bars:
        return []
    boundaries = [0]
    candidates = sorted(
        range(1, len(bars)),
        key=lambda index: float(bars[index]["novelty"]),
        reverse=True,
    )
    for index in candidates:
        if float(bars[index]["novelty"]) < 0.62:
            break
        if all(abs(index - boundary) >= 4 for boundary in boundaries):
            boundaries.append(index)
    boundaries = sorted(boundaries) + [len(bars)]
    sections: list[dict[str, object]] = []
    global_intensity = float(np.median([bar["intensity"] for bar in bars]))
    for section_index, (start, end) in enumerate(zip(boundaries, boundaries[1:])):
        intensity = float(np.mean([bar["intensity"] for bar in bars[start:end]]))
        if section_index == 0:
            section_type = "intro"
        elif end == len(bars):
            section_type = "outro"
        elif intensity < global_intensity * 0.72:
            section_type = "breakdown"
        elif intensity > float(np.mean([bar["intensity"] for bar in bars[max(0, start - 4):start]])) * 1.12:
            section_type = "build"
        else:
            section_type = "main"
        sections.append({
            "startBar": start,
            "endBar": end,
            "startBeat": int(bars[start]["startBeat"]),
            "endBeat": int(bars[end - 1]["startBeat"]) + BEATS_PER_BAR,
            "type": section_type,
            "intensity": round(intensity, 4),
        })
    return sections


def estimate_tempo(envelope: np.ndarray, min_bpm: float, max_bpm: float) -> tuple[float, float]:
    frame_rate = SAMPLE_RATE / HOP_LENGTH
    centered = envelope - envelope.mean()
    correlation = np.correlate(centered, centered, mode="full")[len(centered) - 1 :]
    min_lag = max(1, round(frame_rate * 60 / max_bpm))
    max_lag = min(len(correlation) - 1, round(frame_rate * 60 / min_bpm))
    lags = np.arange(min_lag, max_lag + 1)

    # Favor the musically common middle of the requested range while retaining
    # the actual autocorrelation peak. Harmonics at 2x/0.5x otherwise dominate.
    bpms = 60 * frame_rate / lags
    prior = np.exp(-0.5 * (np.log2(bpms / 120) / 0.7) ** 2)
    scores = correlation[lags] * prior
    best_index = int(np.argmax(scores))
    best_lag = int(lags[best_index])

    # Parabolic peak interpolation yields a less quantized BPM estimate.
    fractional_lag = float(best_lag)
    if 0 < best_lag < len(correlation) - 1:
        left, center, right = correlation[best_lag - 1 : best_lag + 2]
        denominator = left - 2 * center + right
        if denominator != 0:
            fractional_lag += float(0.5 * (left - right) / denominator)

    confidence = float(correlation[best_lag] / max(correlation[0], 1e-9))
    return 60 * frame_rate / fractional_lag, confidence


def estimate_phase(envelope: np.ndarray, bpm: float) -> float:
    frame_rate = SAMPLE_RATE / HOP_LENGTH
    period_frames = frame_rate * 60 / bpm
    phase_candidates = np.linspace(0, period_frames, 512, endpoint=False)
    beat_indices = np.arange(len(envelope), dtype=float)

    # Fold onset energy into one beat period, with a narrow Gaussian tolerance.
    sigma = max(1.0, period_frames * 0.055)
    scores = []
    for phase in phase_candidates:
        distance = np.abs((beat_indices - phase + period_frames / 2) % period_frames - period_frames / 2)
        scores.append(float(np.sum(envelope * np.exp(-0.5 * (distance / sigma) ** 2))))

    phase_frames = float(phase_candidates[int(np.argmax(scores))])
    return phase_frames / frame_rate


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("audio", type=Path)
    parser.add_argument("--min-bpm", type=float, default=70)
    parser.add_argument("--max-bpm", type=float, default=180)
    parser.add_argument("--bpm", type=float, help="Use a known fixed BPM after estimating the beat phase")
    parser.add_argument("--offset", type=float, help="Override the detected first-beat offset in seconds")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    audio = decode_audio(args.audio)
    envelope = onset_envelope(audio)
    estimated_bpm, confidence = estimate_tempo(envelope, args.min_bpm, args.max_bpm)
    bpm = args.bpm if args.bpm is not None else estimated_bpm
    estimated_beat_offset = estimate_phase(envelope, bpm)
    beat_offset = args.offset if args.offset is not None else estimated_beat_offset
    duration = len(audio) / SAMPLE_RATE
    seconds_per_beat = 60 / bpm
    beat_times = np.arange(beat_offset, duration, seconds_per_beat)
    beats = aggregate_at_beats(audio, envelope, beat_times, seconds_per_beat)
    onsets = extract_quantized_onsets(envelope, bpm, beat_offset, duration)
    bars = aggregate_bars(beats, onsets)
    sections = infer_sections(bars)

    result = {
        "source": args.audio.name,
        "durationSeconds": round(duration, 6),
        "bpm": round(bpm, 6),
        "estimatedBpm": round(estimated_bpm, 6),
        "beatOffsetSeconds": round(beat_offset, 6),
        "estimatedBeatOffsetSeconds": round(estimated_beat_offset, 6),
        "confidence": round(confidence, 6),
        "beatTimesSeconds": [round(float(value), 6) for value in beat_times],
        "beats": beats,
        "onsets": onsets,
        "bars": bars,
        "sections": sections,
    }
    rendered = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")


if __name__ == "__main__":
    main()
