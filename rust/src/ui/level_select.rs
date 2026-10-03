//! `LevelSelect`: one card per catalog level (title, artist, difficulty, BPM, best
//! rank/score, lock state) and the difficulty mode row. Focusing a card previews its
//! song and switches the accent to the level palette.

use super::backdrop::add_backdrop;
use super::widgets::{DifficultyPips, nav_hints, tag};
use super::{FontKind, go_to, label, palette, play_sfx};
use crate::core::analysis::SongAnalysis;
use crate::core::chart_gen::LevelSpec;
use crate::game_config::GameConfig;
use crate::level_catalog::{all_levels, rgb_to_color};
use crate::save::save_data;
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{
    Button, Control, FileAccess, HBoxContainer, IButton, IControl, InputEvent, Label,
    MarginContainer, PanelContainer, StyleBoxFlat, VBoxContainer,
};
use godot::prelude::*;

const CARD_SIZE: Vector2 = Vector2::new(330.0, 360.0);
const MODES: [(i32, &str); 3] = [
    (GameConfig::CASUAL, "Fewer hazards. Half score."),
    (
        GameConfig::NORMAL,
        "Rewind to the last checkpoint when everyone is down.",
    ),
    (
        GameConfig::HARDCORE,
        "Denser charts, no rewinds, 1.5x score.",
    ),
];

struct Card {
    button: Gd<Button>,
    spec: LevelSpec,
    unlocked: bool,
    rank: Gd<Label>,
    score: Gd<Label>,
}

#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct LevelSelect {
    cards: Vec<Card>,
    mode_row: Option<Gd<ModeRow>>,
    mode_hint: Option<Gd<Label>>,
    focused_card: usize,
    mode: i32,
    base: Base<Control>,
}

#[godot_api]
impl LevelSelect {
    #[func]
    fn _on_card_focused(&mut self, index: i64) {
        let index = index as usize;
        self.focused_card = index;
        let Some(card) = self.cards.get(index) else {
            return;
        };
        let accent = rgb_to_color(card.spec.palette.accent);
        let id = card.spec.id.clone();
        if let Some(mut ui) = super::services() {
            ui.bind_mut().set_accent(accent);
            ui.bind_mut().preview_level(id.as_str().into());
        }
        if let Some(mut row) = self.mode_row.clone() {
            row.set_focus_neighbor(
                godot::builtin::Side::TOP,
                &self.cards[index].button.get_path(),
            );
        }
    }

    #[func]
    fn _on_card_pressed(&mut self, index: i64) {
        let Some(card) = self.cards.get(index as usize) else {
            return;
        };
        if !card.unlocked {
            play_sfx("ui_error");
            shake(card.button.clone().upcast());
            return;
        }
        let id = card.spec.id.clone();
        if let Some(mut config) = self.game_config() {
            config.bind_mut().selected_level_id = id.as_str().into();
            config.bind_mut().difficulty_mode = self.mode();
        }
        go_to(super::LOBBY_SCENE);
    }

    #[func]
    fn _on_mode_changed(&mut self, mode: i32) {
        self.mode = mode;
        if let Some(mut config) = self.game_config() {
            config.bind_mut().difficulty_mode = mode;
        }
        self.refresh_records();
    }

    /// Selects a level and mode as if confirmed from the cards (tests).
    #[func]
    pub fn get_level_ids(&self) -> PackedStringArray {
        self.cards
            .iter()
            .map(|c| GString::from(&c.spec.id))
            .collect()
    }

    #[func]
    pub fn is_card_unlocked(&self, index: i64) -> bool {
        self.cards.get(index as usize).is_some_and(|c| c.unlocked)
    }
}

impl LevelSelect {
    fn game_config(&self) -> Option<Gd<GameConfig>> {
        self.base()
            .get_node_or_null("/root/GameConfig")
            .and_then(|n| n.try_cast::<GameConfig>().ok())
    }

    fn mode(&self) -> i32 {
        self.mode
    }

    fn refresh_records(&mut self) {
        let mode = self.mode();
        let save = save_data(&self.to_gd().upcast());
        if let Some(mut hint) = self.mode_hint.clone() {
            let text = MODES
                .iter()
                .find(|(m, _)| *m == mode)
                .map_or("", |(_, text)| text);
            hint.set_text(text);
        }
        for card in self.cards.iter_mut().filter(|c| c.unlocked) {
            let record = save
                .as_ref()
                .map(|s| s.bind().get_record(card.spec.id.as_str().into(), mode));
            let rank = record
                .as_ref()
                .and_then(|r| r.get("best_rank"))
                .map(|v| v.to_string())
                .unwrap_or_default();
            let score = record
                .as_ref()
                .and_then(|r| r.get("best_score"))
                .map_or(0, |v| v.to::<i64>());
            if rank.is_empty() {
                card.rank.set_text("-");
                card.rank
                    .add_theme_color_override("font_color", palette::STEEL);
                card.score.set_text("No record");
            } else {
                card.rank.set_text(&rank);
                card.rank
                    .add_theme_color_override("font_color", super::rank_color(&rank));
                card.score.set_text(&format!("{score:07}"));
            }
        }
    }

    fn build_card(&mut self, index: usize, spec: LevelSpec, unlocked: bool, previous: &str) {
        let accent = rgb_to_color(spec.palette.accent);
        let mut button = Button::new_alloc();
        button.set_name(&format!("Card_{}", spec.id));
        button.set_custom_minimum_size(CARD_SIZE);
        super::wire_button_feedback(button.clone().upcast());

        // Accent edge along the top of the (skewed) card.
        if let Some(Ok(normal)) = button
            .get_theme_stylebox("normal")
            .map(|s| s.try_cast::<StyleBoxFlat>())
        {
            let mut style = normal.duplicate_resource();
            style.set_border_width(godot::builtin::Side::TOP, 8);
            style.set_border_color(if unlocked { accent } else { palette::STEEL });
            button.add_theme_stylebox_override("normal", &style);
        }

        let mut margin = MarginContainer::new_alloc();
        margin.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        for side in ["margin_left", "margin_right"] {
            margin.add_theme_constant_override(side, 34);
        }
        margin.add_theme_constant_override("margin_top", 28);
        margin.add_theme_constant_override("margin_bottom", 24);
        margin.set_mouse_filter(MouseFilter::IGNORE);
        button.add_child(&margin);

        let mut column = VBoxContainer::new_alloc();
        column.add_theme_constant_override("separation", 6);
        column.set_mouse_filter(MouseFilter::IGNORE);
        margin.add_child(&column);

        // Levels are played in order, so the number is the track number.
        column.add_child(&label(
            &format!("Track {:02}", index + 1),
            FontKind::Mono,
            20,
            if unlocked { accent } else { palette::MIST },
        ));
        let mut title = label(&spec.title, FontKind::Display, 28, palette::TEXT);
        title.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD);
        title.set_custom_minimum_size(Vector2::new(CARD_SIZE.x - 80.0, 0.0));
        column.add_child(&title);
        column.add_child(&label(&spec.artist, FontKind::Ui, 18, palette::MIST));
        column.add_child(&super::spacer(8.0));

        let mut stats = HBoxContainer::new_alloc();
        stats.add_theme_constant_override("separation", 18);
        stats.set_mouse_filter(MouseFilter::IGNORE);
        let mut pips = DifficultyPips::new_alloc();
        pips.bind_mut().level = spec.difficulty as i64;
        pips.bind_mut().color = if unlocked { accent } else { palette::MIST };
        pips.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        stats.add_child(&pips);
        let (bpm, duration) = song_facts(&spec);
        stats.add_child(&label(
            &format!("{bpm:.0} BPM"),
            FontKind::Mono,
            20,
            palette::TEXT,
        ));
        stats.add_child(&label(
            &super::format_time(duration),
            FontKind::Narrow,
            20,
            palette::MIST,
        ));
        column.add_child(&stats);

        let mut filler = Control::new_alloc();
        filler.set_v_size_flags(SizeFlags::EXPAND_FILL);
        filler.set_mouse_filter(MouseFilter::IGNORE);
        column.add_child(&filler);

        let mut best = HBoxContainer::new_alloc();
        best.add_theme_constant_override("separation", 16);
        best.set_mouse_filter(MouseFilter::IGNORE);
        let rank = label("-", FontKind::Display, 64, palette::STEEL);
        best.add_child(&rank);
        let mut best_text = VBoxContainer::new_alloc();
        best_text.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        best_text.set_mouse_filter(MouseFilter::IGNORE);
        best_text.add_child(&label("Best", FontKind::Narrow, 18, palette::MIST));
        let score = label("No record", FontKind::Mono, 22, palette::TEXT);
        best_text.add_child(&score);
        best.add_child(&best_text);

        if unlocked {
            column.add_child(&best);
        } else {
            margin.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.5));
            // The lock note replaces the record block and stays fully opaque.
            best.queue_free();
            let mut lock = VBoxContainer::new_alloc();
            lock.set_anchors_and_offsets_preset(LayoutPreset::BOTTOM_WIDE);
            lock.set_offset(godot::builtin::Side::TOP, -96.0);
            lock.set_offset(godot::builtin::Side::LEFT, 34.0);
            lock.set_offset(godot::builtin::Side::BOTTOM, -24.0);
            lock.add_theme_constant_override("separation", 8);
            lock.set_mouse_filter(MouseFilter::IGNORE);
            let mut locked_tag: Gd<PanelContainer> = tag("Locked", palette::HOT, palette::VOID, 22);
            locked_tag.set_h_size_flags(SizeFlags::SHRINK_BEGIN);
            lock.add_child(&locked_tag);
            lock.add_child(&label(
                &format!("Clear {previous} to unlock"),
                FontKind::Narrow,
                18,
                palette::TEXT,
            ));
            button.add_child(&lock);
        }

        let this = self.to_gd();
        button.connect(
            "focus_entered",
            &this
                .callable("_on_card_focused")
                .bind(&[(index as i64).to_variant()]),
        );
        button.connect(
            "pressed",
            &this
                .callable("_on_card_pressed")
                .bind(&[(index as i64).to_variant()]),
        );
        self.cards.push(Card {
            button,
            spec,
            unlocked,
            rank,
            score,
        });
    }
}

#[godot_api]
impl IControl for LevelSelect {
    fn ready(&mut self) {
        let mut root = self.to_gd().upcast::<Control>();
        super::full_rect(&mut root);
        add_backdrop(&mut root);

        let mut column = VBoxContainer::new_alloc();
        column.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        for (side, offset) in [
            (godot::builtin::Side::LEFT, 80.0),
            (godot::builtin::Side::RIGHT, -80.0),
            (godot::builtin::Side::TOP, 44.0),
            (godot::builtin::Side::BOTTOM, -80.0),
        ] {
            column.set_offset(side, offset);
        }
        column.add_theme_constant_override("separation", 22);
        column.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&column);
        column.add_child(&super::widgets::screen_header(
            "Pick a song",
            "Every level is one track. Clear it to open the next.",
        ));

        let save = save_data(&self.to_gd().upcast());
        let levels = all_levels();
        let mut cards_row = HBoxContainer::new_alloc();
        cards_row.add_theme_constant_override("separation", 28);
        cards_row.set_mouse_filter(MouseFilter::IGNORE);
        column.add_child(&cards_row);
        let mut previous_title = String::new();
        for (index, spec) in levels.into_iter().enumerate() {
            let unlocked = save
                .as_ref()
                .is_none_or(|s| s.bind().is_level_unlocked(spec.id.as_str().into()));
            let title = spec.title.clone();
            self.build_card(index, spec, unlocked, &previous_title);
            previous_title = title;
        }
        for (i, card) in self.cards.iter().enumerate() {
            cards_row.add_child(&card.button);
            super::fade_in(&card.button.clone().upcast(), 0.05 + i as f64 * 0.07, 0.3);
        }

        let mut mode_line = HBoxContainer::new_alloc();
        mode_line.add_theme_constant_override("separation", 24);
        mode_line.set_mouse_filter(MouseFilter::IGNORE);
        let config_mode = self
            .game_config()
            .map_or(GameConfig::NORMAL, |c| c.bind().difficulty_mode);
        let mut row = ModeRow::new_alloc();
        row.bind_mut().mode = config_mode;
        self.mode = config_mode;
        row.set_name("ModeRow");
        let this = self.to_gd();
        row.connect("mode_changed", &this.callable("_on_mode_changed"));
        mode_line.add_child(&row);
        let mut hint = label("", FontKind::Narrow, 20, palette::MIST);
        hint.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        mode_line.add_child(&hint);
        column.add_child(&mode_line);
        self.mode_row = Some(row.clone());
        self.mode_hint = Some(hint);

        // Up/down moves between the cards and the mode row.
        let row_path = row.get_path();
        let paths: Vec<NodePath> = self.cards.iter().map(|c| c.button.get_path()).collect();
        let n = paths.len();
        for (i, card) in self.cards.iter_mut().enumerate() {
            card.button
                .set_focus_neighbor(godot::builtin::Side::BOTTOM, &row_path);
            card.button
                .set_focus_neighbor(godot::builtin::Side::LEFT, &paths[(i + n - 1) % n]);
            card.button
                .set_focus_neighbor(godot::builtin::Side::RIGHT, &paths[(i + 1) % n]);
        }
        if let Some(first) = self.cards.first() {
            let first_path = first.button.get_path();
            row.set_focus_neighbor(godot::builtin::Side::TOP, &first_path);
        }

        root.add_child(&nav_hints("Choose", "Back"));
        self.refresh_records();

        let selected = self
            .game_config()
            .map(|c| c.bind().selected_level_id.to_string())
            .unwrap_or_default();
        let focus = self
            .cards
            .iter()
            .position(|c| c.spec.id == selected)
            .unwrap_or(0);
        if let Some(card) = self.cards.get(focus) {
            super::grab_focus_deferred(&card.button);
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if super::is_back(&event) && !super::transitioning() {
            play_sfx("ui_back");
            go_to(super::TITLE_SCENE);
            self.base_mut().accept_event();
        }
    }
}

/// Difficulty mode chooser: left/right (or click) cycles Casual/Normal/Hardcore.
#[derive(GodotClass)]
#[class(init, base = Button)]
pub struct ModeRow {
    #[var]
    pub mode: i32,
    base: Base<Button>,
}

#[godot_api]
impl ModeRow {
    #[signal]
    pub fn mode_changed(mode: i32);

    #[func]
    pub fn cycle(&mut self, direction: i32) {
        self.mode = (self.mode + direction).rem_euclid(3);
        self.refresh();
        play_sfx("ui_move");
        let mode = self.mode;
        self.signals().mode_changed().emit(mode);
    }

    fn refresh(&mut self) {
        let text = format!("Mode: {}", super::mode_name(self.mode));
        self.base_mut().set_text(&text);
    }
}

#[godot_api]
impl IButton for ModeRow {
    fn ready(&mut self) {
        self.base_mut()
            .set_custom_minimum_size(Vector2::new(360.0, 54.0));
        // Arrow glyphs at both ends show that left/right changes the mode.
        for (path, x) in [
            (
                "res://assets/kenney_ui-pack/grey/arrow_basic_w_small.png",
                24.0,
            ),
            (
                "res://assets/kenney_ui-pack/grey/arrow_basic_e_small.png",
                318.0,
            ),
        ] {
            let mut arrow = super::texture_rect(path, 18.0);
            arrow.set_position(Vector2::new(x, 18.0));
            arrow.set_size(Vector2::new(18.0, 18.0));
            self.base_mut().add_child(&arrow);
        }
        self.refresh();
    }

    fn gui_input(&mut self, event: Gd<InputEvent>) {
        let left = event
            .is_action_pressed_ex("ui_left")
            .allow_echo(true)
            .done();
        let right = event
            .is_action_pressed_ex("ui_right")
            .allow_echo(true)
            .done();
        if left || right {
            self.cycle(if right { 1 } else { -1 });
            self.base_mut().accept_event();
        }
    }

    fn pressed(&mut self) {
        self.cycle(1);
    }
}

/// BPM and length from the level's analysis file.
fn song_facts(spec: &LevelSpec) -> (f64, f64) {
    let json = FileAccess::get_file_as_string(&spec.analysis_path).to_string();
    SongAnalysis::from_json(&json)
        .map(|a| (a.bpm, a.duration_seconds))
        .unwrap_or((0.0, 0.0))
}

/// Horizontal shake for refused actions.
pub fn shake(control: Gd<Control>) {
    let Some(mut tween) = super::make_tween(&control.clone().upcast()) else {
        return;
    };
    let mut control = control;
    let size = control.get_size();
    control.set_pivot_offset(size * 0.5);
    for angle in [0.04_f32, -0.04, 0.025, -0.015, 0.0] {
        tween.tween_property(&control, "rotation", &angle.to_variant(), 0.05);
    }
}
