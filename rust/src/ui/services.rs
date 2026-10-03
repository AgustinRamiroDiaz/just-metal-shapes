//! `Ui` autoload: UI sound effects, menu/preview music with a beat clock, focus
//! feedback for every focusable button, and the wipe transition between screens.
//!
//! It runs while the tree is paused and ignores `Engine.time_scale`.

use super::{make_tween, palette};
use crate::core::analysis::{SectionType, SongAnalysis};
use crate::level_catalog;
use crate::save::BUS_MUSIC;
use crate::save::BUS_SFX;
use godot::classes::node::ProcessMode;
use godot::classes::tween::{EaseType, TransitionType};
use godot::classes::{
    AudioServer, AudioStream, AudioStreamPlayer, BaseButton, CanvasLayer, Control, FileAccess,
    INode, InputEventKey, InputMap, Node, Polygon2D, Time,
};
use godot::global::Key;
use godot::prelude::*;
use std::collections::HashMap;

pub const AUTOLOAD_PATH: &str = "/root/Ui";

/// Title screen loop (Kevin MacLeod, CC BY 4.0; see the credits).
const MENU_TRACK: &str = "res://music/voxel-revolution.ogg";
/// Tempo of `MENU_TRACK` from `devtools/analyze_beats.py`, used when the track has no
/// `.analysis.json` next to it.
const MENU_BPM: f64 = 122.2;
const MENU_OFFSET: f64 = 0.403;
const MENU_VOLUME_DB: f32 = -6.0;
const PREVIEW_VOLUME_DB: f32 = -8.0;
const SILENT_DB: f32 = -50.0;
const CROSSFADE: f64 = 0.6;
const SFX_VOICES: usize = 8;
const FOCUS_SCALE: f32 = 1.05;
const WIPE_IN: f64 = 0.22;
const WIPE_OUT: f64 = 0.28;
const SCREEN: Vector2 = Vector2::new(1280.0, 720.0);
/// Horizontal lean of the wipe edge (matches the button skew).
const WIPE_LEAN: f32 = 160.0;

struct MusicTrack {
    path: String,
    bpm: f64,
    offset: f64,
    /// Where playback restarts when the stream ends.
    loop_from: f64,
}

#[derive(GodotClass)]
#[class(base = Node)]
pub struct UiServices {
    sfx_players: Vec<Gd<AudioStreamPlayer>>,
    next_voice: usize,
    sfx_cache: HashMap<String, Gd<AudioStream>>,
    /// Two players crossfade; `active` indexes the audible one.
    music: Vec<Gd<AudioStreamPlayer>>,
    active: usize,
    track: Option<MusicTrack>,
    music_time: f64,
    last_raw_position: f64,
    focused: Option<Gd<Control>>,
    wipe_layer: Option<Gd<CanvasLayer>>,
    wipe: Option<Gd<Polygon2D>>,
    transitioning: bool,
    accent: Color,
    base: Base<Node>,
}

#[godot_api]
impl INode for UiServices {
    fn init(base: Base<Node>) -> Self {
        Self {
            sfx_players: Vec::new(),
            next_voice: 0,
            sfx_cache: HashMap::new(),
            music: Vec::new(),
            active: 0,
            track: None,
            music_time: 0.0,
            last_raw_position: -1.0,
            focused: None,
            wipe_layer: None,
            wipe: None,
            transitioning: false,
            accent: palette::ACCENT,
            base,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_process_mode(ProcessMode::ALWAYS);
        for _ in 0..SFX_VOICES {
            let player = self.make_player(BUS_SFX);
            self.sfx_players.push(player);
        }
        for _ in 0..2 {
            let mut player = self.make_player(BUS_MUSIC);
            player.set_volume_db(SILENT_DB);
            self.music.push(player);
        }
        self.build_wipe();
        add_wasd_to_ui_actions();

        if let Some(mut viewport) = self.base().get_viewport() {
            let this = self.to_gd();
            viewport.connect("gui_focus_changed", &this.callable("_on_focus_changed"));
        }
    }

    fn process(&mut self, _delta: f64) {
        self.advance_music_clock();
    }

    /// Stops every player so no playback outlives the tree (the dummy audio driver never
    /// finishes them, which shows up as leaked streams at exit).
    fn exit_tree(&mut self) {
        for player in self.sfx_players.iter_mut().chain(self.music.iter_mut()) {
            player.stop();
            player.set_stream(Option::<&Gd<AudioStream>>::None);
        }
        self.sfx_cache.clear();
        self.track = None;
    }
}

#[godot_api]
impl UiServices {
    /// Plays `res://assets/sfx/<name>.ogg` on the SFX bus.
    #[func]
    pub fn play_sfx(&mut self, name: GString) {
        // The dummy driver (headless) never mixes, so a playback started here would never
        // be released and leaks at exit.
        if AudioServer::singleton().get_driver_name() == "Dummy" {
            return;
        }
        let Some(stream) = self.sfx_stream(&name.to_string()) else {
            return;
        };
        if self.sfx_players.is_empty() {
            return;
        }
        let index = self.next_voice % self.sfx_players.len();
        self.next_voice += 1;
        let player = &mut self.sfx_players[index];
        player.set_stream(&stream);
        player.play();
    }

    /// Loops the title track (no-op if it is already playing).
    #[func]
    pub fn play_menu_music(&mut self) {
        let track = track_info(MENU_TRACK, MENU_BPM, MENU_OFFSET, None);
        self.play_track(track, 0.0, MENU_VOLUME_DB);
    }

    /// Plays a level's song from its first main section, looping there.
    #[func]
    pub fn preview_level(&mut self, level_id: GString) {
        let Some(spec) = level_catalog::find_level(&level_id.to_string()) else {
            return;
        };
        let track = track_info(&spec.music_path, 120.0, 0.0, Some(&spec.analysis_path));
        let from = track.loop_from;
        self.play_track(track, from, PREVIEW_VOLUME_DB);
    }

    #[func]
    pub fn stop_music(&mut self) {
        self.track = None;
        for index in 0..self.music.len() {
            self.fade_player(index, SILENT_DB, CROSSFADE, true);
        }
    }

    /// Path of the playing music stream (empty when stopped).
    #[func]
    pub fn get_music_path(&self) -> GString {
        self.track
            .as_ref()
            .map(|t| GString::from(&t.path))
            .unwrap_or_default()
    }

    /// Fractional beat of the current music (0 when no music plays).
    #[func]
    pub fn music_beat(&self) -> f64 {
        match &self.track {
            Some(track) => (self.music_time - track.offset) * track.bpm / 60.0,
            None => Time::singleton().get_ticks_msec() as f64 / 1000.0 * MENU_BPM / 60.0,
        }
    }

    /// 1 on a beat, decaying to 0 before the next one.
    #[func]
    pub fn beat_pulse(&self) -> f64 {
        let phase = self.music_beat().rem_euclid(1.0);
        (1.0 - phase).powi(3)
    }

    /// Wipes to `scene_path`. Ignored while a transition runs.
    #[func]
    pub fn go_to(&mut self, scene_path: GString) {
        if self.transitioning {
            return;
        }
        self.transitioning = true;
        let Some(mut wipe) = self.wipe.clone() else {
            self.change_scene(scene_path);
            return;
        };
        if let Some(mut layer) = self.wipe_layer.clone() {
            layer.set_visible(true);
        }
        wipe.set_color(palette::VOID);
        wipe.set_position(Vector2::new(-(SCREEN.x + WIPE_LEAN * 2.0), 0.0));
        let Some(mut tween) = make_tween(&self.to_gd().upcast()) else {
            self.change_scene(scene_path);
            return;
        };
        tween
            .tween_property(&wipe, "position:x", &(-WIPE_LEAN).to_variant(), WIPE_IN)
            .set_trans(TransitionType::CUBIC)
            .set_ease(EaseType::IN);
        let this = self.to_gd();
        tween.tween_callback(
            &this
                .callable("_on_wipe_covered")
                .bind(&[scene_path.to_variant()]),
        );
    }

    #[func]
    pub fn is_transitioning(&self) -> bool {
        self.transitioning
    }

    /// Recolors the theme accent and remembers it for the wipe edge.
    #[func]
    pub fn set_accent(&mut self, color: Color) {
        self.accent = color;
        super::set_accent(color);
    }

    #[func]
    pub fn get_accent(&self) -> Color {
        self.accent
    }

    #[func]
    fn _on_wipe_covered(&mut self, scene_path: GString) {
        self.change_scene(scene_path);
        let Some(wipe) = self.wipe.clone() else {
            self.transitioning = false;
            return;
        };
        let Some(mut tween) = make_tween(&self.to_gd().upcast()) else {
            self.transitioning = false;
            return;
        };
        // One idle frame lets the new scene build before it is revealed.
        tween.tween_interval(0.05);
        tween
            .tween_property(
                &wipe,
                "position:x",
                &(SCREEN.x + WIPE_LEAN).to_variant(),
                WIPE_OUT,
            )
            .set_trans(TransitionType::CUBIC)
            .set_ease(EaseType::OUT);
        let this = self.to_gd();
        tween.tween_callback(&this.callable("_on_wipe_done"));
    }

    #[func]
    fn _on_wipe_done(&mut self) {
        self.transitioning = false;
        if let Some(mut layer) = self.wipe_layer.clone() {
            layer.set_visible(false);
        }
    }

    #[func]
    fn _on_focus_changed(&mut self, control: Gd<Control>) {
        let previous = self.focused.take().filter(|c| c.is_instance_valid());
        if let Some(mut previous) = previous.clone()
            && previous != control
            && previous.is_class("BaseButton")
        {
            if let Some(mut tween) = make_tween(&previous.clone().upcast()) {
                tween.tween_property(&previous, "scale", &Vector2::ONE.to_variant(), 0.1);
            } else {
                previous.set_scale(Vector2::ONE);
            }
        }
        if control.clone().try_cast::<BaseButton>().is_ok() {
            let mut target = control.clone();
            let size = target.get_size();
            target.set_pivot_offset(size * 0.5);
            if let Some(mut tween) = make_tween(&target.clone().upcast()) {
                tween
                    .tween_property(
                        &target,
                        "scale",
                        &Vector2::new(FOCUS_SCALE, FOCUS_SCALE).to_variant(),
                        0.14,
                    )
                    .set_trans(TransitionType::BACK)
                    .set_ease(EaseType::OUT);
            }
            // Only moves between two controls make a sound, not a screen's first focus.
            if previous.is_some_and(|p| p != control) && !self.transitioning {
                self.play_sfx("ui_move".into());
            }
        }
        self.focused = Some(control);
    }
}

impl UiServices {
    fn make_player(&mut self, bus: &str) -> Gd<AudioStreamPlayer> {
        let mut player = AudioStreamPlayer::new_alloc();
        if AudioServer::singleton().get_bus_index(bus) >= 0 {
            player.set_bus(bus);
        }
        self.base_mut().add_child(&player);
        player
    }

    fn sfx_stream(&mut self, name: &str) -> Option<Gd<AudioStream>> {
        if let Some(stream) = self.sfx_cache.get(name) {
            return Some(stream.clone());
        }
        let stream = try_load::<AudioStream>(&format!("res://assets/sfx/{name}.ogg")).ok()?;
        self.sfx_cache.insert(name.to_string(), stream.clone());
        Some(stream)
    }

    fn play_track(&mut self, track: MusicTrack, from: f64, volume_db: f32) {
        if self.track.as_ref().is_some_and(|t| t.path == track.path) {
            return;
        }
        // The dummy driver (headless) never mixes, so a started stream is never released
        // and leaks at exit; the frame clock keeps menu beats running without it.
        if AudioServer::singleton().get_driver_name() == "Dummy" {
            self.music_time = from;
            self.track = Some(track);
            return;
        }
        let Ok(stream) = try_load::<AudioStream>(&track.path) else {
            godot_warn!("Ui: missing music {}", track.path);
            return;
        };
        let previous = self.active;
        self.active = (self.active + 1) % self.music.len().max(1);
        if let Some(player) = self.music.get_mut(self.active) {
            player.stop();
            player.set_stream(&stream);
            player.set_volume_db(SILENT_DB);
            player.play_ex().from_position(from as f32).done();
        }
        self.fade_player(self.active, volume_db, CROSSFADE, false);
        self.fade_player(previous, SILENT_DB, CROSSFADE, true);
        self.music_time = from;
        self.last_raw_position = -1.0;
        self.track = Some(track);
    }

    fn fade_player(&mut self, index: usize, volume_db: f32, seconds: f64, stop_after: bool) {
        let Some(player) = self.music.get(index).cloned() else {
            return;
        };
        let Some(mut tween) = make_tween(&self.to_gd().upcast()) else {
            return;
        };
        tween.tween_property(&player, "volume_db", &volume_db.to_variant(), seconds);
        if stop_after {
            tween.tween_callback(&player.callable("stop"));
        }
    }

    /// Follows the playback position when audio runs, the frame clock otherwise
    /// (headless/dummy audio), and restarts the stream at its loop point when it ends.
    fn advance_music_clock(&mut self) {
        let Some(track) = &self.track else {
            return;
        };
        let loop_from = track.loop_from;
        let Some(mut player) = self.music.get(self.active).cloned() else {
            return;
        };
        let delta = self.base().get_process_delta_time()
            / godot::classes::Engine::singleton()
                .get_time_scale()
                .max(0.001);
        self.music_time += delta;
        if !player.is_playing() {
            if player.get_stream().is_some() && !self.transitioning {
                player.play_ex().from_position(loop_from as f32).done();
                self.music_time = loop_from;
            }
            return;
        }
        let raw = player.get_playback_position() as f64;
        if raw != self.last_raw_position {
            self.last_raw_position = raw;
            let server = AudioServer::singleton();
            let heard = raw + server.get_time_since_last_mix() - server.get_output_latency();
            if (heard - self.music_time).abs() > 0.1 {
                self.music_time = heard;
            } else {
                self.music_time += (heard - self.music_time) * 0.1;
            }
        }
    }

    fn build_wipe(&mut self) {
        let mut layer = CanvasLayer::new_alloc();
        layer.set_layer(128);
        layer.set_visible(false);
        self.base_mut().add_child(&layer);
        let mut wipe = Polygon2D::new_alloc();
        let width = SCREEN.x + WIPE_LEAN * 2.0;
        wipe.set_polygon(&PackedVector2Array::from(&[
            Vector2::new(WIPE_LEAN, 0.0),
            Vector2::new(width + WIPE_LEAN, 0.0),
            Vector2::new(width, SCREEN.y),
            Vector2::new(0.0, SCREEN.y),
        ]));
        wipe.set_color(palette::VOID);
        // Accent edge leading the wipe.
        let mut edge = Polygon2D::new_alloc();
        edge.set_polygon(&PackedVector2Array::from(&[
            Vector2::new(width + WIPE_LEAN, 0.0),
            Vector2::new(width + WIPE_LEAN + 14.0, 0.0),
            Vector2::new(width + 14.0, SCREEN.y),
            Vector2::new(width, SCREEN.y),
        ]));
        edge.set_color(palette::ACCENT);
        edge.set_name("Edge");
        wipe.add_child(&edge);
        layer.add_child(&wipe);
        self.wipe_layer = Some(layer);
        self.wipe = Some(wipe);
    }

    fn change_scene(&mut self, scene_path: GString) {
        if let Some(mut edge) = self
            .wipe
            .as_ref()
            .and_then(|w| w.try_get_node_as::<Polygon2D>("Edge"))
        {
            edge.set_color(self.accent);
        }
        self.focused = None;
        let mut tree = self.base().get_tree();
        tree.set_pause(false);
        let err = tree.change_scene_to_file(&scene_path);
        if err != godot::global::Error::OK {
            godot_error!("Ui: could not open {scene_path}: {err:?}");
            self.transitioning = false;
        }
    }
}

/// Tempo and loop point for a music file, from its analysis when available.
fn track_info(path: &str, bpm: f64, offset: f64, analysis_path: Option<&str>) -> MusicTrack {
    let analysis_path = analysis_path
        .map(str::to_string)
        .unwrap_or_else(|| path.replace(".ogg", ".analysis.json"));
    let analysis = FileAccess::file_exists(&analysis_path)
        .then(|| FileAccess::get_file_as_string(&analysis_path).to_string())
        .and_then(|json| SongAnalysis::from_json(&json).ok());
    match analysis {
        Some(analysis) => {
            let main_start = analysis
                .sections
                .iter()
                .find(|s| s.section_type == SectionType::Main)
                .map(|s| analysis.beat_offset_seconds + s.start_beat as f64 * 60.0 / analysis.bpm)
                .unwrap_or(analysis.duration_seconds * 0.3);
            MusicTrack {
                path: path.to_string(),
                bpm: analysis.bpm,
                offset: analysis.beat_offset_seconds,
                loop_from: main_start.min(analysis.duration_seconds * 0.6),
            }
        }
        None => MusicTrack {
            path: path.to_string(),
            bpm,
            offset,
            loop_from: 0.0,
        },
    }
}

/// WASD also navigates menus (keyboard player one already has a hand there).
fn add_wasd_to_ui_actions() {
    let mut map = InputMap::singleton();
    for (action, key) in [
        ("ui_up", Key::W),
        ("ui_down", Key::S),
        ("ui_left", Key::A),
        ("ui_right", Key::D),
    ] {
        if !map.has_action(action) {
            continue;
        }
        let mut event = InputEventKey::new_gd();
        event.set_physical_keycode(key);
        if !map.action_has_event(action, &event) {
            map.action_add_event(action, &event);
        }
    }
}
