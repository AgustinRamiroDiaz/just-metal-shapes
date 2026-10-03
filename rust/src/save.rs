//! `SaveData` autoload: settings, best results and unlocks, persisted to
//! `user://save.json` (model and parsing in `core::save_model`).
//!
//! Settings are Godot properties (`SaveData.screen_shake`, `SaveData.music_volume`, ...).
//! Setting one applies it (audio buses, window mode), emits `settings_changed` and
//! schedules a save a moment later, so sliders do not write on every step.
//! Writes go to `<path>.tmp` and are renamed over the save; the previous file is kept
//! as `<path>.bak` and used when the main file is unreadable.

use crate::core::mode::DifficultyMode;
use crate::core::save_model::{LoadError, SaveModel, Settings, parse_rank};
use crate::level_catalog;
use crate::util::dict_set;
use godot::classes::{
    AudioServer, DirAccess, DisplayServer, FileAccess, INode, Node, Os, display_server::WindowMode,
    file_access::ModeFlags,
};
use godot::prelude::*;

pub const AUTOLOAD_PATH: &str = "/root/SaveData";
/// Audio bus names (see `godot/default_bus_layout.tres`).
pub const BUS_MASTER: &str = "Master";
pub const BUS_MUSIC: &str = "Music";
pub const BUS_SFX: &str = "SFX";
/// Seconds after the last settings change before writing.
const SAVE_DELAY: f64 = 0.4;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct SaveData {
    /// File the save is read from and written to (tests point this at a temp file).
    #[var]
    pub save_path: GString,
    /// Unlock every level in debug builds (tests turn this off to check unlocking).
    #[var]
    pub debug_unlocks_all: bool,

    /// Linear `0..=1`.
    #[var(set)]
    pub master_volume: f64,
    #[var(set)]
    pub music_volume: f64,
    #[var(set)]
    pub sfx_volume: f64,
    /// Read by camera/Fx code before shaking the screen.
    #[var(set)]
    pub screen_shake: bool,
    /// Audio latency calibration (ms); the level applies it to `Conductor.latency_offset`.
    #[var(set)]
    pub latency_offset_ms: i64,
    #[var(set)]
    pub fullscreen: bool,
    #[var(set)]
    pub show_fps: bool,
    #[var(set)]
    pub unlock_all: bool,

    model: SaveModel,
    save_in: Option<f64>,
    base: Base<Node>,
}

#[godot_api]
impl INode for SaveData {
    fn init(base: Base<Node>) -> Self {
        let mut data = Self {
            save_path: "user://save.json".into(),
            debug_unlocks_all: Os::singleton().is_debug_build(),
            master_volume: 0.0,
            music_volume: 0.0,
            sfx_volume: 0.0,
            screen_shake: true,
            latency_offset_ms: 0,
            fullscreen: false,
            show_fps: false,
            unlock_all: false,
            model: SaveModel::default(),
            save_in: None,
            base,
        };
        data.load_settings_fields();
        data
    }

    fn ready(&mut self) {
        self.reload();
    }

    fn process(&mut self, delta: f64) {
        if let Some(left) = self.save_in.as_mut() {
            *left -= delta;
            if *left <= 0.0 {
                self.save();
            }
        }
    }

    fn exit_tree(&mut self) {
        if self.save_in.is_some() {
            self.save();
        }
    }
}

#[godot_api]
impl SaveData {
    #[signal]
    pub fn settings_changed();

    #[func]
    pub fn set_master_volume(&mut self, value: f64) {
        self.master_volume = value.clamp(0.0, 1.0);
        self.settings_touched();
    }

    #[func]
    pub fn set_music_volume(&mut self, value: f64) {
        self.music_volume = value.clamp(0.0, 1.0);
        self.settings_touched();
    }

    #[func]
    pub fn set_sfx_volume(&mut self, value: f64) {
        self.sfx_volume = value.clamp(0.0, 1.0);
        self.settings_touched();
    }

    #[func]
    pub fn set_screen_shake(&mut self, value: bool) {
        self.screen_shake = value;
        self.settings_touched();
    }

    #[func]
    pub fn set_latency_offset_ms(&mut self, value: i64) {
        let limit = crate::core::save_model::LATENCY_LIMIT_MS as i64;
        self.latency_offset_ms = value.clamp(-limit, limit);
        self.settings_touched();
    }

    #[func]
    pub fn set_fullscreen(&mut self, value: bool) {
        self.fullscreen = value;
        self.settings_touched();
    }

    #[func]
    pub fn set_show_fps(&mut self, value: bool) {
        self.show_fps = value;
        self.settings_touched();
    }

    #[func]
    pub fn set_unlock_all(&mut self, value: bool) {
        self.unlock_all = value;
        self.settings_touched();
    }

    /// Latency offset in seconds, for `Conductor.latency_offset`.
    #[func]
    pub fn latency_offset_seconds(&self) -> f64 {
        self.latency_offset_ms as f64 / 1000.0
    }

    /// Re-reads `save_path` (falling back to the backup, then defaults) and applies the
    /// settings. Returns false when neither file could be parsed.
    #[func]
    pub fn reload(&mut self) -> bool {
        self.save_in = None;
        let path = self.save_path.to_string();
        let (model, ok) = match read_model(&path) {
            Some(Ok(model)) => (model, true),
            Some(Err(err)) => {
                godot_warn!("SaveData: {path}: {err}; trying the backup");
                let backup = format!("{path}.bak");
                match read_model(&backup) {
                    Some(Ok(model)) => (model, true),
                    _ => (SaveModel::default(), false),
                }
            }
            None => (SaveModel::default(), true),
        };
        self.model = model;
        self.load_settings_fields();
        self.apply_all();
        self.signals().settings_changed().emit();
        ok
    }

    /// Writes the save now. Returns false on I/O failure.
    #[func]
    pub fn save(&mut self) -> bool {
        self.save_in = None;
        self.store_settings_fields();
        let path = self.save_path.to_string();
        let ok = write_atomic(&path, &self.model.to_json());
        if !ok {
            godot_error!("SaveData: could not write {path}");
        }
        ok
    }

    /// Forgets all progress and settings (in memory; call `save` to persist).
    #[func]
    pub fn reset_to_defaults(&mut self) {
        self.model = SaveModel::default();
        self.load_settings_fields();
        self.apply_all();
        self.signals().settings_changed().emit();
    }

    /// Keys: `best_score`, `best_rank` (`""` if none), `plays`, `clears`.
    #[func]
    pub fn get_record(&self, level_id: GString, mode: i32) -> VarDictionary {
        let record = self
            .model
            .record(&level_id.to_string(), DifficultyMode::from_i32(mode));
        let mut dict = VarDictionary::new();
        dict_set(&mut dict, "best_score", record.best_score);
        dict_set(&mut dict, "best_rank", GString::from(&record.best_rank));
        dict_set(&mut dict, "plays", record.plays as i64);
        dict_set(&mut dict, "clears", record.clears as i64);
        dict
    }

    /// Best rank across modes (`""` if never finished).
    #[func]
    pub fn get_best_rank(&self, level_id: GString) -> GString {
        self.model
            .best_rank_any_mode(&level_id.to_string())
            .map(|rank| GString::from(rank.as_str()))
            .unwrap_or_default()
    }

    /// Folds a finished run in and saves. Returns `new_best_score`, `new_best_rank`,
    /// `first_clear`, `previous_best_score`.
    #[func]
    pub fn record_result(
        &mut self,
        level_id: GString,
        mode: i32,
        score: i64,
        rank: GString,
        cleared: bool,
    ) -> VarDictionary {
        let rank = parse_rank(&rank.to_string()).unwrap_or(crate::core::scoring::Rank::D);
        let outcome = self.model.record_result(
            &level_id.to_string(),
            DifficultyMode::from_i32(mode),
            score,
            rank,
            cleared,
        );
        self.save();
        let mut dict = VarDictionary::new();
        dict_set(&mut dict, "new_best_score", outcome.new_best_score);
        dict_set(&mut dict, "new_best_rank", outcome.new_best_rank);
        dict_set(&mut dict, "first_clear", outcome.first_clear);
        dict_set(
            &mut dict,
            "previous_best_score",
            outcome.previous_best_score,
        );
        dict
    }

    #[func]
    pub fn is_level_unlocked(&self, level_id: GString) -> bool {
        let levels = level_catalog::all_levels();
        let order: Vec<&str> = levels.iter().map(|l| l.id.as_str()).collect();
        self.model.is_unlocked(
            &order,
            &level_id.to_string(),
            self.unlock_all || self.debug_unlocks_all,
        )
    }

    #[func]
    pub fn is_level_cleared(&self, level_id: GString) -> bool {
        self.model.cleared.contains(&level_id.to_string())
    }

    /// Applies the volume settings to the `Master`, `Music` and `SFX` buses.
    #[func]
    pub fn apply_audio(&self) {
        let mut server = AudioServer::singleton();
        for (bus, volume) in [
            (BUS_MASTER, self.master_volume),
            (BUS_MUSIC, self.music_volume),
            (BUS_SFX, self.sfx_volume),
        ] {
            let index = server.get_bus_index(bus);
            if index < 0 {
                continue;
            }
            server.set_bus_mute(index, volume <= 0.001);
            server.set_bus_volume_db(index, linear_to_db(volume) as f32);
        }
    }

    /// Applies `fullscreen` on desktop builds (no-op on web and headless).
    #[func]
    pub fn apply_display(&self) {
        if !fullscreen_supported() {
            return;
        }
        let mut display = DisplayServer::singleton();
        let current = display.window_get_mode();
        let fullscreen_now = matches!(
            current,
            WindowMode::FULLSCREEN | WindowMode::EXCLUSIVE_FULLSCREEN
        );
        if fullscreen_now != self.fullscreen {
            display.window_set_mode(if self.fullscreen {
                WindowMode::FULLSCREEN
            } else {
                WindowMode::WINDOWED
            });
        }
    }

    /// Whether the fullscreen setting does anything on this platform.
    #[func]
    pub fn fullscreen_supported() -> bool {
        fullscreen_supported()
    }
}

impl SaveData {
    fn settings_touched(&mut self) {
        self.apply_all();
        self.save_in = Some(SAVE_DELAY);
        if self.base().is_inside_tree() {
            self.signals().settings_changed().emit();
        }
    }

    fn apply_all(&self) {
        if !self.base().is_inside_tree() {
            return;
        }
        self.apply_audio();
        self.apply_display();
    }

    fn load_settings_fields(&mut self) {
        let s = &self.model.settings;
        self.master_volume = s.master_volume as f64;
        self.music_volume = s.music_volume as f64;
        self.sfx_volume = s.sfx_volume as f64;
        self.screen_shake = s.screen_shake;
        self.latency_offset_ms = s.latency_offset_ms as i64;
        self.fullscreen = s.fullscreen;
        self.show_fps = s.show_fps;
        self.unlock_all = s.unlock_all;
    }

    fn store_settings_fields(&mut self) {
        self.model.settings = Settings {
            master_volume: self.master_volume as f32,
            music_volume: self.music_volume as f32,
            sfx_volume: self.sfx_volume as f32,
            screen_shake: self.screen_shake,
            latency_offset_ms: self.latency_offset_ms as i32,
            fullscreen: self.fullscreen,
            show_fps: self.show_fps,
            unlock_all: self.unlock_all,
        };
    }
}

/// `SaveData` autoload, if registered.
pub fn save_data(node: &Gd<Node>) -> Option<Gd<SaveData>> {
    node.get_node_or_null(AUTOLOAD_PATH)
        .and_then(|n| n.try_cast::<SaveData>().ok())
}

fn fullscreen_supported() -> bool {
    let os = Os::singleton();
    !os.has_feature("web") && DisplayServer::singleton().get_name() != "headless"
}

fn linear_to_db(volume: f64) -> f64 {
    20.0 * volume.max(0.0001).log10()
}

/// `None` when the file does not exist.
fn read_model(path: &str) -> Option<Result<SaveModel, String>> {
    if !FileAccess::file_exists(path) {
        return None;
    }
    let text = FileAccess::get_file_as_string(path).to_string();
    let (model, error) = SaveModel::from_json(&text);
    Some(match error {
        Some(LoadError::Parse(err)) => Err(err),
        Some(LoadError::NewerVersion(version)) => {
            godot_warn!("SaveData: {path} is from a newer version ({version})");
            Ok(model)
        }
        None => Ok(model),
    })
}

fn write_atomic(path: &str, text: &str) -> bool {
    let tmp = format!("{path}.tmp");
    let Some(mut file) = FileAccess::open(&tmp, ModeFlags::WRITE) else {
        return false;
    };
    let stored = file.store_string(text);
    file.flush();
    file.close();
    if !stored {
        return false;
    }
    if FileAccess::file_exists(path) {
        let backup = format!("{path}.bak");
        if FileAccess::file_exists(&backup) {
            DirAccess::remove_absolute(&backup);
        }
        DirAccess::rename_absolute(path, &backup);
    }
    DirAccess::rename_absolute(&tmp, path) == godot::global::Error::OK
}
