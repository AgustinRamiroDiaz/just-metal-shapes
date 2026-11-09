//! Serde model of `godot/music/<song-id>.analysis.json`, written by
//! `devtools/analyze_beats.py`. Field names follow the JSON (camelCase); fields the
//! game does not use (raw dB values) are ignored on load.

use serde::{Deserialize, Serialize};

use super::timing::{BEATS_PER_BAR, Timing};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionType {
    Intro,
    Build,
    Main,
    Breakdown,
    Outro,
}

impl SectionType {
    pub const ALL: [SectionType; 5] = [
        SectionType::Intro,
        SectionType::Build,
        SectionType::Main,
        SectionType::Breakdown,
        SectionType::Outro,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SectionType::Intro => "intro",
            SectionType::Build => "build",
            SectionType::Main => "main",
            SectionType::Breakdown => "breakdown",
            SectionType::Outro => "outro",
        }
    }
}

/// Per-beat features. Every normalized value is in `0..=1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatInfo {
    /// Beat index counted from `beatOffsetSeconds` (beat 0 is the first beat).
    pub beat: i64,
    pub time_seconds: f64,
    pub loudness: f32,
    pub onset_strength: f32,
    pub low_energy: f32,
    pub mid_energy: f32,
    pub high_energy: f32,
    pub novelty: f32,
    pub accent: f32,
    pub silent: bool,
}

/// An onset quantized to the nearest half beat.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Onset {
    pub time_seconds: f64,
    pub quantized_beat: f64,
    pub strength: f32,
}

/// Per-bar aggregate (4 beats per bar).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BarInfo {
    pub bar: i64,
    pub start_beat: i64,
    pub time_seconds: f64,
    pub intensity: f32,
    #[serde(default)]
    pub onset_count: u32,
    #[serde(default)]
    pub strongest_beat: i64,
    pub silent: bool,
    #[serde(default)]
    pub onset_density: f32,
    #[serde(default)]
    pub novelty: f32,
}

/// A contiguous run of bars. `end_bar` and `end_beat` are exclusive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub start_bar: i64,
    pub end_bar: i64,
    pub start_beat: i64,
    pub end_beat: i64,
    #[serde(rename = "type")]
    pub section_type: SectionType,
    pub intensity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongAnalysis {
    #[serde(default)]
    pub source: String,
    pub duration_seconds: f64,
    pub bpm: f64,
    pub beat_offset_seconds: f64,
    #[serde(default)]
    pub confidence: f64,
    /// The tempo drifts (live recordings): `beat_times_seconds` is a tracked tempo map
    /// rather than a fixed grid at `bpm`.
    #[serde(default)]
    pub variable_tempo: bool,
    #[serde(default)]
    pub beat_times_seconds: Vec<f64>,
    pub beats: Vec<BeatInfo>,
    #[serde(default)]
    pub onsets: Vec<Onset>,
    pub bars: Vec<BarInfo>,
    pub sections: Vec<Section>,
}

impl SongAnalysis {
    /// Parses and validates analysis JSON.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let analysis: SongAnalysis =
            serde_json::from_str(json).map_err(|err| format!("invalid analysis JSON: {err}"))?;
        analysis.validate()?;
        Ok(analysis)
    }

    fn validate(&self) -> Result<(), String> {
        if !(self.bpm.is_finite() && self.bpm > 0.0) {
            return Err(format!("bpm must be positive, got {}", self.bpm));
        }
        if !(self.duration_seconds.is_finite() && self.duration_seconds > 0.0) {
            return Err(format!(
                "durationSeconds must be positive, got {}",
                self.duration_seconds
            ));
        }
        if self.beats.is_empty() {
            return Err("analysis has no beats".into());
        }
        if self.sections.is_empty() {
            return Err("analysis has no sections".into());
        }
        if self.beats.windows(2).any(|w| w[1].beat <= w[0].beat) {
            return Err("beats are not strictly increasing".into());
        }
        if self.variable_tempo
            && (self.beat_times_seconds.len() < 2
                || self.beat_times_seconds.windows(2).any(|w| w[1] <= w[0]))
        {
            return Err("variableTempo needs strictly increasing beatTimesSeconds".into());
        }
        Ok(())
    }

    pub fn timing(&self) -> Timing {
        if self.variable_tempo {
            Timing::with_beat_times(self.bpm, &self.beat_times_seconds)
        } else {
            Timing::new(self.bpm, self.beat_offset_seconds)
        }
    }

    /// The tempo map charts carry: empty for fixed-tempo songs.
    pub fn tempo_map(&self) -> Vec<f64> {
        if self.variable_tempo {
            self.beat_times_seconds.clone()
        } else {
            Vec::new()
        }
    }

    /// Bar containing `beat`, if the analysis has one.
    pub fn bar_for_beat(&self, beat: i64) -> Option<&BarInfo> {
        let index = beat.div_euclid(BEATS_PER_BAR as i64);
        self.bars
            .get(usize::try_from(index).ok()?)
            .filter(|bar| bar.start_beat <= beat && beat < bar.start_beat + BEATS_PER_BAR as i64)
            .or_else(|| {
                self.bars.iter().find(|bar| {
                    bar.start_beat <= beat && beat < bar.start_beat + BEATS_PER_BAR as i64
                })
            })
    }

    /// Index of the section containing `beat`. Beats past the last section map to it.
    pub fn section_index_for_beat(&self, beat: i64) -> usize {
        self.sections
            .iter()
            .position(|section| beat < section.end_beat)
            .unwrap_or(self.sections.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::test_support::{REAL_ANALYSES, synthetic_analysis};

    #[test]
    fn parses_every_shipped_analysis() {
        for (name, json) in REAL_ANALYSES {
            let analysis = SongAnalysis::from_json(json).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(analysis.bpm > 60.0 && analysis.bpm < 200.0, "{name} bpm");
            assert!(analysis.duration_seconds > 60.0, "{name} duration");
            assert!(!analysis.bars.is_empty(), "{name} bars");
            assert_eq!(analysis.sections[0].start_beat, 0, "{name} first section");
            let last = analysis.sections.last().unwrap();
            assert_eq!(last.section_type, SectionType::Outro, "{name} last section");
        }
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(SongAnalysis::from_json("{").is_err());
        let mut bad = synthetic_analysis(120.0, 12);
        bad.bpm = 0.0;
        let json = serde_json::to_string(&bad).unwrap();
        assert!(SongAnalysis::from_json(&json).unwrap_err().contains("bpm"));
    }

    #[test]
    fn round_trips_through_json() {
        let analysis = synthetic_analysis(128.0, 16);
        let json = serde_json::to_string(&analysis).unwrap();
        assert_eq!(SongAnalysis::from_json(&json).unwrap(), analysis);
    }

    #[test]
    fn finds_bars_and_sections() {
        let analysis = synthetic_analysis(120.0, 16);
        assert_eq!(analysis.bar_for_beat(5).unwrap().bar, 1);
        assert!(analysis.bar_for_beat(-1).is_none());
        assert_eq!(analysis.section_index_for_beat(0), 0);
        let last = analysis.sections.len() - 1;
        assert_eq!(analysis.section_index_for_beat(10_000), last);
    }
}
