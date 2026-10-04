#!/usr/bin/env python3
"""Fetch the game-icons.net glyphs used by enemy sprites and rasterize them.

Run from the repository root:
    uv run --project devtools devtools/fetch_icons.py

Each icon in `ICONS` is downloaded from the game-icons repository (CC BY 3.0, see
`docs/CREDITS.md`), restyled for `enemy_body.gdshader` (white body with a dark rim,
no background square) into `devtools/icons/<name>.svg`, then rasterized to
`godot/assets/game-icons/<name>.png` (128 px) by a headless, silent Godot run.
"""

from __future__ import annotations

import os
import re
import subprocess
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SVG_DIR = Path(__file__).resolve().parent / "icons"
PNG_DIR = ROOT / "godot" / "assets" / "game-icons"
SOURCE = "https://raw.githubusercontent.com/game-icons/icons/master/{}.svg"
SIZE = 128
RIM = "#3a3f55"
RIM_WIDTH = 22

# Sprite name -> game-icons path (author/icon).
ICONS = {
    "light_bulb": "lorc/light-bulb",
    "potato": "delapouite/potato",
    "light_sabers": "delapouite/light-sabers",
    "beer_bottle": "delapouite/beer-bottle",
    "drum": "delapouite/drum",
    "heart_beats": "delapouite/heart-beats",
    "fangs": "skoll/fangs",
    "pill": "lorc/pill",
    "hood": "lorc/hood",
    "air_balloon": "delapouite/air-balloon",
    "soccer_ball": "delapouite/soccer-ball",
    "hand_of_god": "delapouite/hand-of-god",
    "egg": "sbed/big-egg",
    "chicken": "delapouite/chicken",
    "syringe": "lorc/syringe",
    "sheep": "delapouite/sheep",
    "bee": "lorc/bee",
    "shaking_hands": "delapouite/shaking-hands",
    "rocket": "lorc/rocket",
    "microphone": "delapouite/microphone",
    "drop": "lorc/drop",
    "lemon": "delapouite/lemon",
}


def restyle(svg: str) -> str:
    """Drops the black background square; draws each glyph path twice: a dark rim
    (stroked) under a white body."""
    body = re.findall(r'<path[^>]*\sd="([^"]+)"', svg)
    glyphs = [d for d in body if not re.fullmatch(r"M0 0h512v512H0z", d)]
    rim = "".join(
        f'<path d="{d}" fill="{RIM}" stroke="{RIM}" stroke-width="{RIM_WIDTH}" '
        'stroke-linejoin="round"/>'
        for d in glyphs
    )
    fill = "".join(f'<path d="{d}" fill="#fff"/>' for d in glyphs)
    pad = RIM_WIDTH
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{-pad} {-pad} {512 + 2 * pad} '
        f'{512 + 2 * pad}">{rim}{fill}</svg>\n'
    )


def main() -> None:
    SVG_DIR.mkdir(parents=True, exist_ok=True)
    PNG_DIR.mkdir(parents=True, exist_ok=True)
    for name, path in ICONS.items():
        target = SVG_DIR / f"{name}.svg"
        if not target.exists():
            with urllib.request.urlopen(SOURCE.format(path)) as response:
                target.write_text(restyle(response.read().decode()), encoding="utf-8")
            print(f"Fetched {path} -> {target.relative_to(ROOT)}")
    godot = os.environ.get("GODOT_BIN", "godot")
    subprocess.run(
        [
            godot, "--headless", "--audio-driver", "Dummy", "--path", str(ROOT / "godot"),
            "-s", "res://tests/tools/rasterize_icons.gd", "--",
            f"--in={SVG_DIR}", f"--out={PNG_DIR}", f"--size={SIZE}",
        ],
        check=True,
    )


if __name__ == "__main__":
    main()
