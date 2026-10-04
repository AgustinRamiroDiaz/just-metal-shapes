//! `LevelDirector`: loads a level's analysis, generates its chart, and plays the chart
//! against the `Conductor`.
//!
//! Each frame it spawns every event whose spawn beat (`beat - telegraph_beats`) has
//! been reached, dispatching through a registry keyed by `EventKind`. Hazard and enemy
//! kinds create nodes; presentation kinds emit signals on the director for other
//! systems (camera, Fx, HUD) to connect to. Kinds without a handler are logged once and
//! skipped.

use crate::conductor::Conductor;
use crate::core::analysis::SongAnalysis;
use crate::core::chart::{Chart, ChartEvent, EventKind, HINTS, PatternParams};
use crate::core::chart_gen::{LevelSpec, generate_chart, section_type_index};
use crate::core::mode::DifficultyMode;
use crate::core::timing::Timing;
use crate::game_config::GameConfig;
use crate::level_catalog::{self, rgb_to_color};
use crate::util::dict_set;
use crate::{enemy_spawn, groups, hazards};
use godot::classes::{AudioStream, FileAccess, INode2D, Node, Node2D, PackedScene, ResourceLoader};
use godot::prelude::*;
use std::collections::{HashMap, HashSet};

/// Handler for one `EventKind`. Receives the director (for helpers and signals) and
/// the event at its spawn beat.
pub type SpawnFn = fn(&mut LevelDirector, &ChartEvent);

const ARENA_SIZE: Vector2 = Vector2::new(1280.0, 720.0);

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct LevelDirector {
    #[export]
    #[init(val = NodePath::from("../Conductor"))]
    pub conductor_path: NodePath,
    /// Chart seed; -1 uses the level's own seed.
    #[export]
    #[init(val = -1)]
    pub seed_override: i64,
    /// Stops dispatching events (level over).
    #[var]
    #[init(val = true)]
    pub active: bool,

    level: Option<LevelSpec>,
    mode: DifficultyMode,
    chart: Option<Chart>,
    #[init(val = Timing::new(120.0, 0.0))]
    timing: Timing,
    /// Event indices in spawn order.
    order: Vec<usize>,
    cursor: usize,
    seen_seek_count: i64,
    /// Checkpoint beat of the last rewind; the song replays from a little before it, and
    /// a second rewind in that lead-in must not fall back to an earlier checkpoint.
    rewind_floor: f64,
    /// Highest checkpoint index announced, so replays after a rewind stay quiet.
    #[init(val = -1)]
    announced_checkpoint: i64,
    registry: HashMap<EventKind, SpawnFn>,
    warned: HashSet<EventKind>,
    conductor: Option<Gd<Conductor>>,
    enemy_scenes: Vec<Option<Gd<PackedScene>>>,
    spawn_effect_scene: Option<Gd<PackedScene>>,
    #[init(val = Rect2::new(Vector2::ZERO, ARENA_SIZE))]
    arena: Rect2,

    base: Base<Node2D>,
}

#[godot_api]
impl LevelDirector {
    /// Every dispatched event: kind name, hit beat, and the song beat it spawned at.
    #[signal]
    pub fn event_spawned(kind: GString, beat: f64, song_beat: f64);
    /// Downbeat pulse; `intensity` is the bar intensity (0..1).
    #[signal]
    pub fn arena_pulse(beat: f64, intensity: f64);
    /// Strong accent; `strength` 0..1.
    #[signal]
    pub fn camera_kick(strength: f64);
    #[signal]
    pub fn flash(color: Color, duration_seconds: f64);
    /// Section change; `section_type` is intro|build|main|breakdown|outro.
    #[signal]
    pub fn palette_shift(section_type: GString, intensity: f64);
    #[signal]
    pub fn checkpoint_reached(index: i64, beat: f64);
    #[signal]
    pub fn show_hint(text: GString, duration_seconds: f64);
    /// A lyric cue's word at `position` (arena px).
    #[signal]
    pub fn caption(text: GString, position: Vector2, duration_seconds: f64);
    #[signal]
    pub fn enemy_spawned(enemy: Gd<Node2D>);
    #[signal]
    pub fn enemy_died();
    /// After `rewind_to_checkpoint`: the checkpoint beat the song resumed from.
    #[signal]
    pub fn rewound(beat: f64);

    /// Loads a level by id (empty: `GameConfig.selected_level_id`, then the first
    /// level), generates its chart for the current `GameConfig.difficulty_mode`, and
    /// configures the Conductor. Returns false (and logs) on failure.
    #[func]
    pub fn load_level(&mut self, level_id: GString) -> bool {
        let config = self.base().get_node_or_null("/root/GameConfig");
        let mut id = level_id.to_string();
        if id.is_empty()
            && let Some(config) = &config
        {
            id = config.get("selected_level_id").to::<GString>().to_string();
        }
        if id.is_empty() {
            id = level_catalog::first_level_id();
        }
        let Some(spec) = level_catalog::find_level(&id) else {
            godot_error!("LevelDirector: unknown level id '{id}'");
            return false;
        };
        let mode = config
            .as_ref()
            .map(|c| DifficultyMode::from_i32(c.get("difficulty_mode").to::<i32>()))
            .unwrap_or_default();

        let json = FileAccess::get_file_as_string(&spec.analysis_path);
        if json.is_empty() {
            godot_error!(
                "LevelDirector: could not read analysis '{}'",
                spec.analysis_path
            );
            return false;
        }
        let analysis = match SongAnalysis::from_json(&json.to_string()) {
            Ok(analysis) => analysis,
            Err(err) => {
                godot_error!("LevelDirector: {}: {err}", spec.analysis_path);
                return false;
            }
        };

        let mut tuned = spec.clone();
        tuned.density *= mode.density_scale();
        let seed = if self.seed_override >= 0 {
            self.seed_override as u64
        } else {
            spec.seed
        };
        let chart = generate_chart(&analysis, &tuned, seed);

        let mut loader = ResourceLoader::singleton();
        self.enemy_scenes = spec
            .enemy_pool
            .iter()
            .map(|entry| {
                loader
                    .load(&entry.scene)
                    .and_then(|r| r.try_cast::<PackedScene>().ok())
            })
            .collect();
        self.spawn_effect_scene = loader
            .load("res://scenes/spawn_effect.tscn")
            .and_then(|r| r.try_cast::<PackedScene>().ok());

        self.conductor = self
            .base()
            .try_get_node_as::<Conductor>(&self.conductor_path);
        if let Some(conductor) = self.conductor.as_mut() {
            let stream = loader
                .load(&spec.music_path)
                .and_then(|r| r.try_cast::<AudioStream>().ok());
            if stream.is_none() {
                godot_warn!(
                    "LevelDirector: no music at '{}', using clock",
                    spec.music_path
                );
            }
            conductor
                .bind_mut()
                .configure_from_analysis(&analysis, stream);
            self.seen_seek_count = conductor.bind().get_seek_count();
        }

        self.order = chart.spawn_order();
        self.cursor = 0;
        self.rewind_floor = 0.0;
        self.announced_checkpoint = -1;
        self.timing = chart.timing();
        self.chart = Some(chart);
        self.level = Some(spec);
        self.mode = mode;
        true
    }

    #[func]
    pub fn get_level_id(&self) -> GString {
        self.level
            .as_ref()
            .map(|spec| GString::from(&spec.id))
            .unwrap_or_default()
    }

    /// The `LevelCatalog` dictionary for the loaded level (empty if none).
    #[func]
    pub fn get_level_info(&self) -> VarDictionary {
        let Some(spec) = &self.level else {
            return VarDictionary::new();
        };
        let index = level_catalog::all_levels()
            .iter()
            .position(|l| l.id == spec.id)
            .unwrap_or(0);
        level_catalog::level_to_dictionary(spec, index)
    }

    #[func]
    pub fn get_mode(&self) -> i32 {
        self.mode.as_i32()
    }

    #[func]
    pub fn get_event_count(&self) -> i64 {
        self.chart.as_ref().map_or(0, |c| c.events.len() as i64)
    }

    /// Number of events already dispatched (position in spawn order).
    #[func]
    pub fn get_cursor(&self) -> i64 {
        self.cursor as i64
    }

    /// Chart events as dictionaries, sorted by hit beat. Keys: `kind`, `beat`,
    /// `telegraph_beats`, `spawn_beat`, `x`, `y`, `angle`, `count`, `speed`, `size`,
    /// `duration_beats`, `color_index`, `variant`, `intensity`.
    #[func]
    pub fn get_events(&self) -> Array<VarDictionary> {
        let Some(chart) = &self.chart else {
            return Array::new();
        };
        chart.events.iter().map(event_to_dictionary).collect()
    }

    /// Chart summary: `bpm`, `offset_seconds`, `duration_seconds`, `event_count`,
    /// `hazard_count`, `enemy_count`, `checkpoint_count`.
    #[func]
    pub fn get_chart_summary(&self) -> VarDictionary {
        let mut dict = VarDictionary::new();
        if let Some(chart) = &self.chart {
            dict_set(&mut dict, "bpm", chart.bpm);
            dict_set(&mut dict, "offset_seconds", chart.offset_seconds);
            dict_set(&mut dict, "duration_seconds", chart.duration_seconds);
            dict_set(&mut dict, "event_count", chart.events.len() as i64);
            let hazards = chart.events.iter().filter(|e| e.kind.is_hazard()).count();
            dict_set(&mut dict, "hazard_count", hazards as i64);
            dict_set(
                &mut dict,
                "enemy_count",
                chart.count_kind(EventKind::SpawnEnemy) as i64,
            );
            dict_set(
                &mut dict,
                "checkpoint_count",
                chart.count_kind(EventKind::Checkpoint) as i64,
            );
        }
        dict
    }

    #[func]
    pub fn get_checkpoint_beats(&self) -> PackedFloat64Array {
        self.chart
            .as_ref()
            .map(|c| PackedFloat64Array::from(c.checkpoint_beats().as_slice()))
            .unwrap_or_default()
    }

    /// Latest checkpoint beat at or before the current song beat.
    #[func]
    pub fn current_checkpoint_beat(&self) -> f64 {
        let beat = self.song_beat();
        self.chart
            .as_ref()
            .map_or(0.0, |c| c.checkpoint_at_or_before(beat))
            .max(self.rewind_floor)
    }

    /// Clears the arena, seeks the Conductor back to the last reached checkpoint and
    /// replays the chart from there. Playback resumes early enough that events hitting
    /// at or after the checkpoint get their full warning. Returns the checkpoint beat.
    #[func]
    pub fn rewind_to_checkpoint(&mut self) -> f64 {
        let beat = self.current_checkpoint_beat();
        let start = self
            .chart
            .as_ref()
            .map_or(beat, |c| c.replay_start_beat(beat));
        self.rewind_floor = beat;
        self.clear_arena();
        if let Some(conductor) = self.conductor.as_mut() {
            let seconds = conductor.bind().beat_to_time(start).max(0.0);
            conductor.bind_mut().seek(seconds);
            self.seen_seek_count = conductor.bind().get_seek_count();
        }
        self.jump_to_beat(start);
        self.signals().rewound().emit(beat);
        beat
    }

    /// Frees hazards, enemies, pending enemy spawns, enemy projectiles and mines.
    #[func]
    pub fn clear_arena(&mut self) {
        let tree = self.base().get_tree();
        for group in [
            groups::HAZARDS,
            groups::ENEMIES,
            groups::SPAWN_EFFECTS,
            groups::ENEMY_PROJECTILES,
            groups::MINES,
        ] {
            for mut node in tree.get_nodes_in_group(group).iter_shared() {
                // Leave the group now so counts are correct before the free happens.
                node.remove_from_group(group);
                node.queue_free();
            }
        }
    }

    /// Moves the cursor so the next dispatched event is the first spawning at or
    /// after `beat`.
    #[func]
    pub fn jump_to_beat(&mut self, beat: f64) {
        let Some(chart) = &self.chart else {
            return;
        };
        self.cursor = self
            .order
            .iter()
            .position(|&i| chart.events[i].spawn_beat() >= beat - 1e-6)
            .unwrap_or(self.order.len());
    }

    /// Names of `EventKind`s with a registered handler.
    #[func]
    pub fn get_implemented_kinds(&self) -> PackedStringArray {
        let mut kinds: Vec<EventKind> = self.registry.keys().copied().collect();
        kinds.sort();
        kinds.iter().map(|k| GString::from(k.name())).collect()
    }

    /// Dispatches one event now, outside the chart (tests, previews, debug tools).
    /// `params` uses the `get_events` keys; missing keys keep `PatternParams` defaults.
    /// Without a loaded level, beats use a 120 BPM grid starting at 0 s and hazards
    /// run on their own clock from their spawn time. Returns false for unknown kinds.
    #[func]
    pub fn spawn_event(
        &mut self,
        kind: GString,
        beat: f64,
        telegraph_beats: f64,
        params: VarDictionary,
    ) -> bool {
        let Some(kind) = EventKind::from_name(&kind.to_string()) else {
            return false;
        };
        let event = ChartEvent {
            beat,
            telegraph_beats,
            kind,
            params: params_from_dictionary(&params),
        };
        self.dispatch(&event);
        true
    }

    #[func]
    fn _on_enemy_died(&mut self) {
        self.signals().enemy_died().emit();
    }
}

/// Helpers for `SpawnFn`s.
impl LevelDirector {
    pub fn level(&self) -> Option<&LevelSpec> {
        self.level.as_ref()
    }

    pub fn chart(&self) -> Option<&Chart> {
        self.chart.as_ref()
    }

    pub fn timing(&self) -> Timing {
        self.timing.clone()
    }

    pub fn conductor(&self) -> Option<Gd<Conductor>> {
        self.conductor.clone()
    }

    pub fn song_beat(&self) -> f64 {
        self.conductor
            .as_ref()
            .map_or(0.0, |c| c.bind().song_beat())
    }

    /// Arena rectangle in global pixels.
    pub fn arena_rect(&self) -> Rect2 {
        self.arena
    }

    /// Normalized `0..1` arena coordinates to global pixels.
    pub fn arena_point(&self, x: f32, y: f32) -> Vector2 {
        self.arena.position + self.arena.size * Vector2::new(x, y)
    }

    /// A `PatternParams::size`/`speed` value (fraction of arena height) in pixels.
    pub fn arena_length(&self, fraction: f32) -> f32 {
        fraction * self.arena.size.y
    }

    pub fn accent_color(&self) -> Color {
        self.level
            .as_ref()
            .map_or(Color::from_rgb(1.0, 0.2, 0.5), |l| {
                rgb_to_color(l.palette.accent)
            })
    }

    pub fn background_color(&self) -> Color {
        self.level
            .as_ref()
            .map_or(Color::from_rgb(0.05, 0.05, 0.08), |l| {
                rgb_to_color(l.palette.bg)
            })
    }

    /// The level's hazard color (neon pink unless the palette says otherwise).
    pub fn danger_color(&self) -> Color {
        self.level
            .as_ref()
            .map_or(Color::from_rgb(1.0, 0.15, 0.5), |l| {
                rgb_to_color(l.palette.danger)
            })
    }

    /// Hazard color for a `color_index`: the danger color, varied slightly so mirrored
    /// halves of a pattern read as a pair.
    pub fn hazard_color(&self, color_index: u32) -> Color {
        let danger = self.danger_color();
        match color_index % 4 {
            0 => danger,
            1 => danger.lightened(0.12),
            2 => danger.lightened(0.3),
            _ => self.accent_color(),
        }
    }

    /// Global positions of living players (aim targets for `Barrage`).
    pub fn alive_player_positions(&self) -> Vec<Vector2> {
        self.base()
            .get_tree()
            .get_nodes_in_group(groups::PLAYERS)
            .iter_shared()
            .filter_map(|node| node.try_cast::<Node2D>().ok())
            .filter(|player| !player.get("is_dead").try_to::<bool>().unwrap_or(false))
            .map(|player| player.get_global_position())
            .collect()
    }

    /// Adds a hazard node under the director and puts it in the `hazards` and
    /// `danger` groups.
    pub fn add_hazard(&mut self, node: Gd<impl Inherits<Node>>) {
        let mut node = node.upcast::<Node>();
        node.add_to_group(groups::HAZARDS);
        node.add_to_group(groups::DANGER);
        self.base_mut().add_child(&node);
    }

    /// Node that enemies and other level-wide spawns are added to.
    pub fn level_root(&self) -> Gd<Node> {
        self.base()
            .get_parent()
            .unwrap_or_else(|| self.to_gd().upcast())
    }

    pub fn enemy_scene(&self, index: usize) -> Option<Gd<PackedScene>> {
        self.enemy_scenes.get(index).cloned().flatten()
    }

    pub fn spawn_effect_scene(&self) -> Option<Gd<PackedScene>> {
        self.spawn_effect_scene.clone()
    }

    fn register_builtin(&mut self) {
        hazards::register_all(&mut self.registry);
        self.registry
            .insert(EventKind::SpawnEnemy, enemy_spawn::spawn_enemy);
        self.registry.insert(EventKind::ArenaPulse, |d, e| {
            d.signals()
                .arena_pulse()
                .emit(e.beat, e.params.intensity as f64);
        });
        self.registry.insert(EventKind::CameraKick, |d, e| {
            d.signals().camera_kick().emit(e.params.intensity as f64);
        });
        self.registry.insert(EventKind::Flash, |d, e| {
            let color = d.accent_color();
            let seconds = d.timing().beats_to_duration(e.params.duration_beats);
            d.signals().flash().emit(color, seconds);
        });
        self.registry.insert(EventKind::PaletteShift, |d, e| {
            let name = crate::core::analysis::SectionType::ALL
                .iter()
                .find(|t| section_type_index(**t) == e.params.variant)
                .map_or("main", |t| t.as_str());
            d.signals()
                .palette_shift()
                .emit(&GString::from(name), e.params.intensity as f64);
        });
        self.registry.insert(EventKind::Checkpoint, |d, e| {
            let index = e.params.variant as i64;
            if index <= d.announced_checkpoint {
                return;
            }
            d.announced_checkpoint = index;
            d.signals().checkpoint_reached().emit(index, e.beat);
        });
        self.registry.insert(EventKind::Caption, |d, e| {
            let Some(text) = d
                .level()
                .and_then(|level| level.captions.get(e.params.variant as usize))
                .map(GString::from)
            else {
                return;
            };
            let position = d.arena_point(e.params.x, e.params.y);
            let seconds = d.timing().beats_to_duration(e.params.duration_beats);
            d.signals().caption().emit(&text, position, seconds);
        });
        self.registry.insert(EventKind::ShowHint, |d, e| {
            let text = HINTS.get(e.params.variant as usize).copied().unwrap_or("");
            let seconds = d.timing().beats_to_duration(e.params.duration_beats);
            d.signals().show_hint().emit(&GString::from(text), seconds);
        });
    }

    fn dispatch(&mut self, event: &ChartEvent) {
        let beat = self.song_beat();
        self.signals()
            .event_spawned()
            .emit(&GString::from(event.kind.name()), event.beat, beat);
        match self.registry.get(&event.kind).copied() {
            Some(handler) => handler(self, event),
            None => {
                if self.warned.insert(event.kind) {
                    godot_print!(
                        "LevelDirector: no handler for {}, skipping these events",
                        event.kind.name()
                    );
                }
            }
        }
    }
}

fn event_to_dictionary(event: &ChartEvent) -> VarDictionary {
    let mut dict = VarDictionary::new();
    let p = &event.params;
    dict_set(&mut dict, "kind", GString::from(event.kind.name()));
    dict_set(&mut dict, "beat", event.beat);
    dict_set(&mut dict, "telegraph_beats", event.telegraph_beats);
    dict_set(&mut dict, "spawn_beat", event.spawn_beat());
    dict_set(&mut dict, "x", p.x);
    dict_set(&mut dict, "y", p.y);
    dict_set(&mut dict, "angle", p.angle);
    dict_set(&mut dict, "count", p.count as i64);
    dict_set(&mut dict, "speed", p.speed);
    dict_set(&mut dict, "size", p.size);
    dict_set(&mut dict, "duration_beats", p.duration_beats);
    dict_set(&mut dict, "color_index", p.color_index as i64);
    dict_set(&mut dict, "variant", p.variant as i64);
    dict_set(&mut dict, "intensity", p.intensity);
    dict
}

fn params_from_dictionary(dict: &VarDictionary) -> PatternParams {
    let f32_of = |key: &str, default: f32| {
        dict.get(key)
            .and_then(|v| v.try_to::<f64>().ok())
            .map_or(default, |v| v as f32)
    };
    let u32_of = |key: &str, default: u32| {
        dict.get(key)
            .and_then(|v| v.try_to::<i64>().ok())
            .map_or(default, |v| v.max(0) as u32)
    };
    let d = PatternParams::default();
    PatternParams {
        x: f32_of("x", d.x),
        y: f32_of("y", d.y),
        angle: f32_of("angle", d.angle),
        count: u32_of("count", d.count),
        speed: f32_of("speed", d.speed),
        size: f32_of("size", d.size),
        duration_beats: f32_of("duration_beats", d.duration_beats as f32) as f64,
        color_index: u32_of("color_index", d.color_index),
        variant: u32_of("variant", d.variant),
        intensity: f32_of("intensity", d.intensity),
    }
}

#[godot_api]
impl INode2D for LevelDirector {
    fn ready(&mut self) {
        self.register_builtin();
        if let Some(viewport) = self.base().get_viewport() {
            let rect = viewport.get_visible_rect();
            if rect.size.x > 0.0 && rect.size.y > 0.0 {
                self.arena = rect;
            }
        }
    }

    fn process(&mut self, _delta: f64) {
        if !self.active {
            return;
        }
        let Some(conductor) = self.conductor.clone() else {
            return;
        };
        let (playing, beat, seek_count, seek_beat) = {
            let c = conductor.bind();
            let seek_beat = c.time_to_beat(c.get_last_seek_time());
            (c.is_playing(), c.song_beat(), c.get_seek_count(), seek_beat)
        };
        if seek_count != self.seen_seek_count {
            // Resync from where the song jumped to, so events at that beat still play.
            self.seen_seek_count = seek_count;
            self.jump_to_beat(seek_beat);
        }
        if !playing {
            return;
        }
        while self.cursor < self.order.len() && self.active {
            let Some(event) = self
                .chart
                .as_ref()
                .map(|c| c.events[self.order[self.cursor]].clone())
            else {
                return;
            };
            if event.spawn_beat() > beat {
                break;
            }
            self.cursor += 1;
            self.dispatch(&event);
        }
    }
}

/// Reads `GameConfig.difficulty_mode` for systems outside the director.
pub fn current_mode(node: &Gd<Node>) -> DifficultyMode {
    node.get_node_or_null("/root/GameConfig")
        .and_then(|c| c.try_cast::<GameConfig>().ok())
        .map(|c| DifficultyMode::from_i32(c.bind().difficulty_mode))
        .unwrap_or_default()
}
