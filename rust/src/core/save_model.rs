//! Save file model: settings, best results per level and difficulty mode, cleared
//! levels. Serialized as JSON by the `SaveData` autoload (`save.rs`).
//!
//! Loading is migration-safe: every field has a default (`#[serde(default)]`), unknown
//! fields are ignored, out-of-range values are clamped, and unreadable files fall back
//! to defaults. `version` is stamped on every write. Unversioned files (v0) share the
//! v1 field names; a future schema change adds a step to `merge_lenient` keyed on the
//! version it read.

use super::mode::DifficultyMode;
use super::scoring::Rank;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SAVE_VERSION: u32 = 1;

pub const LATENCY_LIMIT_MS: i32 = 300;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Linear bus volumes, `0..=1`.
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub screen_shake: bool,
    /// Dims full-screen flashes for photosensitive players.
    pub reduce_flashing: bool,
    /// Audio latency calibration; positive means audio is heard later than reported.
    pub latency_offset_ms: i32,
    pub fullscreen: bool,
    pub show_fps: bool,
    /// Every level selectable regardless of progress.
    pub unlock_all: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            master_volume: 0.8,
            music_volume: 0.8,
            sfx_volume: 0.8,
            screen_shake: true,
            reduce_flashing: false,
            latency_offset_ms: 0,
            fullscreen: false,
            show_fps: false,
            unlock_all: false,
        }
    }
}

impl Settings {
    fn sanitize(&mut self) {
        for volume in [
            &mut self.master_volume,
            &mut self.music_volume,
            &mut self.sfx_volume,
        ] {
            *volume = if volume.is_finite() {
                volume.clamp(0.0, 1.0)
            } else {
                0.8
            };
        }
        self.latency_offset_ms = self
            .latency_offset_ms
            .clamp(-LATENCY_LIMIT_MS, LATENCY_LIMIT_MS);
    }
}

/// Best result for one level in one difficulty mode.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LevelRecord {
    pub best_score: i64,
    /// `S`..`D`, empty if never finished.
    pub best_rank: String,
    pub plays: u32,
    pub clears: u32,
}

impl LevelRecord {
    pub fn rank(&self) -> Option<Rank> {
        parse_rank(&self.best_rank)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveModel {
    pub version: u32,
    pub settings: Settings,
    /// `records[level_id][mode]` with mode `casual|normal|hardcore`.
    pub records: BTreeMap<String, BTreeMap<String, LevelRecord>>,
    /// Level ids cleared at least once in any mode.
    pub cleared: BTreeSet<String>,
}

impl Default for SaveModel {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            settings: Settings::default(),
            records: BTreeMap::new(),
            cleared: BTreeSet::new(),
        }
    }
}

/// What a finished run changed in the save.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordOutcome {
    pub new_best_score: bool,
    pub new_best_rank: bool,
    /// First clear of this level (unlocks the next one).
    pub first_clear: bool,
    pub previous_best_score: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    Parse(String),
    /// Written by a newer build; known fields were still loaded.
    NewerVersion(u32),
}

impl SaveModel {
    /// Parses a save. Always returns a usable model; the error (if any) says what was
    /// lost so the caller can log it and keep a backup of the unreadable file.
    pub fn from_json(text: &str) -> (SaveModel, Option<LoadError>) {
        let value: serde_json::Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(err) => {
                return (
                    SaveModel::default(),
                    Some(LoadError::Parse(err.to_string())),
                );
            }
        };
        if !value.is_object() {
            return (
                SaveModel::default(),
                Some(LoadError::Parse("save root is not an object".into())),
            );
        }
        let found_version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u32;
        let mut model = merge_lenient(value);
        model.sanitize();
        let error =
            (found_version > SAVE_VERSION).then_some(LoadError::NewerVersion(found_version));
        model.version = SAVE_VERSION;
        (model, error)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }

    fn sanitize(&mut self) {
        self.settings.sanitize();
        for modes in self.records.values_mut() {
            modes.retain(|mode, _| parse_mode(mode).is_some());
            for record in modes.values_mut() {
                record.best_score = record.best_score.max(0);
                if record.rank().is_none() {
                    record.best_rank.clear();
                }
            }
        }
        self.records.retain(|_, modes| !modes.is_empty());
    }

    pub fn record(&self, level_id: &str, mode: DifficultyMode) -> LevelRecord {
        self.records
            .get(level_id)
            .and_then(|modes| modes.get(mode.as_str()))
            .cloned()
            .unwrap_or_default()
    }

    /// Best rank for a level across all modes.
    pub fn best_rank_any_mode(&self, level_id: &str) -> Option<Rank> {
        self.records
            .get(level_id)?
            .values()
            .filter_map(LevelRecord::rank)
            .min()
    }

    /// Folds a finished run into the records. Failed runs count as plays and can still
    /// set a best score.
    pub fn record_result(
        &mut self,
        level_id: &str,
        mode: DifficultyMode,
        score: i64,
        rank: Rank,
        cleared: bool,
    ) -> RecordOutcome {
        let record = self
            .records
            .entry(level_id.to_string())
            .or_default()
            .entry(mode.as_str().to_string())
            .or_default();
        let previous_best_score = record.best_score;
        record.plays += 1;
        let score = score.max(0);
        let new_best_score = score > record.best_score;
        if new_best_score {
            record.best_score = score;
        }
        let new_best_rank = record.rank().is_none_or(|best| rank < best);
        if new_best_rank {
            record.best_rank = rank.as_str().to_string();
        }
        if cleared {
            record.clears += 1;
        }
        let first_clear = cleared && self.cleared.insert(level_id.to_string());
        RecordOutcome {
            new_best_score,
            new_best_rank,
            first_clear,
            previous_best_score,
        }
    }

    /// Level `index` of `order` is open when it is the first level, the previous level
    /// was cleared, it was cleared itself, or `unlock_all` is set.
    pub fn is_unlocked(&self, order: &[&str], level_id: &str, unlock_all: bool) -> bool {
        if unlock_all || self.settings.unlock_all || self.cleared.contains(level_id) {
            return true;
        }
        match order.iter().position(|id| *id == level_id) {
            Some(0) => true,
            Some(index) => self.cleared.contains(order[index - 1]),
            None => false,
        }
    }
}

/// Deserializes field by field so one malformed section (say, a string where a number
/// was expected) keeps its defaults instead of discarding the whole save.
fn merge_lenient(value: serde_json::Value) -> SaveModel {
    let mut model = SaveModel::default();
    let serde_json::Value::Object(map) = value else {
        return model;
    };
    if let Some(settings) = map.get("settings") {
        model.settings = merge_settings(settings);
    }
    if let Some(serde_json::Value::Object(levels)) = map.get("records") {
        for (level_id, modes) in levels {
            let serde_json::Value::Object(modes) = modes else {
                continue;
            };
            for (mode, record) in modes {
                if let Ok(record) = serde_json::from_value::<LevelRecord>(record.clone()) {
                    model
                        .records
                        .entry(level_id.clone())
                        .or_default()
                        .insert(mode.clone(), record);
                }
            }
        }
    }
    if let Some(serde_json::Value::Array(ids)) = map.get("cleared") {
        model.cleared = ids
            .iter()
            .filter_map(|id| id.as_str().map(str::to_string))
            .collect();
    }
    model
}

fn merge_settings(value: &serde_json::Value) -> Settings {
    let mut settings = Settings::default();
    let serde_json::Value::Object(map) = value else {
        return settings;
    };
    let f32_of = |key: &str, into: &mut f32| {
        if let Some(v) = map.get(key).and_then(serde_json::Value::as_f64) {
            *into = v as f32;
        }
    };
    f32_of("master_volume", &mut settings.master_volume);
    f32_of("music_volume", &mut settings.music_volume);
    f32_of("sfx_volume", &mut settings.sfx_volume);
    let bool_of = |key: &str, into: &mut bool| {
        if let Some(v) = map.get(key).and_then(serde_json::Value::as_bool) {
            *into = v;
        }
    };
    bool_of("screen_shake", &mut settings.screen_shake);
    bool_of("reduce_flashing", &mut settings.reduce_flashing);
    bool_of("fullscreen", &mut settings.fullscreen);
    bool_of("show_fps", &mut settings.show_fps);
    bool_of("unlock_all", &mut settings.unlock_all);
    if let Some(v) = map
        .get("latency_offset_ms")
        .and_then(serde_json::Value::as_f64)
    {
        settings.latency_offset_ms = v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    }
    settings
}

pub fn parse_rank(text: &str) -> Option<Rank> {
    match text {
        "S" => Some(Rank::S),
        "A" => Some(Rank::A),
        "B" => Some(Rank::B),
        "C" => Some(Rank::C),
        "D" => Some(Rank::D),
        _ => None,
    }
}

pub fn parse_mode(text: &str) -> Option<DifficultyMode> {
    match text {
        "casual" => Some(DifficultyMode::Casual),
        "normal" => Some(DifficultyMode::Normal),
        "hardcore" => Some(DifficultyMode::Hardcore),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [&str; 3] = ["one", "two", "three"];

    #[test]
    fn round_trip_preserves_everything() {
        let mut model = SaveModel::default();
        model.settings.music_volume = 0.25;
        model.settings.screen_shake = false;
        model.settings.reduce_flashing = true;
        model.settings.latency_offset_ms = -40;
        model.record_result("one", DifficultyMode::Hardcore, 1234, Rank::A, true);
        let (loaded, error) = SaveModel::from_json(&model.to_json());
        assert_eq!(error, None);
        assert_eq!(loaded, model);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let (model, error) = SaveModel::from_json(r#"{"settings": {"sfx_volume": 0.1}}"#);
        assert_eq!(error, None);
        assert_eq!(model.version, SAVE_VERSION);
        assert_eq!(model.settings.sfx_volume, 0.1);
        assert_eq!(
            model.settings.music_volume,
            Settings::default().music_volume
        );
        assert!(model.settings.screen_shake);
        assert!(!model.settings.reduce_flashing);
        assert!(model.records.is_empty());
    }

    #[test]
    fn garbage_falls_back_to_defaults() {
        for text in ["", "not json", "[1, 2]", "42"] {
            let (model, error) = SaveModel::from_json(text);
            assert!(matches!(error, Some(LoadError::Parse(_))), "{text:?}");
            assert_eq!(model, SaveModel::default());
        }
    }

    #[test]
    fn malformed_sections_keep_the_rest() {
        let text = r#"{
            "version": 1,
            "settings": {"master_volume": "loud", "show_fps": true, "latency_offset_ms": 9000},
            "records": {"one": {"normal": {"best_score": "x"}, "hardcore": {"best_score": 50, "best_rank": "Z"}, "bogus": {}}},
            "cleared": ["one", 7],
            "future_field": {"anything": true}
        }"#;
        let (model, error) = SaveModel::from_json(text);
        assert_eq!(error, None);
        assert_eq!(
            model.settings.master_volume,
            Settings::default().master_volume
        );
        assert!(model.settings.show_fps);
        assert_eq!(model.settings.latency_offset_ms, LATENCY_LIMIT_MS);
        let hardcore = model.record("one", DifficultyMode::Hardcore);
        assert_eq!(hardcore.best_score, 50);
        assert_eq!(hardcore.best_rank, "");
        assert_eq!(
            model.record("one", DifficultyMode::Normal),
            LevelRecord::default()
        );
        assert_eq!(model.records["one"].len(), 1);
        assert!(model.cleared.contains("one") && model.cleared.len() == 1);
    }

    #[test]
    fn unversioned_and_newer_saves_load() {
        let (model, error) = SaveModel::from_json(r#"{"cleared": ["one"]}"#);
        assert_eq!(error, None);
        assert_eq!(model.version, SAVE_VERSION);
        assert!(model.cleared.contains("one"));
        let (model, error) = SaveModel::from_json(r#"{"version": 99, "cleared": ["two"]}"#);
        assert_eq!(error, Some(LoadError::NewerVersion(99)));
        assert!(model.cleared.contains("two"));
    }

    #[test]
    fn volumes_are_clamped() {
        let (model, _) =
            SaveModel::from_json(r#"{"settings": {"master_volume": 3.0, "music_volume": -1}}"#);
        assert_eq!(model.settings.master_volume, 1.0);
        assert_eq!(model.settings.music_volume, 0.0);
    }

    #[test]
    fn records_keep_the_best() {
        let mut model = SaveModel::default();
        let first = model.record_result("one", DifficultyMode::Normal, 500, Rank::C, true);
        assert!(first.new_best_score && first.new_best_rank && first.first_clear);
        let worse = model.record_result("one", DifficultyMode::Normal, 300, Rank::D, false);
        assert!(!worse.new_best_score && !worse.new_best_rank && !worse.first_clear);
        assert_eq!(worse.previous_best_score, 500);
        let better = model.record_result("one", DifficultyMode::Normal, 900, Rank::S, true);
        assert!(better.new_best_score && better.new_best_rank && !better.first_clear);
        let record = model.record("one", DifficultyMode::Normal);
        assert_eq!(
            (
                record.best_score,
                record.best_rank.as_str(),
                record.plays,
                record.clears
            ),
            (900, "S", 3, 2)
        );
        // Modes are tracked separately.
        assert_eq!(
            model.record("one", DifficultyMode::Casual),
            LevelRecord::default()
        );
        model.record_result("one", DifficultyMode::Casual, 10, Rank::B, true);
        assert_eq!(model.best_rank_any_mode("one"), Some(Rank::S));
        assert_eq!(model.best_rank_any_mode("two"), None);
    }

    #[test]
    fn zero_score_failure_is_not_a_best() {
        let mut model = SaveModel::default();
        let outcome = model.record_result("one", DifficultyMode::Normal, 0, Rank::D, false);
        assert!(!outcome.new_best_score);
        assert!(outcome.new_best_rank);
        assert!(!model.cleared.contains("one"));
    }

    #[test]
    fn clearing_unlocks_the_next_level() {
        let mut model = SaveModel::default();
        assert!(model.is_unlocked(&ORDER, "one", false));
        assert!(!model.is_unlocked(&ORDER, "two", false));
        assert!(!model.is_unlocked(&ORDER, "unknown", false));
        model.record_result("one", DifficultyMode::Casual, 100, Rank::B, true);
        assert!(model.is_unlocked(&ORDER, "two", false));
        assert!(!model.is_unlocked(&ORDER, "three", false));
        assert!(model.is_unlocked(&ORDER, "three", true));
        model.settings.unlock_all = true;
        assert!(model.is_unlocked(&ORDER, "three", false));
    }

    #[test]
    fn parsers_round_trip() {
        for rank in [Rank::S, Rank::A, Rank::B, Rank::C, Rank::D] {
            assert_eq!(parse_rank(rank.as_str()), Some(rank));
        }
        for mode in [
            DifficultyMode::Casual,
            DifficultyMode::Normal,
            DifficultyMode::Hardcore,
        ] {
            assert_eq!(parse_mode(mode.as_str()), Some(mode));
        }
        assert_eq!(parse_rank("s"), None);
    }
}
