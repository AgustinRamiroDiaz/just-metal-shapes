//! The ordered, static list of levels: one song each, increasing difficulty.
//!
//! Tune a level's hazards here through its `pattern_pool` (kinds, weights, sections,
//! telegraph/duration/coverage) and its choreography through `phrases`;
//! `core::chart_gen` turns both into a chart. See `docs/spec/levels.md`.

use crate::core::analysis::SectionType;
use crate::core::chart::EventKind;
use crate::core::chart_gen::{
    EnemyEntry, LevelSpec, Palette, PatternEntry, Phrase, PhraseEntry, Rgb,
};
use crate::util::dict_set;
use godot::prelude::*;

const STATIC_SHOOTER: &str = "res://scenes/static_shooter_enemy.tscn";
const SHOTGUN: &str = "res://scenes/shotgun_enemy.tscn";
const TURRET: &str = "res://scenes/turret_enemy.tscn";
const RUNNER: &str = "res://scenes/runner_enemy.tscn";
const MINE_LAYER: &str = "res://scenes/mine_layer_enemy.tscn";
const HOPPER: &str = "res://scenes/hopper_enemy.tscn";
const PULSER: &str = "res://scenes/pulser_enemy.tscn";
const BOUNCER: &str = "res://scenes/bouncer_enemy.tscn";
const SPLITTER: &str = "res://scenes/splitter_enemy.tscn";
const DASHER: &str = "res://scenes/dasher_enemy.tscn";
const CHAMELEON: &str = "res://scenes/chameleon_enemy.tscn";
const LANCER: &str = "res://scenes/lancer_enemy.tscn";
const WARDEN: &str = "res://scenes/warden_enemy.tscn";

use SectionType::{Breakdown, Build, Intro, Main, Outro};

fn level(
    id: &str,
    title: &str,
    artist: &str,
    difficulty: u8,
    seed: u64,
    palette: Palette,
) -> LevelSpec {
    LevelSpec {
        id: id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        music_path: format!("res://music/{id}.ogg"),
        analysis_path: format!("res://music/{id}.analysis.json"),
        difficulty,
        seed,
        palette,
        pattern_pool: Vec::new(),
        enemy_pool: Vec::new(),
        phrases: Vec::new(),
        density: 1.0,
        tutorial: false,
        finale: false,
    }
}

/// All levels in play order. Level 1 is the gentle tutorial; the last ends with the
/// finale set piece. Each level has its own palette, signature phrases (weighted up in
/// `phrases`), hazard pool and enemy mix.
pub fn all_levels() -> Vec<LevelSpec> {
    // 1. Tutorial: pulses on the grid, a few lasers, gapped rings. Turrets and shooters.
    let mut wonders = level(
        "wonders-of-the-earth",
        "Wonders of the Earth",
        "Grand Project",
        1,
        0x5EED_0001,
        Palette {
            bg: Rgb::new(0.04, 0.07, 0.12),
            accent: Rgb::new(0.30, 0.85, 1.0),
            danger: Rgb::new(1.0, 0.25, 0.6),
        },
    );
    wonders.tutorial = true;
    wonders.density = 0.8;
    wonders.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 3.0),
        PatternEntry::new(EventKind::Laser, 1.0).in_sections(&[Build, Breakdown, Outro]),
        PatternEntry::new(EventKind::BulletRing, 0.6)
            .in_sections(&[Build])
            .count(8, 10),
    ];
    wonders.phrases = vec![
        PhraseEntry::new(Phrase::PulseGrid, 2.0),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.0),
        PhraseEntry::new(Phrase::RingsOnKicks, 2.0),
    ];
    wonders.enemy_pool = vec![
        EnemyEntry::new(PULSER, false, 1.2).introduced(),
        EnemyEntry::new(HOPPER, false, 1.2).introduced(),
        EnemyEntry::new(STATIC_SHOOTER, false, 1.5),
        EnemyEntry::new(TURRET, false, 0.8),
    ];

    // 2. Voxel Revolution: blocky geometry. Spikes from the sides and walls with gaps.
    let mut voxel = level(
        "voxel-revolution",
        "Voxel Revolution",
        "Kevin MacLeod",
        2,
        0x5EED_0004,
        Palette {
            bg: Rgb::new(0.08, 0.05, 0.14),
            accent: Rgb::new(0.85, 1.0, 0.25),
            danger: Rgb::new(1.0, 0.2, 0.75),
        },
    );
    voxel.density = 1.05;
    voxel.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 2.0),
        PatternEntry::new(EventKind::Laser, 1.5),
        PatternEntry::new(EventKind::Spikes, 1.2).count(6, 8),
        PatternEntry::new(EventKind::Wall, 1.0).in_sections(&[Build, Main]),
        PatternEntry::new(EventKind::BulletRing, 0.6)
            .in_sections(&[Main])
            .count(8, 12),
    ];
    voxel.phrases = vec![
        PhraseEntry::new(Phrase::SpikeSides, 2.5),
        PhraseEntry::new(Phrase::SweepingWall, 2.0),
        PhraseEntry::new(Phrase::PulseGrid, 1.0),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.0),
        PhraseEntry::new(Phrase::RingsOnKicks, 0.5),
        PhraseEntry::new(Phrase::Breather, 1.0),
    ];
    voxel.enemy_pool = vec![
        EnemyEntry::new(BOUNCER, false, 1.3).introduced(),
        EnemyEntry::new(SPLITTER, false, 1.0).introduced(),
        EnemyEntry::new(STATIC_SHOOTER, false, 1.0),
        EnemyEntry::new(RUNNER, true, 1.0),
        EnemyEntry::new(TURRET, false, 0.7),
        EnemyEntry::new(HOPPER, false, 0.8),
        EnemyEntry::new(PULSER, false, 0.7),
    ];

    // 3. Celtic: a reel. Rings on every kick and lasers trading sides, with sweeps.
    let mut celtic = level(
        "celtic",
        "Celtic",
        "Alex Morgan",
        3,
        0x5EED_0002,
        Palette {
            bg: Rgb::new(0.05, 0.10, 0.06),
            accent: Rgb::new(0.55, 1.0, 0.35),
            danger: Rgb::new(1.0, 0.3, 0.5),
        },
    );
    celtic.density = 0.9;
    celtic.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 2.0),
        PatternEntry::new(EventKind::Laser, 2.0),
        PatternEntry::new(EventKind::LaserSweep, 1.0).in_sections(&[Build, Main]),
        PatternEntry::new(EventKind::BulletRing, 1.5),
        PatternEntry::new(EventKind::Wall, 0.8).in_sections(&[Main]),
        PatternEntry::new(EventKind::Spikes, 1.0).in_sections(&[Main, Breakdown, Outro]),
        PatternEntry::new(EventKind::Spiral, 0.6).in_sections(&[Build]),
    ];
    celtic.phrases = vec![
        PhraseEntry::new(Phrase::RingsOnKicks, 2.5),
        PhraseEntry::new(Phrase::LaserCallResponse, 2.0),
        PhraseEntry::new(Phrase::SweepCross, 1.0),
        PhraseEntry::new(Phrase::SweepingWall, 0.8),
        PhraseEntry::new(Phrase::SpikeSides, 0.8),
        PhraseEntry::new(Phrase::SpiralRiser, 1.0),
        PhraseEntry::new(Phrase::PulseGrid, 1.0),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::Scatter, 0.5),
    ];
    celtic.enemy_pool = vec![
        EnemyEntry::new(DASHER, false, 1.2).introduced(),
        EnemyEntry::new(CHAMELEON, false, 1.0).introduced(),
        EnemyEntry::new(STATIC_SHOOTER, false, 0.7),
        EnemyEntry::new(SHOTGUN, true, 0.9),
        EnemyEntry::new(RUNNER, true, 0.8),
        EnemyEntry::new(TURRET, false, 0.5),
        EnemyEntry::new(HOPPER, false, 0.8),
        EnemyEntry::new(BOUNCER, false, 0.8),
        EnemyEntry::new(SPLITTER, false, 0.6),
    ];

    // 4. Ouroboros: the serpent. Spirals, rotating sweeps and bombs; mine layers.
    let mut ouroboros = level(
        "ouroboros",
        "Ouroboros",
        "Kevin MacLeod",
        4,
        0x5EED_0005,
        Palette {
            bg: Rgb::new(0.03, 0.07, 0.09),
            accent: Rgb::new(1.0, 0.78, 0.25),
            danger: Rgb::new(1.0, 0.15, 0.45),
        },
    );
    ouroboros.density = 1.3;
    ouroboros.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 1.5),
        PatternEntry::new(EventKind::Laser, 1.5),
        PatternEntry::new(EventKind::LaserSweep, 1.5),
        PatternEntry::new(EventKind::Spiral, 1.5).in_sections(&[Build, Main]),
        PatternEntry::new(EventKind::BulletRing, 1.0),
        PatternEntry::new(EventKind::Bomb, 1.0),
        PatternEntry::new(EventKind::Spikes, 0.6),
        PatternEntry::new(EventKind::Wall, 0.6).in_sections(&[Main]),
    ];
    ouroboros.phrases = vec![
        PhraseEntry::new(Phrase::SpiralRiser, 2.5),
        PhraseEntry::new(Phrase::SweepCross, 2.0),
        PhraseEntry::new(Phrase::BombPairs, 1.5),
        PhraseEntry::new(Phrase::RingsOnKicks, 1.0),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.0),
        PhraseEntry::new(Phrase::SweepingWall, 0.6),
        PhraseEntry::new(Phrase::SpikeSides, 0.6),
        PhraseEntry::new(Phrase::PulseGrid, 0.3),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::Scatter, 0.5),
    ];
    ouroboros.enemy_pool = vec![
        EnemyEntry::new(LANCER, false, 1.2).introduced(),
        EnemyEntry::new(WARDEN, false, 0.8)
            .introduced()
            .supporting(),
        EnemyEntry::new(TURRET, false, 0.8),
        EnemyEntry::new(MINE_LAYER, true, 0.9),
        EnemyEntry::new(SHOTGUN, true, 0.8),
        EnemyEntry::new(RUNNER, true, 0.6),
        EnemyEntry::new(PULSER, false, 0.7),
        EnemyEntry::new(DASHER, false, 0.9),
        EnemyEntry::new(CHAMELEON, false, 0.6),
    ];

    // 5. Surf Rock (final): everything, aimed barrages on the snare, bomb pairs, and the
    // finale set piece before the outro.
    let mut surf = level(
        "surf-rock",
        "Surf Rock",
        "Alex Morgan",
        5,
        0x5EED_0003,
        Palette {
            bg: Rgb::new(0.12, 0.04, 0.08),
            accent: Rgb::new(1.0, 0.6, 0.2),
            danger: Rgb::new(1.0, 0.1, 0.42),
        },
    );
    surf.finale = true;
    surf.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 1.5),
        PatternEntry::new(EventKind::Laser, 2.0),
        PatternEntry::new(EventKind::LaserSweep, 1.5),
        PatternEntry::new(EventKind::BulletRing, 1.5),
        PatternEntry::new(EventKind::Spiral, 1.0).in_sections(&[Main, Build]),
        PatternEntry::new(EventKind::Wall, 1.0).in_sections(&[Main, Build, Outro]),
        PatternEntry::new(EventKind::Bomb, 0.8),
        PatternEntry::new(EventKind::Spikes, 1.0),
        PatternEntry::new(EventKind::Barrage, 1.0).in_sections(&[Main, Build]),
    ];
    surf.phrases = vec![
        PhraseEntry::new(Phrase::BarrageSnares, 2.5),
        PhraseEntry::new(Phrase::BombPairs, 1.5),
        PhraseEntry::new(Phrase::SweepingWall, 1.5),
        PhraseEntry::new(Phrase::RingsOnKicks, 1.2),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.2),
        PhraseEntry::new(Phrase::SweepCross, 1.0),
        PhraseEntry::new(Phrase::SpikeSides, 1.0),
        PhraseEntry::new(Phrase::SpiralRiser, 1.0),
        PhraseEntry::new(Phrase::PulseGrid, 0.6),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::Scatter, 0.6),
    ];
    surf.enemy_pool = vec![
        EnemyEntry::new(STATIC_SHOOTER, false, 0.6),
        EnemyEntry::new(SHOTGUN, true, 0.8),
        EnemyEntry::new(RUNNER, true, 0.8),
        EnemyEntry::new(TURRET, false, 0.6),
        EnemyEntry::new(MINE_LAYER, true, 0.6),
        EnemyEntry::new(HOPPER, false, 0.8),
        EnemyEntry::new(PULSER, false, 0.7),
        EnemyEntry::new(BOUNCER, false, 0.8),
        EnemyEntry::new(SPLITTER, false, 0.7),
        EnemyEntry::new(DASHER, false, 1.0),
        EnemyEntry::new(CHAMELEON, false, 0.8),
        EnemyEntry::new(LANCER, false, 1.0),
        EnemyEntry::new(WARDEN, false, 0.6).supporting(),
    ];

    let mut levels = vec![wonders, voxel, celtic, ouroboros, surf];
    // Intro sections stay pulse-only on every level so the first hits are readable.
    for spec in &mut levels {
        for entry in &mut spec.pattern_pool {
            if entry.sections.is_empty() && entry.kind != EventKind::Pulse {
                entry.sections = vec![Build, Main, Breakdown, Outro];
            }
        }
        if !spec.pattern_pool.iter().any(|e| e.allowed_in(Intro)) {
            spec.pattern_pool[0].sections.clear();
        }
    }
    levels
}

pub fn find_level(id: &str) -> Option<LevelSpec> {
    all_levels().into_iter().find(|spec| spec.id == id)
}

pub fn first_level_id() -> String {
    all_levels()
        .first()
        .map(|spec| spec.id.clone())
        .unwrap_or_default()
}

pub fn rgb_to_color(rgb: Rgb) -> Color {
    Color::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Godot dictionary view of a level. Keys: `id`, `index`, `title`, `artist`,
/// `music_path`, `analysis_path`, `difficulty`, `seed`, `bg_color`, `accent_color`,
/// `danger_color`, `density`, `tutorial`, `finale`, `pattern_pool` (kind names),
/// `phrases` (phrase names), `enemy_pool` (scene paths).
pub fn level_to_dictionary(spec: &LevelSpec, index: usize) -> VarDictionary {
    let mut dict = VarDictionary::new();
    let kinds: PackedStringArray = spec
        .pattern_pool
        .iter()
        .map(|entry| GString::from(entry.kind.name()))
        .collect();
    let phrases: PackedStringArray = spec
        .phrases
        .iter()
        .map(|entry| GString::from(entry.phrase.name()))
        .collect();
    let enemies: PackedStringArray = spec
        .enemy_pool
        .iter()
        .map(|entry| GString::from(&entry.scene))
        .collect();
    dict_set(&mut dict, "id", GString::from(&spec.id));
    dict_set(&mut dict, "index", index as i64);
    dict_set(&mut dict, "title", GString::from(&spec.title));
    dict_set(&mut dict, "artist", GString::from(&spec.artist));
    dict_set(&mut dict, "music_path", GString::from(&spec.music_path));
    dict_set(
        &mut dict,
        "analysis_path",
        GString::from(&spec.analysis_path),
    );
    dict_set(&mut dict, "difficulty", spec.difficulty as i64);
    dict_set(&mut dict, "seed", spec.seed as i64);
    dict_set(&mut dict, "bg_color", rgb_to_color(spec.palette.bg));
    dict_set(&mut dict, "accent_color", rgb_to_color(spec.palette.accent));
    dict_set(&mut dict, "danger_color", rgb_to_color(spec.palette.danger));
    dict_set(&mut dict, "density", spec.density as f64);
    dict_set(&mut dict, "tutorial", spec.tutorial);
    dict_set(&mut dict, "finale", spec.finale);
    dict_set(&mut dict, "pattern_pool", kinds);
    dict_set(&mut dict, "phrases", phrases);
    dict_set(&mut dict, "enemy_pool", enemies);
    dict
}

/// Static access to the level list from Godot: `LevelCatalog.count()`,
/// `LevelCatalog.get_level(i)`, `LevelCatalog.find_level(id)`, `LevelCatalog.list()`.
#[derive(GodotClass)]
#[class(init, base = Object)]
pub struct LevelCatalog {
    base: Base<Object>,
}

#[godot_api]
impl LevelCatalog {
    #[func]
    pub fn count() -> i64 {
        all_levels().len() as i64
    }

    /// Level dictionary at `index` (empty if out of range).
    #[func]
    pub fn get_level(index: i64) -> VarDictionary {
        usize::try_from(index)
            .ok()
            .and_then(|i| all_levels().get(i).map(|spec| level_to_dictionary(spec, i)))
            .unwrap_or_default()
    }

    /// Level dictionary by id (empty if unknown).
    #[func]
    pub fn find_level(id: GString) -> VarDictionary {
        let id = id.to_string();
        all_levels()
            .iter()
            .enumerate()
            .find(|(_, spec)| spec.id == id)
            .map(|(i, spec)| level_to_dictionary(spec, i))
            .unwrap_or_default()
    }

    /// Index of a level id, or -1.
    #[func]
    pub fn index_of(id: GString) -> i64 {
        let id = id.to_string();
        all_levels()
            .iter()
            .position(|spec| spec.id == id)
            .map_or(-1, |i| i as i64)
    }

    #[func]
    pub fn list() -> Array<VarDictionary> {
        all_levels()
            .iter()
            .enumerate()
            .map(|(i, spec)| level_to_dictionary(spec, i))
            .collect()
    }

    #[func(rename = first_level_id)]
    pub fn first_level_id_gd() -> GString {
        GString::from(&first_level_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::analysis::SongAnalysis;
    use crate::core::chart::{Chart, ChartEvent};
    use crate::core::chart_gen::{
        Phrase, coverage_cap, generate_chart, max_active_hazards, phrase_plan,
    };
    use crate::core::mode::DifficultyMode;
    use crate::core::test_support::REAL_ANALYSES;
    use std::collections::{HashMap, HashSet};

    fn analysis(id: &str) -> SongAnalysis {
        let (_, json) = REAL_ANALYSES.iter().find(|(name, _)| *name == id).unwrap();
        SongAnalysis::from_json(json).unwrap()
    }

    fn chart(spec: &LevelSpec) -> Chart {
        generate_chart(&analysis(&spec.id), spec, spec.seed)
    }

    #[test]
    fn levels_are_distinct_and_ordered() {
        let levels = all_levels();
        assert_eq!(levels.len(), 5);
        let ids: Vec<&str> = levels.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "wonders-of-the-earth",
                "voxel-revolution",
                "celtic",
                "ouroboros",
                "surf-rock"
            ]
        );
        assert!(levels.windows(2).all(|w| w[0].difficulty < w[1].difficulty));
        assert!(levels[0].tutorial);
        assert!(levels.last().unwrap().finale);
        assert_eq!(levels.iter().filter(|l| l.finale).count(), 1);
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), levels.len());
        let accents: HashSet<String> = levels
            .iter()
            .map(|l| format!("{:?}", l.palette.accent))
            .collect();
        assert_eq!(accents.len(), levels.len());
        for spec in &levels {
            assert!(!spec.pattern_pool.is_empty() && !spec.enemy_pool.is_empty());
            assert!(!spec.phrases.is_empty(), "{}", spec.id);
            assert!(spec.pattern_pool.iter().any(|e| e.allowed_in(Intro)));
            assert!(REAL_ANALYSES.iter().any(|(id, _)| *id == spec.id));
        }
        assert!(levels[0].pattern_pool.len() < levels[4].pattern_pool.len());
        // The final level uses every hazard kind.
        let kinds: HashSet<EventKind> = levels[4].pattern_pool.iter().map(|e| e.kind).collect();
        assert_eq!(kinds.len(), EventKind::HAZARDS.len());
    }

    /// Hazard pressure: summed `coverage x active seconds` per minute of song. Counts
    /// alone undersell levels built on fewer, longer hazards (spirals, sweeps).
    fn pressure(spec: &LevelSpec, chart: &Chart) -> f64 {
        let timing = chart.timing();
        let load: f64 = chart
            .events
            .iter()
            .filter(|e| e.kind.is_hazard())
            .map(|e| {
                let coverage = spec.pattern(e.kind).unwrap().coverage as f64;
                coverage * timing.beats_to_duration(e.params.duration_beats)
            })
            .sum();
        load / (chart.duration_seconds / 60.0)
    }

    /// Peak pressure: the busiest `PEAK_WINDOW_BARS` stretch, as coverage-seconds per
    /// minute. Ranks levels by their hardest passage, independent of long quiet intros.
    fn peak_pressure(spec: &LevelSpec, chart: &Chart) -> f64 {
        const PEAK_WINDOW_BARS: f64 = 8.0;
        let timing = chart.timing();
        let window = PEAK_WINDOW_BARS * 4.0;
        let last = chart.events.iter().map(|e| e.beat).fold(0.0, f64::max);
        let mut peak: f64 = 0.0;
        let mut start = 0.0;
        while start <= last {
            let load: f64 = chart
                .events
                .iter()
                .filter(|e| e.kind.is_hazard() && e.beat >= start && e.beat < start + window)
                .map(|e| {
                    let coverage = spec.pattern(e.kind).unwrap().coverage as f64;
                    coverage * timing.beats_to_duration(e.params.duration_beats)
                })
                .sum();
            peak = peak.max(load / (timing.beats_to_duration(window) / 60.0));
            start += 4.0;
        }
        peak
    }

    #[test]
    fn every_level_generates_a_playable_chart() {
        let mut pressures = Vec::new();
        for spec in all_levels() {
            let chart = chart(&spec);
            let hazards = chart.events.iter().filter(|e| e.kind.is_hazard()).count();
            assert!(hazards > 10, "{}: {hazards} hazards", spec.id);
            assert!(chart.count_kind(EventKind::SpawnEnemy) > 0, "{}", spec.id);
            for event in chart.events.iter().filter(|e| e.kind.is_hazard()) {
                assert!(spec.pattern(event.kind).is_some(), "{}: {event:?}", spec.id);
            }
            pressures.push(peak_pressure(&spec, &chart));
        }
        assert!(pressures.windows(2).all(|w| w[0] < w[1]), "{pressures:?}");
    }

    /// Tuning aid: `cargo test level_stats -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn level_stats() {
        for spec in all_levels() {
            let chart = chart(&spec);
            let plan = phrase_plan(&analysis(&spec.id), &spec, spec.seed);
            let mut kinds: Vec<(EventKind, usize)> = EventKind::ALL
                .iter()
                .filter(|k| k.is_hazard() || **k == EventKind::SpawnEnemy)
                .map(|k| (*k, chart.count_kind(*k)))
                .filter(|(_, n)| *n > 0)
                .collect();
            kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            let hazards = chart.events.iter().filter(|e| e.kind.is_hazard()).count();
            println!(
                "{}: {hazards} hazards, {:.1}/min, pressure {:.2}, peak {:.2}",
                spec.id,
                hazards as f64 / (chart.duration_seconds / 60.0),
                pressure(&spec, &chart),
                peak_pressure(&spec, &chart)
            );
            println!("  kinds: {kinds:?}");
            let phrases: Vec<String> = plan
                .iter()
                .map(|(s, e, p)| format!("{}-{}:{}", s / 4, e / 4, p.name()))
                .collect();
            println!("  phrases: {}", phrases.join(" "));
        }
    }

    #[test]
    fn levels_play_their_signature_phrases() {
        for spec in all_levels() {
            let plan = phrase_plan(&analysis(&spec.id), &spec, spec.seed);
            let mut counts: HashMap<Phrase, usize> = HashMap::new();
            for (_, _, phrase) in &plan {
                *counts.entry(*phrase).or_default() += 1;
            }
            let signature = spec
                .phrases
                .iter()
                .max_by(|a, b| a.weight.total_cmp(&b.weight))
                .unwrap()
                .phrase;
            assert!(
                counts.get(&signature).copied().unwrap_or(0) >= 2,
                "{}: signature {signature:?} in {counts:?}",
                spec.id
            );
            let distinct = counts.len();
            assert!(distinct >= 3, "{}: only {distinct} phrases", spec.id);
        }
    }

    /// Never more than `max_active_hazards` at any beat, and coverage stays under the
    /// cap, for every level in every difficulty mode.
    #[test]
    fn every_level_keeps_a_safe_path_in_every_mode() {
        for spec in all_levels() {
            for mode in [
                DifficultyMode::Casual,
                DifficultyMode::Normal,
                DifficultyMode::Hardcore,
            ] {
                let mut tuned = spec.clone();
                tuned.density *= mode.density_scale();
                let chart = chart(&tuned);
                let hazards: Vec<&ChartEvent> =
                    chart.events.iter().filter(|e| e.kind.is_hazard()).collect();
                for probe in &hazards {
                    let active: Vec<&&ChartEvent> = hazards
                        .iter()
                        .filter(|e| e.beat <= probe.beat && e.end_beat() > probe.beat)
                        .collect();
                    let coverage: f32 = active
                        .iter()
                        .map(|e| spec.pattern(e.kind).unwrap().coverage)
                        .sum();
                    assert!(active.len() <= max_active_hazards(spec.difficulty));
                    assert!(coverage <= coverage_cap(spec.difficulty) + 1e-5);
                }
            }
        }
    }

    #[test]
    fn find_level_by_id() {
        assert_eq!(find_level("celtic").unwrap().difficulty, 3);
        assert_eq!(find_level("ouroboros").unwrap().difficulty, 4);
        assert!(find_level("nope").is_none());
    }
}
