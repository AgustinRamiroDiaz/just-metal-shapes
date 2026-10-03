//! The ordered, static list of levels: one song each, increasing difficulty.
//!
//! Tune a level's hazards here through its `pattern_pool` (kinds, weights, sections,
//! telegraph/duration/coverage); `core::chart_gen` turns it into a chart.

use crate::core::analysis::SectionType;
use crate::core::chart::EventKind;
use crate::core::chart_gen::{EnemyEntry, LevelSpec, Palette, PatternEntry, Rgb};
use crate::util::dict_set;
use godot::prelude::*;

const STATIC_SHOOTER: &str = "res://scenes/static_shooter_enemy.tscn";
const SHOTGUN: &str = "res://scenes/shotgun_enemy.tscn";
const TURRET: &str = "res://scenes/turret_enemy.tscn";
const RUNNER: &str = "res://scenes/runner_enemy.tscn";
const MINE_LAYER: &str = "res://scenes/mine_layer_enemy.tscn";

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
        density: 1.0,
        tutorial: false,
    }
}

/// All levels in play order. Level 1 is the gentle tutorial.
pub fn all_levels() -> Vec<LevelSpec> {
    let mut wonders = level(
        "wonders-of-the-earth",
        "Wonders of the Earth",
        "Grand Project",
        1,
        0x5EED_0001,
        Palette {
            bg: Rgb::new(0.04, 0.07, 0.12),
            accent: Rgb::new(0.30, 0.85, 1.0),
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
    wonders.enemy_pool = vec![
        EnemyEntry::new(STATIC_SHOOTER, false, 2.0),
        EnemyEntry::new(TURRET, false, 1.0),
    ];

    let mut celtic = level(
        "celtic",
        "Celtic",
        "Alex Morgan",
        3,
        0x5EED_0002,
        Palette {
            bg: Rgb::new(0.05, 0.10, 0.06),
            accent: Rgb::new(0.55, 1.0, 0.35),
        },
    );
    celtic.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 2.0),
        PatternEntry::new(EventKind::Laser, 2.0),
        PatternEntry::new(EventKind::LaserSweep, 1.0).in_sections(&[Build, Main]),
        PatternEntry::new(EventKind::BulletRing, 1.5),
        PatternEntry::new(EventKind::Wall, 0.8).in_sections(&[Main]),
        PatternEntry::new(EventKind::Spikes, 1.0).in_sections(&[Main, Breakdown, Outro]),
    ];
    celtic.enemy_pool = vec![
        EnemyEntry::new(STATIC_SHOOTER, false, 1.0),
        EnemyEntry::new(SHOTGUN, true, 1.0),
        EnemyEntry::new(RUNNER, true, 1.0),
        EnemyEntry::new(TURRET, false, 0.7),
    ];

    let mut surf = level(
        "surf-rock",
        "Surf Rock",
        "Alex Morgan",
        5,
        0x5EED_0003,
        Palette {
            bg: Rgb::new(0.12, 0.04, 0.08),
            accent: Rgb::new(1.0, 0.35, 0.55),
        },
    );
    surf.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 1.5),
        PatternEntry::new(EventKind::Laser, 2.0),
        PatternEntry::new(EventKind::LaserSweep, 1.5),
        PatternEntry::new(EventKind::BulletRing, 1.5),
        PatternEntry::new(EventKind::Spiral, 1.0).in_sections(&[Main, Build]),
        PatternEntry::new(EventKind::Wall, 1.0).in_sections(&[Main, Build, Outro]),
        PatternEntry::new(EventKind::Bomb, 0.8).min_accent(0.6),
        PatternEntry::new(EventKind::Spikes, 1.0),
        PatternEntry::new(EventKind::Barrage, 1.0).in_sections(&[Main]),
    ];
    surf.enemy_pool = vec![
        EnemyEntry::new(STATIC_SHOOTER, false, 1.0),
        EnemyEntry::new(SHOTGUN, true, 1.0),
        EnemyEntry::new(RUNNER, true, 1.2),
        EnemyEntry::new(TURRET, false, 1.0),
        EnemyEntry::new(MINE_LAYER, true, 0.8),
    ];

    // Intro sections stay pulse-only on every level so the first hits are readable.
    for spec in [&mut wonders, &mut celtic, &mut surf] {
        for entry in &mut spec.pattern_pool {
            if entry.sections.is_empty() && entry.kind != EventKind::Pulse {
                entry.sections = vec![Build, Main, Breakdown, Outro];
            }
        }
        if !spec.pattern_pool.iter().any(|e| e.allowed_in(Intro)) {
            spec.pattern_pool[0].sections.clear();
        }
    }

    vec![wonders, celtic, surf]
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
/// `density`, `tutorial`, `pattern_pool` (kind names), `enemy_pool` (scene paths).
pub fn level_to_dictionary(spec: &LevelSpec, index: usize) -> VarDictionary {
    let mut dict = VarDictionary::new();
    let kinds: PackedStringArray = spec
        .pattern_pool
        .iter()
        .map(|entry| GString::from(entry.kind.name()))
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
    dict_set(&mut dict, "density", spec.density as f64);
    dict_set(&mut dict, "tutorial", spec.tutorial);
    dict_set(&mut dict, "pattern_pool", kinds);
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
    use crate::core::chart_gen::generate_chart;
    use crate::core::test_support::REAL_ANALYSES;
    use std::collections::HashSet;

    #[test]
    fn levels_are_distinct_and_ordered() {
        let levels = all_levels();
        assert_eq!(levels.len(), 3);
        assert!(levels.windows(2).all(|w| w[0].difficulty < w[1].difficulty));
        assert!(levels[0].tutorial);
        let ids: HashSet<&str> = levels.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids.len(), levels.len());
        let accents: HashSet<String> = levels
            .iter()
            .map(|l| format!("{:?}", l.palette.accent))
            .collect();
        assert_eq!(accents.len(), levels.len());
        for spec in &levels {
            assert!(!spec.pattern_pool.is_empty() && !spec.enemy_pool.is_empty());
            assert!(spec.pattern_pool.iter().any(|e| e.allowed_in(Intro)));
            assert!(REAL_ANALYSES.iter().any(|(id, _)| *id == spec.id));
        }
        assert!(levels[0].pattern_pool.len() < levels[2].pattern_pool.len());
    }

    #[test]
    fn every_level_generates_a_playable_chart() {
        let mut hazards_per_minute = Vec::new();
        for spec in all_levels() {
            let (_, json) = REAL_ANALYSES.iter().find(|(id, _)| *id == spec.id).unwrap();
            let analysis = SongAnalysis::from_json(json).unwrap();
            let chart = generate_chart(&analysis, &spec, spec.seed);
            let hazards = chart.events.iter().filter(|e| e.kind.is_hazard()).count();
            assert!(hazards > 10, "{}: {hazards} hazards", spec.id);
            assert!(chart.count_kind(EventKind::SpawnEnemy) > 0, "{}", spec.id);
            hazards_per_minute.push(hazards as f64 / (chart.duration_seconds / 60.0));
        }
        assert!(
            hazards_per_minute.windows(2).all(|w| w[0] < w[1]),
            "{hazards_per_minute:?}"
        );
    }

    #[test]
    fn find_level_by_id() {
        assert_eq!(find_level("celtic").unwrap().difficulty, 3);
        assert!(find_level("nope").is_none());
    }
}
