#!/usr/bin/env python3
"""Transcribe a song's lyrics with word timings: RoFormer vocal separation, then Whisper.

    uv run --project devtools --extra ml devtools/lyrics.py godot/music/las-huevas.ogg --language es

1. Mel-Band RoFormer isolates the vocals into `build/<id>.vocals.wav` (reused when present;
   listen to it to judge the separation).
2. faster-whisper large-v3 transcribes the vocal stem with word timestamps.
3. Each word is placed on the beats of `<id>.analysis.json`.

Writes `godot/music/<id>.lyrics.json` and prints the lines for a quick read. Only run it on
songs with vocals: Whisper invents text on instrumentals.
"""

from __future__ import annotations

import argparse
import gc
import json
from pathlib import Path

import numpy as np

from analyze_beats import decode_audio, time_to_beat

ROOT = Path(__file__).resolve().parents[1]
SEPARATOR_MODEL = "vocals_mel_band_roformer.ckpt"  # best vocal SDR (12.6) in audio-separator's list
WHISPER_MODEL = "large-v3"


def separate_vocals(audio: Path) -> Path:
    out = ROOT / "build" / f"{audio.stem}.vocals.wav"
    if out.exists():
        return out
    import torch
    from audio_separator.separator import Separator

    separator = Separator(
        output_dir=str(out.parent),
        model_file_dir=str(Path.home() / ".cache" / "audio-separator"),
        output_single_stem="Vocals",
        output_format="WAV",
    )
    separator.load_model(SEPARATOR_MODEL)
    separator.separate(str(audio), {"Vocals": out.stem})
    del separator
    gc.collect()
    torch.cuda.empty_cache()  # free the separator before Whisper loads
    return out


def transcribe(vocals: Path, language: str | None) -> tuple[list[dict], str]:
    from faster_whisper import WhisperModel

    model = WhisperModel(WHISPER_MODEL, device="cuda", compute_type="float16")
    # Decode with FFmpeg ourselves: faster-whisper's PyAV decoder breaks on current PyAV.
    audio = decode_audio(vocals, sample_rate=16_000)
    segments, info = model.transcribe(audio, language=language, word_timestamps=True, vad_filter=True)
    words = []
    for segment in segments:
        print(f"[{segment.start:7.2f}] {segment.text.strip()}", flush=True)
        words += [{"word": w.word.strip(), "start": w.start, "end": w.end, "probability": w.probability}
                  for w in segment.words]
    return words, info.language


def words_on_grid(words: list[dict], beat_times: np.ndarray) -> list[dict]:
    beat_indices = np.arange(len(beat_times), dtype=float)
    return [
        {
            "word": w["word"],
            "timeSeconds": round(w["start"], 3),
            "durationSeconds": round(w["end"] - w["start"], 3),
            "beat": round(time_to_beat(w["start"], beat_times, beat_indices), 4),
            "probability": round(w["probability"], 3),
        }
        for w in words
    ]


def self_check() -> None:
    rows = words_on_grid([{"word": "hola", "start": 1.25, "end": 1.5, "probability": 0.9}], np.array([0.0, 0.5, 1.0, 1.5]))
    assert rows == [{"word": "hola", "timeSeconds": 1.25, "durationSeconds": 0.25, "beat": 2.5, "probability": 0.9}], rows


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("audio", type=Path, nargs="*")
    parser.add_argument("--language", help="ISO code such as 'es' (default: Whisper detects it)")
    parser.add_argument("--self-check", action="store_true", help="Run the offline self-check and exit")
    args = parser.parse_args()
    if args.self_check:
        self_check()
        print("self-check ok")
        return
    if not args.audio:
        parser.error("give the songs with vocals to transcribe")

    for audio in args.audio:
        analysis = json.loads(audio.with_name(f"{audio.stem}.analysis.json").read_text(encoding="utf-8"))
        vocals = separate_vocals(audio)
        words, language = transcribe(vocals, args.language)
        output = audio.with_name(f"{audio.stem}.lyrics.json")
        output.write_text(json.dumps({
            "source": audio.name,
            "model": f"{SEPARATOR_MODEL} + whisper-{WHISPER_MODEL}",
            "language": language,
            "words": words_on_grid(words, np.array(analysis["beatTimesSeconds"])),
        }, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"Wrote {output}:{len(words)} words, language={language}", flush=True)


if __name__ == "__main__":
    main()
