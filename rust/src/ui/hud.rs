//! `Hud`: song progress (section ticks, checkpoint markers), team score, countdown, and
//! toasts for checkpoints, rewinds and tutorial hints. Lives, names and revives are
//! drawn on the players themselves (`PlayerVisual`). Driven by `GameManager`, `Conductor` and `LevelDirector` signals;
//! per-frame reads are limited to song time, score and revive progress.

use super::widgets::tag;
use super::{FontKind, label, palette, play_sfx};
use crate::conductor::Conductor;
use crate::core::analysis::SongAnalysis;
use crate::director::LevelDirector;
use crate::save::save_data;
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::tween::{EaseType, TransitionType};
use godot::classes::{
    CanvasLayer, Control, Engine, FileAccess, HBoxContainer, ICanvasLayer, IControl, Label, Node,
    Os, PanelContainer,
};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

const BAR_WIDTH: f32 = 560.0;
const TOAST_SECONDS: f64 = 1.8;
const GO_SECONDS: f64 = 0.7;
/// Captions sit this far (px) above their cue position.
const CAPTION_RISE: f32 = 58.0;
const CAPTION_COLOR: Color = Color::from_rgb(1.0, 0.82, 0.55);

#[derive(GodotClass)]
#[class(init, base = CanvasLayer)]
pub struct Hud {
    manager: Option<Gd<Node>>,
    conductor: Option<Gd<Conductor>>,
    director: Option<Gd<LevelDirector>>,
    progress: Option<Gd<SongProgress>>,
    time_label: Option<Gd<Label>>,
    length_label: Option<Gd<Label>>,
    score_label: Option<Gd<Label>>,
    score_caption: Option<Gd<Label>>,
    top_row: Option<Gd<HBoxContainer>>,
    shown_score: f64,
    title_label: Option<Gd<Label>>,
    count_label: Option<Gd<Label>>,
    count_hide_in: f64,
    toast: Option<Gd<PanelContainer>>,
    toast_left: f64,
    hint_label: Option<Gd<Label>>,
    hint_left: f64,
    fps_label: Option<Gd<Label>>,
    last_checkpoint_beat: f64,
    accent: Color,
    base: Base<CanvasLayer>,
}

#[godot_api]
impl Hud {
    /// Shows big centered text (countdowns); `0` hides it after a moment.
    #[func]
    pub fn show_count(&mut self, text: GString, hold_seconds: f64) {
        let Some(mut count) = self.count_label.clone() else {
            return;
        };
        count.set_text(&text);
        count.set_visible(true);
        count.set_modulate(Color::WHITE);
        let size = count.get_size();
        count.set_pivot_offset(size * 0.5);
        count.set_scale(Vector2::new(1.6, 1.6));
        if let Some(mut tween) = super::make_tween(&count.clone().upcast()) {
            tween
                .tween_property(&count, "scale", &Vector2::ONE.to_variant(), 0.25)
                .set_trans(TransitionType::BACK)
                .set_ease(EaseType::OUT);
        }
        self.count_hide_in = hold_seconds;
    }

    #[func]
    pub fn hide_count(&mut self) {
        if let Some(mut count) = self.count_label.clone() {
            count.set_visible(false);
        }
        self.count_hide_in = 0.0;
    }

    /// Text of the visible toast (empty if none; tests).
    #[func]
    pub fn get_toast_text(&self) -> GString {
        if self.toast_left <= 0.0 {
            return GString::new();
        }
        self.toast
            .as_ref()
            .and_then(|t| t.get_child(0))
            .and_then(|c| c.try_cast::<Label>().ok())
            .map(|l| l.get_text())
            .unwrap_or_default()
    }

    #[func]
    fn _on_countdown_tick(&mut self, remaining: i64) {
        if remaining > 0 {
            play_sfx("countdown_tick");
            self.show_count(remaining.to_string().as_str().into(), 10.0);
        } else {
            play_sfx("countdown_go");
            self.show_count("GO!".into(), GO_SECONDS);
        }
    }

    #[func]
    fn _on_checkpoint(&mut self, index: i64, beat: f64) {
        if index == 0 || beat <= self.last_checkpoint_beat + 1e-6 {
            self.last_checkpoint_beat = self.last_checkpoint_beat.max(beat);
            return;
        }
        self.last_checkpoint_beat = beat;
        play_sfx("checkpoint");
        let accent = self.accent;
        self.show_toast("Checkpoint", accent);
        if let Some(mut progress) = self.progress.clone() {
            progress.bind_mut().flash = 1.0;
        }
    }

    #[func]
    fn _on_rewound(&mut self, count: i64, _beat: f64) {
        let text = if count > 1 {
            format!("Rewind x{count}")
        } else {
            "Rewind".to_string()
        };
        self.show_toast(&text, palette::HOT);
    }

    #[func]
    fn _on_hint(&mut self, text: GString, duration: f64) {
        let Some(mut hint) = self.hint_label.clone() else {
            return;
        };
        hint.set_text(&text);
        hint.set_visible(true);
        super::fade_in(&hint.clone().upcast(), 0.0, 0.25);
        self.hint_left = duration.max(1.5);
    }

    /// A lyric word over its cue: pops in, holds, then fades and drifts up.
    #[func]
    fn _on_caption(&mut self, text: GString, position: Vector2, duration: f64) {
        let mut caption = label(&text.to_string(), FontKind::Display, 30, CAPTION_COLOR);
        caption.add_theme_constant_override("outline_size", 10);
        caption.add_theme_color_override("font_outline_color", palette::VOID);
        caption.set_horizontal_alignment(HorizontalAlignment::CENTER);
        self.base_mut().add_child(&caption);
        let size = caption.get_combined_minimum_size();
        let screen = self
            .base()
            .get_viewport()
            .map_or(Vector2::new(1280.0, 720.0), |v| v.get_visible_rect().size);
        let at = position - Vector2::new(size.x / 2.0, CAPTION_RISE + size.y / 2.0);
        let at = Vector2::new(
            at.x.clamp(8.0, (screen.x - size.x - 8.0).max(8.0)),
            at.y.clamp(8.0, (screen.y - size.y - 8.0).max(8.0)),
        );
        caption.set_position(at);
        caption.set_size(size);
        caption.set_pivot_offset(size / 2.0);
        caption.set_scale(Vector2::splat(0.4));
        let Some(mut tween) = super::make_tween(&caption.clone().upcast()) else {
            return;
        };
        tween
            .tween_property(&caption, "scale", &Vector2::ONE.to_variant(), 0.22)
            .set_trans(TransitionType::BACK)
            .set_ease(EaseType::OUT);
        tween.tween_interval((duration - 0.6).max(0.3));
        tween.tween_property(&caption, "modulate:a", &0.0.to_variant(), 0.4);
        tween
            .parallel()
            .tween_property(&caption, "position:y", &(at.y - 18.0).to_variant(), 0.4);
        tween.tween_callback(&caption.callable("queue_free"));
    }

    #[func]
    fn _on_gym_song_changed(&mut self) {
        self.setup_level_info();
    }

    #[func]
    fn _on_level_ended(&mut self, _score: i64) {
        self.hide_count();
        if let Some(mut hint) = self.hint_label.clone() {
            hint.set_visible(false);
        }
    }

    #[func]
    fn _on_settings_changed(&mut self) {
        let show = save_data(&self.to_gd().upcast()).is_some_and(|s| s.bind().show_fps);
        if let Some(mut fps) = self.fps_label.clone() {
            fps.set_visible(show);
        }
        if let Some(mut debug) = self
            .manager
            .as_ref()
            .and_then(|m| m.try_get_node_as::<Control>("DebugLabel"))
        {
            debug.set_visible(show && Os::singleton().is_debug_build());
        }
    }

    /// The level is loaded only after the HUD's `ready`.
    #[func]
    fn _setup_level_info(&mut self) {
        self.setup_level_info();
    }
}

impl Hud {
    fn show_toast(&mut self, text: &str, color: Color) {
        let Some(mut toast) = self.toast.clone() else {
            return;
        };
        super::widgets::retag(&mut toast, text, color, palette::VOID);
        toast.set_visible(true);
        let size = toast.get_size();
        toast.set_pivot_offset(size * 0.5);
        toast.set_scale(Vector2::new(0.7, 0.7));
        toast.set_modulate(Color::WHITE);
        if let Some(mut tween) = super::make_tween(&toast.clone().upcast()) {
            tween
                .tween_property(&toast, "scale", &Vector2::ONE.to_variant(), 0.3)
                .set_trans(TransitionType::BACK)
                .set_ease(EaseType::OUT);
        }
        self.toast_left = TOAST_SECONDS;
    }

    fn setup_level_info(&mut self) {
        let Some(director) = self.director.clone() else {
            return;
        };
        let info = director.bind().get_level_info();
        let title = info.get("title").map(|v| v.to_string()).unwrap_or_default();
        let accent = info
            .get("accent_color")
            .and_then(|v| v.try_to::<Color>().ok())
            .unwrap_or(palette::ACCENT);
        self.accent = accent;
        if let Some(mut ui) = super::services() {
            ui.bind_mut().set_accent(accent);
        }
        let mode = super::mode_name(director.bind().get_mode());
        if let Some(mut title_label) = self.title_label.clone() {
            if super::gym_mode() {
                title_label.set_text(&format!("Gym  /  {title}"));
            } else {
                title_label.set_text(&format!("{title}  /  {mode}"));
            }
        }
        let Some(conductor) = self.conductor.clone() else {
            return;
        };
        let duration = conductor.bind().get_duration();
        if let Some(mut length) = self.length_label.clone() {
            length.set_text(&super::format_time(duration));
        }
        let analysis_path = info
            .get("analysis_path")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let json = FileAccess::get_file_as_string(&analysis_path).to_string();
        let sections: Vec<f64> = SongAnalysis::from_json(&json)
            .map(|a| {
                a.sections
                    .iter()
                    .skip(1)
                    .map(|s| conductor.bind().beat_to_time(s.start_beat as f64))
                    .collect()
            })
            .unwrap_or_default();
        let checkpoints: Vec<f64> = director
            .bind()
            .get_checkpoint_beats()
            .as_slice()
            .iter()
            .map(|b| conductor.bind().beat_to_time(*b))
            .collect();
        if let Some(mut progress) = self.progress.clone() {
            let mut p = progress.bind_mut();
            p.duration = duration.max(0.001);
            p.sections = sections;
            p.checkpoints = checkpoints;
            p.accent = accent;
        }
    }

    fn build(&mut self) {
        let mut root = Control::new_alloc();
        root.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);
        self.base_mut().add_child(&root);

        // Top: title + progress on the left two thirds, score on the right.
        let mut top = HBoxContainer::new_alloc();
        top.set_anchors_and_offsets_preset(LayoutPreset::TOP_WIDE);
        top.set_offset(godot::builtin::Side::LEFT, 28.0);
        top.set_offset(godot::builtin::Side::RIGHT, -28.0);
        top.set_offset(godot::builtin::Side::TOP, 16.0);
        top.add_theme_constant_override("separation", 14);
        top.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&top);

        let mut progress_column = godot::classes::VBoxContainer::new_alloc();
        progress_column.add_theme_constant_override("separation", 2);
        progress_column.set_mouse_filter(MouseFilter::IGNORE);
        let title = label("", FontKind::Narrow, 18, palette::MIST);
        progress_column.add_child(&title);
        let mut bar_row = HBoxContainer::new_alloc();
        bar_row.add_theme_constant_override("separation", 10);
        bar_row.set_mouse_filter(MouseFilter::IGNORE);
        let time_label = label("0:00", FontKind::Narrow, 18, palette::TEXT);
        bar_row.add_child(&time_label);
        let mut progress = SongProgress::new_alloc();
        progress.set_custom_minimum_size(Vector2::new(BAR_WIDTH, 22.0));
        progress.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        bar_row.add_child(&progress);
        let length_label = label("0:00", FontKind::Narrow, 18, palette::MIST);
        bar_row.add_child(&length_label);
        progress_column.add_child(&bar_row);
        top.add_child(&progress_column);

        let mut filler = Control::new_alloc();
        filler.set_h_size_flags(SizeFlags::EXPAND_FILL);
        filler.set_mouse_filter(MouseFilter::IGNORE);
        top.add_child(&filler);

        let mut score_column = godot::classes::VBoxContainer::new_alloc();
        score_column.add_theme_constant_override("separation", -4);
        score_column.set_mouse_filter(MouseFilter::IGNORE);
        let mut score_caption = label("Team score", FontKind::Narrow, 18, palette::MIST);
        score_caption.set_horizontal_alignment(HorizontalAlignment::RIGHT);
        score_column.add_child(&score_caption);
        let mut score = label("0000000", FontKind::Mono, 40, palette::TEXT);
        score.set_horizontal_alignment(HorizontalAlignment::RIGHT);
        score.set_name("Score");
        score_column.add_child(&score);
        let mut fps = label("", FontKind::Mono, 16, palette::MIST);
        fps.set_horizontal_alignment(HorizontalAlignment::RIGHT);
        fps.set_visible(false);
        score_column.add_child(&fps);
        top.add_child(&score_column);

        // Toast under the progress bar.
        let mut toast = tag("", palette::ACCENT, palette::VOID, 26);
        toast.set_anchors_and_offsets_preset(LayoutPreset::CENTER_TOP);
        toast.set_h_grow_direction(godot::classes::control::GrowDirection::BOTH);
        toast.set_offset(godot::builtin::Side::TOP, 96.0);
        toast.set_visible(false);
        root.add_child(&toast);

        // Countdown in the middle.
        let mut count = label("", FontKind::Display, 170, palette::TEXT);
        count.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        count.set_horizontal_alignment(HorizontalAlignment::CENTER);
        count.set_vertical_alignment(godot::global::VerticalAlignment::CENTER);
        count.add_theme_constant_override("outline_size", 18);
        count.add_theme_color_override("font_outline_color", palette::VOID);
        count.set_visible(false);
        root.add_child(&count);

        // Tutorial hint near the bottom edge.
        let mut hint = label("", FontKind::Ui, 26, palette::TEXT);
        hint.set_anchors_and_offsets_preset(LayoutPreset::BOTTOM_WIDE);
        hint.set_offset(godot::builtin::Side::TOP, -84.0);
        hint.set_offset(godot::builtin::Side::BOTTOM, -38.0);
        hint.set_horizontal_alignment(HorizontalAlignment::CENTER);
        hint.add_theme_constant_override("outline_size", 10);
        hint.add_theme_color_override("font_outline_color", palette::VOID);
        hint.set_visible(false);
        root.add_child(&hint);

        self.title_label = Some(title);
        self.time_label = Some(time_label);
        self.length_label = Some(length_label);
        self.progress = Some(progress);
        self.score_label = Some(score);
        self.score_caption = Some(score_caption);
        self.top_row = Some(top);
        self.fps_label = Some(fps);
        self.toast = Some(toast);
        self.count_label = Some(count);
        self.hint_label = Some(hint);
    }
}

#[godot_api]
impl ICanvasLayer for Hud {
    fn ready(&mut self) {
        self.base_mut().set_layer(10);
        self.accent = palette::ACCENT;
        self.build();
        if super::gym_mode() {
            // The gym has no score, and its sidebar covers the right edge.
            for mut label in [self.score_label.clone(), self.score_caption.clone()]
                .into_iter()
                .flatten()
            {
                label.set_visible(false);
            }
            if let Some(mut top) = self.top_row.clone() {
                top.set_offset(
                    godot::builtin::Side::RIGHT,
                    -28.0 - super::gym::SIDEBAR_WIDTH,
                );
            }
        }
        let this = self.to_gd();
        let level_ui = self.base().get_parent();
        let manager = level_ui.and_then(|p| p.get_parent());
        if let Some(mut manager) = manager.clone() {
            manager.connect("countdown_tick", &this.callable("_on_countdown_tick"));
            manager.connect("rewound", &this.callable("_on_rewound"));
            manager.connect("level_cleared", &this.callable("_on_level_ended"));
            manager.connect("game_over", &this.callable("_on_level_ended"));
            manager.connect("gym_song_changed", &this.callable("_on_gym_song_changed"));
            self.conductor = manager.try_get_node_as::<Conductor>("Conductor");
            self.director = manager.try_get_node_as::<LevelDirector>("LevelDirector");
        }
        self.manager = manager;
        if let Some(mut director) = self.director.clone() {
            director.connect("checkpoint_reached", &this.callable("_on_checkpoint"));
            director.connect("show_hint", &this.callable("_on_hint"));
            director.connect("caption", &this.callable("_on_caption"));
        }
        if let Some(mut save) = save_data(&this.clone().upcast()) {
            save.connect("settings_changed", &this.callable("_on_settings_changed"));
        }
        self.base_mut().call_deferred("_setup_level_info", &[]);
        self.base_mut().call_deferred("_on_settings_changed", &[]);
    }

    fn process(&mut self, delta: f64) {
        let real_delta = delta / Engine::singleton().get_time_scale().max(0.001);
        if let Some(conductor) = self.conductor.clone() {
            let (time, progress) = {
                let c = conductor.bind();
                (c.song_time(), c.progress())
            };
            if let Some(mut label) = self.time_label.clone() {
                label.set_text(&super::format_time(time));
            }
            if let Some(mut bar) = self.progress.clone() {
                let mut b = bar.bind_mut();
                b.value = progress;
                b.time = time;
                if let Some(director) = &self.director {
                    let beat = director.bind().current_checkpoint_beat();
                    b.reached = conductor.bind().beat_to_time(beat);
                }
            }
        }
        if let Some(manager) = self.manager.clone() {
            let score = manager
                .clone()
                .call("get_score", &[])
                .try_to::<i64>()
                .unwrap_or(0);
            let target = score as f64;
            // Roll toward the real score so jumps read as a count-up.
            self.shown_score += (target - self.shown_score) * (real_delta * 8.0).min(1.0);
            if (target - self.shown_score).abs() < 1.0 {
                self.shown_score = target;
            }
            if let Some(mut label) = self.score_label.clone() {
                label.set_text(&format!("{:07}", self.shown_score.round() as i64));
            }
        }
        if let Some(mut fps) = self.fps_label.clone()
            && fps.is_visible()
        {
            fps.set_text(&format!(
                "{} fps",
                Engine::singleton().get_frames_per_second()
            ));
        }
        if self.count_hide_in > 0.0 {
            self.count_hide_in -= real_delta;
            if self.count_hide_in <= 0.0
                && let Some(mut count) = self.count_label.clone()
                && let Some(mut tween) = super::make_tween(&count.clone().upcast())
            {
                tween.tween_property(&count, "modulate:a", &0.0.to_variant(), 0.2);
                let size = count.get_size();
                count.set_pivot_offset(size * 0.5);
            }
        }
        if self.toast_left > 0.0 {
            self.toast_left -= real_delta;
            if self.toast_left <= 0.0
                && let Some(toast) = self.toast.clone()
                && let Some(mut tween) = super::make_tween(&toast.clone().upcast())
            {
                tween.tween_property(&toast, "modulate:a", &0.0.to_variant(), 0.3);
            }
        }
        if self.hint_left > 0.0 {
            self.hint_left -= delta;
            if self.hint_left <= 0.0
                && let Some(hint) = self.hint_label.clone()
                && let Some(mut tween) = super::make_tween(&hint.clone().upcast())
            {
                tween.tween_property(&hint, "modulate:a", &0.0.to_variant(), 0.4);
            }
        }
    }
}

/// Song progress bar: fill, section ticks, checkpoint diamonds (filled once reached).
#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct SongProgress {
    pub value: f64,
    pub time: f64,
    pub duration: f64,
    pub sections: Vec<f64>,
    pub checkpoints: Vec<f64>,
    /// Song time of the latest reached checkpoint.
    pub reached: f64,
    pub flash: f64,
    pub accent: Color,
    base: Base<Control>,
}

#[godot_api]
impl IControl for SongProgress {
    fn ready(&mut self) {
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
        if self.accent == Color::from_rgba(0.0, 0.0, 0.0, 0.0) {
            self.accent = palette::ACCENT;
        }
    }

    fn process(&mut self, delta: f64) {
        if self.flash > 0.0 {
            self.flash = (self.flash - delta * 2.0).max(0.0);
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let size = self.base().get_size();
        let bar_h = 6.0;
        let y = size.y * 0.5 - bar_h * 0.5;
        let width = size.x;
        let accent = self.accent.lerp(Color::WHITE, self.flash * 0.6);
        self.base_mut().draw_rect(
            Rect2::new(Vector2::new(0.0, y), Vector2::new(width, bar_h)),
            palette::STEEL,
        );
        let fill = (self.value.clamp(0.0, 1.0) as f32) * width;
        self.base_mut().draw_rect(
            Rect2::new(Vector2::new(0.0, y), Vector2::new(fill, bar_h)),
            accent,
        );
        let duration = self.duration.max(0.001);
        for t in self.sections.clone() {
            let x = (t / duration) as f32 * width;
            self.base_mut().draw_rect(
                Rect2::new(
                    Vector2::new(x - 1.0, y - 4.0),
                    Vector2::new(2.0, bar_h + 8.0),
                ),
                palette::MIST.with_alpha(0.7),
            );
        }
        for t in self.checkpoints.clone() {
            if t <= 0.01 {
                continue;
            }
            let x = (t / duration) as f32 * width;
            let c = Vector2::new(x, y - 7.0);
            let r = 5.0;
            let points = PackedVector2Array::from(&[
                c + Vector2::new(0.0, -r),
                c + Vector2::new(r, 0.0),
                c + Vector2::new(0.0, r),
                c + Vector2::new(-r, 0.0),
            ]);
            let reached = t <= self.reached + 0.01;
            let color = if reached { accent } else { palette::VOID };
            self.base_mut().draw_colored_polygon(&points, color);
            let mut outline = points.clone();
            outline.push(c + Vector2::new(0.0, -r));
            self.base_mut()
                .draw_polyline_ex(&outline, if reached { accent } else { palette::MIST })
                .width(1.5)
                .done();
        }
        // Playhead.
        self.base_mut().draw_rect(
            Rect2::new(
                Vector2::new(fill - 2.0, y - 5.0),
                Vector2::new(4.0, bar_h + 10.0),
            ),
            palette::TEXT,
        );
    }
}
