//! `Conductor`: the single source of song time.
//!
//! Time comes from the music's playback position with latency compensation
//! (`core::timing::audio_song_time`), smoothed against an internal clock so it is
//! monotonic between audio mixes. The internal clock advances by the frame `delta`,
//! which Godot already scales by `Engine.time_scale`. The clock alone is used when
//! `use_clock` is set, when there is no stream, or when the audio driver is the dummy
//! driver / playback does not advance (headless runs).

use crate::core::analysis::SongAnalysis;
use crate::core::timing::{Timing, audio_song_time};
use godot::classes::{AudioServer, AudioStream, AudioStreamPlayer, INode, Node};
use godot::prelude::*;

/// Audio and clock may disagree by this much before the clock snaps to audio.
const SNAP_THRESHOLD: f64 = 0.1;
/// Fraction of the audio/clock drift corrected per frame below the snap threshold.
const DRIFT_CORRECTION: f64 = 0.1;
/// Playback position frozen for this long (clock seconds) means audio is not running.
const STALL_LIMIT: f64 = 0.5;

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct Conductor {
    /// Ignore audio and drive song time from the frame clock (tests, headless).
    #[export]
    pub use_clock: bool,
    /// User audio latency calibration in seconds; positive delays the game clock.
    #[export]
    pub latency_offset: f64,

    #[init(val = Timing::new(120.0, 0.0))]
    timing: Timing,
    #[init(val = 60.0)]
    duration_seconds: f64,
    /// (start beat, section type) per section, ascending.
    sections: Vec<(i64, String)>,

    player: Option<Gd<AudioStreamPlayer>>,
    clock_time: f64,
    running: bool,
    paused: bool,
    finished: bool,
    /// Last beat index already signalled.
    #[init(val = -1)]
    last_beat: i64,
    /// Bumped on every seek/play/stop so listeners can detect discontinuities.
    seek_count: i64,
    /// Song time the last play/seek jumped to.
    last_seek_time: f64,
    audio_failed: bool,
    /// The dummy driver (headless) never mixes, so playback would neither advance nor
    /// be released.
    dummy_driver: bool,
    last_audio_position: f64,
    stalled_for: f64,

    base: Base<Node>,
}

#[godot_api]
impl Conductor {
    #[signal]
    pub fn beat(index: i64);
    #[signal]
    pub fn bar(index: i64);
    #[signal]
    pub fn section_started(index: i64, section_type: GString);
    #[signal]
    pub fn song_finished();
    /// Emitted after `play`, `seek` and rewinds with the new song time.
    #[signal]
    pub fn seeked(seconds: f64);
    #[signal]
    pub fn paused_changed(paused: bool);

    /// Configures timing and the music stream. `stream` may be null (clock only).
    #[func]
    pub fn setup(
        &mut self,
        stream: Option<Gd<AudioStream>>,
        bpm: f64,
        offset_seconds: f64,
        duration_seconds: f64,
    ) {
        self.stop();
        self.timing = Timing::new(bpm.max(1.0), offset_seconds);
        self.duration_seconds = duration_seconds.max(0.0);
        let mut player = self.ensure_player();
        player.set_stream(stream.as_ref());
        self.audio_failed = false;
    }

    /// Section start beats and types (`intro|build|main|breakdown|outro`).
    #[func]
    pub fn set_sections(&mut self, start_beats: PackedInt64Array, types: PackedStringArray) {
        self.sections = start_beats
            .as_slice()
            .iter()
            .zip(types.as_slice())
            .map(|(beat, kind)| (*beat, kind.to_string()))
            .collect();
        self.sections.sort_by_key(|(beat, _)| *beat);
    }

    /// Starts the song at `from_seconds`.
    #[func]
    pub fn play(&mut self, #[opt(default = 0.0)] from_seconds: f64) {
        self.running = true;
        self.paused = false;
        self.finished = false;
        self.stalled_for = 0.0;
        self.last_audio_position = -1.0;
        self.jump_clock(from_seconds);
        if self.audio_enabled() {
            let mut player = self.ensure_player();
            player.set_stream_paused(false);
            player.play_ex().from_position(from_seconds as f32).done();
        }
        let seconds = self.clock_time;
        self.signals().seeked().emit(seconds);
    }

    #[func]
    pub fn stop(&mut self) {
        self.running = false;
        self.paused = false;
        self.seek_count += 1;
        if let Some(player) = self.player.as_mut() {
            player.stop();
        }
    }

    #[func]
    pub fn pause(&mut self) {
        if !self.running || self.paused {
            return;
        }
        self.paused = true;
        if let Some(player) = self.player.as_mut() {
            player.set_stream_paused(true);
        }
        self.signals().paused_changed().emit(true);
    }

    #[func]
    pub fn resume(&mut self) {
        if !self.running || !self.paused {
            return;
        }
        self.paused = false;
        if let Some(player) = self.player.as_mut() {
            player.set_stream_paused(false);
        }
        self.signals().paused_changed().emit(false);
    }

    /// Jumps to `seconds` (checkpoints, debugging). Beats from the new position are
    /// signalled again, including a beat that lands exactly on `seconds`.
    #[func]
    pub fn seek(&mut self, seconds: f64) {
        let seconds = seconds.clamp(0.0, self.duration_seconds);
        self.finished = false;
        self.jump_clock(seconds);
        if self.running && self.audio_enabled() {
            let mut player = self.ensure_player();
            if player.is_playing() {
                player.seek(seconds as f32);
            } else {
                player.play_ex().from_position(seconds as f32).done();
                player.set_stream_paused(self.paused);
            }
            self.stalled_for = 0.0;
            self.last_audio_position = -1.0;
        }
        self.signals().seeked().emit(seconds);
    }

    /// Current song time in seconds (negative never; 0 before `play`).
    #[func]
    pub fn song_time(&self) -> f64 {
        self.clock_time
    }

    /// Current fractional beat (negative before the first beat).
    #[func]
    pub fn song_beat(&self) -> f64 {
        self.timing.seconds_to_beat(self.clock_time)
    }

    #[func]
    pub fn song_bar(&self) -> i64 {
        self.timing.bar_of_beat(self.song_beat())
    }

    #[func]
    pub fn beat_to_time(&self, beat: f64) -> f64 {
        self.timing.beat_to_seconds(beat)
    }

    #[func]
    pub fn time_to_beat(&self, seconds: f64) -> f64 {
        self.timing.seconds_to_beat(seconds)
    }

    #[func]
    pub fn seconds_per_beat(&self) -> f64 {
        self.timing.seconds_per_beat()
    }

    #[func]
    pub fn get_bpm(&self) -> f64 {
        self.timing.bpm
    }

    #[func]
    pub fn get_duration(&self) -> f64 {
        self.duration_seconds
    }

    /// Song progress `0..=1`.
    #[func]
    pub fn progress(&self) -> f64 {
        if self.duration_seconds <= 0.0 {
            return 0.0;
        }
        (self.clock_time / self.duration_seconds).clamp(0.0, 1.0)
    }

    /// True while the song is advancing (started, not paused, not finished).
    #[func]
    pub fn is_playing(&self) -> bool {
        self.running && !self.paused && !self.finished
    }

    #[func]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    #[func]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Whether song time is currently derived from audio playback.
    #[func]
    pub fn is_using_audio_clock(&self) -> bool {
        self.audio_enabled()
    }

    /// Incremented on every play/seek/stop; compare to detect time jumps.
    #[func]
    pub fn get_seek_count(&self) -> i64 {
        self.seek_count
    }

    /// Song time (seconds) the most recent `play`/`seek` jumped to.
    #[func]
    pub fn get_last_seek_time(&self) -> f64 {
        self.last_seek_time
    }

    /// Section index containing `beat` (-1 if there are no sections).
    #[func]
    pub fn section_index_at(&self, beat: f64) -> i64 {
        self.sections
            .iter()
            .rposition(|(start, _)| (*start as f64) <= beat + 1e-6)
            .map_or(if self.sections.is_empty() { -1 } else { 0 }, |i| i as i64)
    }

    #[func]
    pub fn get_music_player(&self) -> Option<Gd<AudioStreamPlayer>> {
        self.player.clone()
    }
}

impl Conductor {
    pub fn timing(&self) -> Timing {
        self.timing
    }

    pub fn configure_from_analysis(
        &mut self,
        analysis: &SongAnalysis,
        stream: Option<Gd<AudioStream>>,
    ) {
        self.setup(
            stream,
            analysis.bpm,
            analysis.beat_offset_seconds,
            analysis.duration_seconds,
        );
        self.sections = analysis
            .sections
            .iter()
            .map(|section| {
                (
                    section.start_beat,
                    section.section_type.as_str().to_string(),
                )
            })
            .collect();
    }

    fn ensure_player(&mut self) -> Gd<AudioStreamPlayer> {
        if let Some(player) = &self.player {
            return player.clone();
        }
        let mut player = self
            .base()
            .try_get_node_as::<AudioStreamPlayer>("Music")
            .unwrap_or_else(|| {
                let mut player = AudioStreamPlayer::new_alloc();
                player.set_name("Music");
                player
            });
        if AudioServer::singleton().get_bus_index("Music") >= 0 {
            player.set_bus("Music");
        }
        if player.get_parent().is_none() {
            self.base_mut().add_child(&player);
        }
        self.player = Some(player.clone());
        player
    }

    fn audio_enabled(&self) -> bool {
        !self.use_clock
            && !self.audio_failed
            && !self.dummy_driver
            && self
                .player
                .as_ref()
                .is_some_and(|player| player.get_stream().is_some())
    }

    fn jump_clock(&mut self, seconds: f64) {
        self.clock_time = seconds;
        self.last_seek_time = seconds;
        self.seek_count += 1;
        // Re-signal a beat that starts exactly at `seconds`.
        self.last_beat = (self.timing.seconds_to_beat(seconds) - 1e-6).floor() as i64;
    }

    fn mark_audio_failed(&mut self, reason: &str) {
        if !self.audio_failed {
            godot_print!("Conductor: {reason}; using the internal clock");
        }
        self.audio_failed = true;
        if let Some(player) = self.player.as_mut() {
            player.stop();
        }
    }

    fn sync_to_audio(&mut self, delta: f64) {
        if !self.audio_enabled() {
            return;
        }
        let Some(player) = self.player.clone() else {
            return;
        };
        if !player.is_playing() {
            return;
        }
        let position = player.get_playback_position() as f64;
        if (position - self.last_audio_position).abs() < 1e-9 {
            self.stalled_for += delta;
            if self.stalled_for > STALL_LIMIT {
                self.mark_audio_failed("audio playback is not advancing");
            }
            return;
        }
        self.stalled_for = 0.0;
        self.last_audio_position = position;
        let server = AudioServer::singleton();
        let audio_time = audio_song_time(
            position,
            server.get_time_since_last_mix(),
            server.get_output_latency(),
            self.latency_offset,
        );
        let drift = audio_time - self.clock_time;
        if drift.abs() > SNAP_THRESHOLD {
            self.clock_time = audio_time;
        } else {
            self.clock_time += drift * DRIFT_CORRECTION;
        }
    }

    fn emit_crossed_beats(&mut self) {
        let generation = self.seek_count;
        let current = self.song_beat().floor() as i64;
        while self.last_beat < current {
            self.last_beat += 1;
            let beat = self.last_beat;
            if beat < 0 {
                continue;
            }
            self.signals().beat().emit(beat);
            let beats_per_bar = self.timing.beats_per_bar as i64;
            if beat % beats_per_bar == 0 {
                self.signals().bar().emit(beat / beats_per_bar);
            }
            if let Some(index) = self.sections.iter().position(|(start, _)| *start == beat) {
                let kind = GString::from(&self.sections[index].1);
                self.signals().section_started().emit(index as i64, &kind);
            }
            // A listener seeked or stopped: resume from the new position next frame.
            if self.seek_count != generation {
                return;
            }
        }
    }
}

#[godot_api]
impl INode for Conductor {
    fn ready(&mut self) {
        self.base_mut().add_to_group(crate::groups::CONDUCTOR);
        self.ensure_player();
        self.dummy_driver = AudioServer::singleton().get_driver_name() == "Dummy";
        if self.dummy_driver {
            godot_print!("Conductor: dummy audio driver; using the internal clock");
        }
    }

    fn exit_tree(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.stop();
            player.set_stream(Option::<&Gd<AudioStream>>::None);
        }
    }

    fn process(&mut self, delta: f64) {
        if !self.is_playing() {
            return;
        }
        self.clock_time += delta;
        self.sync_to_audio(delta);
        self.emit_crossed_beats();
        if self.clock_time >= self.duration_seconds && self.is_playing() {
            self.clock_time = self.duration_seconds;
            self.finished = true;
            if let Some(player) = self.player.as_mut() {
                player.stop();
            }
            self.signals().song_finished().emit();
        }
    }
}
