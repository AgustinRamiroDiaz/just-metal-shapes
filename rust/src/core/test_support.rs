//! Fixtures shared by core unit tests.

use super::analysis::{BarInfo, BeatInfo, Section, SectionType, SongAnalysis};
use super::chart::EventKind;
use super::chart_gen::{EnemyEntry, LevelSpec, Palette, PatternEntry, Rgb};
use super::timing::BEATS_PER_BAR;

/// The analyses shipped in `godot/music/`.
pub const REAL_ANALYSES: [(&str, &str); 5] = [
    (
        "wonders-of-the-earth",
        include_str!("../../../godot/music/wonders-of-the-earth.analysis.json"),
    ),
    (
        "celtic",
        include_str!("../../../godot/music/celtic.analysis.json"),
    ),
    (
        "voxel-revolution",
        include_str!("../../../godot/music/voxel-revolution.analysis.json"),
    ),
    (
        "ouroboros",
        include_str!("../../../godot/music/ouroboros.analysis.json"),
    ),
    (
        "surf-rock",
        include_str!("../../../godot/music/surf-rock.analysis.json"),
    ),
];

/// A hand-built analysis with every section type, varied accents, and one fully
/// silent bar (bar 2) plus a few isolated silent beats.
pub fn synthetic_analysis(bpm: f64, bar_count: usize) -> SongAnalysis {
    assert!(bar_count >= 10);
    let seconds_per_beat = 60.0 / bpm;
    let offset = 0.1;
    let beat_count = bar_count * BEATS_PER_BAR as usize;
    let beats: Vec<BeatInfo> = (0..beat_count)
        .map(|i| {
            let wave = ((i as f32) * 0.37).sin() * 0.5 + 0.5;
            let silent = i / 4 == 2 || i % 29 == 5;
            BeatInfo {
                beat: i as i64,
                time_seconds: offset + i as f64 * seconds_per_beat,
                loudness: if silent { 0.0 } else { 0.3 + 0.6 * wave },
                onset_strength: ((i * 37) % 100) as f32 / 100.0,
                low_energy: wave,
                mid_energy: 1.0 - wave,
                high_energy: 0.5,
                novelty: if i % 32 == 0 { 0.9 } else { 0.2 },
                accent: if silent {
                    0.0
                } else {
                    ((i * 53) % 100) as f32 / 100.0
                },
                silent,
            }
        })
        .collect();
    let bars: Vec<BarInfo> = (0..bar_count)
        .map(|bar| BarInfo {
            bar: bar as i64,
            start_beat: (bar * 4) as i64,
            time_seconds: offset + (bar * 4) as f64 * seconds_per_beat,
            intensity: if bar == 2 {
                0.0
            } else {
                0.3 + 0.5 * ((bar as f32) * 0.5).sin().abs()
            },
            onset_count: 4,
            strongest_beat: (bar * 4) as i64,
            silent: bar == 2,
            onset_density: 0.5,
            novelty: if bar % 8 == 0 { 0.9 } else { 0.3 },
        })
        .collect();
    let types = [
        SectionType::Intro,
        SectionType::Build,
        SectionType::Main,
        SectionType::Breakdown,
        SectionType::Main,
        SectionType::Outro,
    ];
    let per = bar_count / types.len();
    let sections: Vec<Section> = types
        .iter()
        .enumerate()
        .map(|(index, section_type)| {
            let start_bar = index * per;
            let end_bar = if index + 1 == types.len() {
                bar_count
            } else {
                (index + 1) * per
            };
            Section {
                start_bar: start_bar as i64,
                end_bar: end_bar as i64,
                start_beat: (start_bar * 4) as i64,
                end_beat: (end_bar * 4) as i64,
                section_type: *section_type,
                intensity: 0.5,
            }
        })
        .collect();
    SongAnalysis {
        source: "synthetic".into(),
        duration_seconds: offset + beat_count as f64 * seconds_per_beat + 0.5,
        bpm,
        beat_offset_seconds: offset,
        confidence: 1.0,
        beats,
        onsets: Vec::new(),
        bars,
        sections,
    }
}

/// A spec using every hazard kind and two enemies.
pub fn test_spec(difficulty: u8) -> LevelSpec {
    LevelSpec {
        id: "test".into(),
        title: "Test".into(),
        artist: "Nobody".into(),
        music_path: "res://music/test.ogg".into(),
        analysis_path: "res://music/test.analysis.json".into(),
        difficulty,
        seed: 1,
        palette: Palette {
            bg: Rgb::new(0.0, 0.0, 0.0),
            accent: Rgb::new(1.0, 0.0, 1.0),
            danger: Rgb::new(1.0, 0.1, 0.5),
        },
        pattern_pool: EventKind::HAZARDS
            .iter()
            .map(|kind| PatternEntry::new(*kind, 1.0))
            .collect(),
        enemy_pool: vec![
            EnemyEntry::new("res://scenes/static_shooter_enemy.tscn", false, 1.0),
            EnemyEntry::new("res://scenes/shotgun_enemy.tscn", true, 1.0),
        ],
        phrases: Vec::new(),
        density: 1.0,
        tutorial: false,
        finale: false,
    }
}
