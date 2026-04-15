"""Generate the Am-Dm-G-Cmaj7-Fmaj7-Bm7b5-E7sus4-E7 loop as a MIDI file."""
from pathlib import Path
import aldakit

# Chord progression: Am Dm G Cmaj7 Fmaj7 Bm7b5 E7sus4 E7
# Voicings (close position, centred around C4-E4):
#   Am    → A3 C4 E4
#   Dm    → D4 F4 A4
#   G     → G3 B3 D4
#   Cmaj7 → C4 E4 G4 B4
#   Fmaj7 → F4 A4 C5 E5
#   Bm7b5 → B3 D4 F4 A4
#   E7sus4→ E4 A4 B4 D5
#   E7    → E4 G#4 B4 D5
#
# Octave tracking (o = current octave after each chord):
#   start o3 → Am a1/>c/e  → o4
#              Dm  d/f/a   → o4
#              G  <g/b/>d  → o4
#          Cmaj7  c/e/g/b  → o4
#          Fmaj7  f/a/>c/e → o5
#          Bm7b5 <<b/>d/f/a→ o4
#         E7sus4  e/a/b/>d → o5
#             E7 <e/g+/b/>d→ o5

alda_score = """
piano:
  (tempo 100)
  o3
  a1/>c/e |
  d/f/a |
  <g/b/>d |
  c/e/g/b |
  f/a/>c/e |
  <<b/>d/f/a |
  e/a/b/>d |
  <e/g+/b/>d
"""

out = Path(__file__).parent.parent / "assets" / "loop_progression.mid"
aldakit.save(alda_score, out)
print(f"Saved: {out}")
