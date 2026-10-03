//! `ResultsScreen`: shown when a level is cleared (rank, score breakdown, new-best badge,
//! Retry / Next level / Level select) or lost in hardcore (game over). Records the run
//! in `SaveData`.

use super::widgets::tag;
use super::{FontKind, go_to, label, palette, play_sfx, rank_color};
use crate::game_config::GameConfig;
use crate::level_catalog::all_levels;
use crate::save::save_data;
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::node::ProcessMode;
use godot::classes::tween::{EaseType, TransitionType};
use godot::classes::{
    Button, CanvasLayer, ColorRect, Control, GridContainer, HBoxContainer, ICanvasLayer,
    InputEvent, Label, PanelContainer, StyleBoxFlat, VBoxContainer,
};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

const COUNT_UP_SECONDS: f64 = 1.1;

#[derive(GodotClass)]
#[class(init, base = CanvasLayer)]
pub struct ResultsScreen {
    /// `GameManager.get_run_stats()`.
    #[var]
    pub stats: VarDictionary,
    /// `LevelDirector.get_level_info()`.
    #[var]
    pub level: VarDictionary,
    #[var]
    pub cleared: bool,
    #[var]
    pub mode: i32,
    /// The level never started (shown as an error, not recorded).
    #[var]
    pub load_failed: bool,
    /// Set after recording: `SaveData.record_result` outcome.
    #[var]
    pub outcome: VarDictionary,
    score_label: Option<Gd<Label>>,
    score_target: i64,
    elapsed: f64,
    best_tag: Option<Gd<PanelContainer>>,
    best_shown: bool,
    buttons: Vec<Gd<Button>>,
    next_level_id: Option<String>,
    base: Base<CanvasLayer>,
}

#[godot_api]
impl ResultsScreen {
    #[func]
    fn _on_retry(&mut self) {
        go_to(super::LEVEL_SCENE);
    }

    #[func]
    fn _on_next(&mut self) {
        let Some(next) = self.next_level_id.clone() else {
            return;
        };
        if let Some(mut config) = self
            .base()
            .get_node_or_null("/root/GameConfig")
            .and_then(|n| n.try_cast::<GameConfig>().ok())
        {
            config.bind_mut().selected_level_id = next.as_str().into();
        }
        go_to(super::LEVEL_SCENE);
    }

    #[func]
    fn _on_level_select(&mut self) {
        go_to(super::LEVEL_SELECT_SCENE);
    }

    #[func]
    fn _center_rank_pivot(&mut self) {
        if let Some(mut letter) = self
            .base()
            .find_child("RankLetter")
            .and_then(|n| n.try_cast::<Control>().ok())
        {
            let size = letter.get_size();
            letter.set_pivot_offset(size * 0.5);
        }
    }

    /// Button labels in focus order (tests).
    #[func]
    pub fn get_button_names(&self) -> PackedStringArray {
        self.buttons.iter().map(|b| b.get_text()).collect()
    }
}

impl ResultsScreen {
    fn stat_i64(&self, key: &str) -> i64 {
        self.stats
            .get(key)
            .and_then(|v| v.try_to::<i64>().ok())
            .unwrap_or(0)
    }

    fn stat_f64(&self, key: &str) -> f64 {
        self.stats
            .get(key)
            .and_then(|v| v.try_to::<f64>().ok())
            .unwrap_or(0.0)
    }

    fn level_str(&self, key: &str) -> String {
        self.level
            .get(key)
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    fn record(&mut self) {
        let id = self.level_str("id");
        if self.load_failed {
            return;
        }
        let Some(mut save) = save_data(&self.to_gd().upcast()) else {
            return;
        };
        if id.is_empty() {
            return;
        }
        let rank = self
            .stats
            .get("rank")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "D".into());
        let score = self.stat_i64("score");
        let (mode, cleared) = (self.mode, self.cleared);
        self.outcome = save.bind_mut().record_result(
            id.as_str().into(),
            mode,
            score,
            rank.as_str().into(),
            cleared,
        );
        let levels = all_levels();
        if cleared
            && let Some(index) = levels.iter().position(|l| l.id == id)
            && let Some(next) = levels.get(index + 1)
            && save.bind().is_level_unlocked(next.id.as_str().into())
        {
            self.next_level_id = Some(next.id.clone());
        }
    }

    fn build(&mut self) {
        let accent = self
            .level
            .get("accent_color")
            .and_then(|v| v.try_to::<Color>().ok())
            .unwrap_or(palette::ACCENT);
        let rank = self
            .stats
            .get("rank")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "D".into());

        let mut root = Control::new_alloc();
        root.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);
        self.base_mut().add_child(&root);

        let mut dim = ColorRect::new_alloc();
        dim.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        dim.set_color(palette::VOID.with_alpha(0.86));
        root.add_child(&dim);
        super::fade_in(&dim.clone().upcast(), 0.0, 0.35);

        let mut layout = HBoxContainer::new_alloc();
        layout.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        layout.set_offset(godot::builtin::Side::LEFT, 90.0);
        layout.set_offset(godot::builtin::Side::RIGHT, -90.0);
        layout.set_offset(godot::builtin::Side::TOP, 60.0);
        layout.set_offset(godot::builtin::Side::BOTTOM, -60.0);
        layout.add_theme_constant_override("separation", 56);
        layout.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&layout);

        // Rank block.
        let mut rank_panel = PanelContainer::new_alloc();
        let mut style = StyleBoxFlat::new_gd();
        let rank_tint = if self.cleared {
            rank_color(&rank)
        } else {
            palette::HOT
        };
        style.set_bg_color(palette::PANEL);
        style.set_border_width(godot::builtin::Side::LEFT, 10);
        style.set_border_color(rank_tint);
        style.set_skew(Vector2::new(0.12, 0.0));
        rank_panel.add_theme_stylebox_override("panel", &style);
        rank_panel.set_custom_minimum_size(Vector2::new(330.0, 0.0));
        rank_panel.set_mouse_filter(MouseFilter::IGNORE);
        let mut rank_column = VBoxContainer::new_alloc();
        rank_column.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);
        rank_column.set_mouse_filter(MouseFilter::IGNORE);
        let caption = if self.cleared { "Rank" } else { "Run over" };
        rank_column.add_child(&super::centered(label(
            caption,
            FontKind::Ui,
            24,
            palette::MIST,
        )));
        let mut rank_letter = super::centered(label(
            if self.cleared { &rank } else { "X" },
            FontKind::Display,
            260,
            rank_tint,
        ));
        rank_letter.set_name("RankLetter");
        rank_column.add_child(&rank_letter);
        rank_panel.add_child(&rank_column);
        layout.add_child(&rank_panel);

        // Details.
        let mut details = VBoxContainer::new_alloc();
        details.set_h_size_flags(SizeFlags::EXPAND_FILL);
        details.add_theme_constant_override("separation", 8);
        details.set_mouse_filter(MouseFilter::IGNORE);
        let (headline, headline_color) = if self.load_failed {
            ("Level failed to load", palette::HOT)
        } else if self.cleared {
            ("Level clear", accent)
        } else {
            ("Game over", palette::HOT)
        };
        let headline_label = label(headline, FontKind::Display, 64, headline_color);
        details.add_child(&headline_label);
        details.add_child(&label(
            &format!(
                "{}  /  {}",
                self.level_str("title"),
                super::mode_name(self.mode)
            ),
            FontKind::Ui,
            22,
            palette::MIST,
        ));
        details.add_child(&super::spacer(10.0));

        let mut score_row = HBoxContainer::new_alloc();
        score_row.add_theme_constant_override("separation", 20);
        score_row.set_mouse_filter(MouseFilter::IGNORE);
        let score_label = label("0000000", FontKind::Mono, 64, palette::TEXT);
        score_row.add_child(&score_label);
        let mut best_tag = tag("New best", palette::HOT, palette::VOID, 22);
        best_tag.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        best_tag.set_visible(false);
        best_tag.set_name("NewBest");
        score_row.add_child(&best_tag);
        details.add_child(&score_row);

        let mut grid = GridContainer::new_alloc();
        grid.set_columns(2);
        grid.add_theme_constant_override("h_separation", 40);
        grid.add_theme_constant_override("v_separation", 4);
        grid.set_mouse_filter(MouseFilter::IGNORE);
        let time = format!(
            "{} / {}",
            super::format_time(self.stat_f64("seconds_survived")),
            super::format_time(self.stat_f64("song_duration"))
        );
        let rows: Vec<(&str, String)> = vec![
            ("Time", time),
            (
                "Enemies destroyed",
                self.stat_i64("enemies_killed").to_string(),
            ),
            ("Revives", self.stat_i64("revives").to_string()),
            ("Hits taken", self.stat_i64("hits_taken").to_string()),
            ("Rewinds", self.stat_i64("rewinds").to_string()),
        ];
        for (i, (name, value)) in rows.into_iter().enumerate() {
            let name_label = label(name, FontKind::Narrow, 22, palette::MIST);
            let font = if name == "Time" {
                FontKind::Narrow
            } else {
                FontKind::Mono
            };
            let mut value_label = label(&value, font, 24, palette::TEXT);
            value_label.set_horizontal_alignment(HorizontalAlignment::RIGHT);
            value_label.set_custom_minimum_size(Vector2::new(150.0, 0.0));
            super::fade_in(&name_label.clone().upcast(), 0.4 + i as f64 * 0.08, 0.2);
            super::fade_in(&value_label.clone().upcast(), 0.4 + i as f64 * 0.08, 0.2);
            grid.add_child(&name_label);
            grid.add_child(&value_label);
        }
        details.add_child(&grid);
        let mut filler = Control::new_alloc();
        filler.set_v_size_flags(SizeFlags::EXPAND_FILL);
        filler.set_mouse_filter(MouseFilter::IGNORE);
        details.add_child(&filler);

        let mut button_row = HBoxContainer::new_alloc();
        button_row.add_theme_constant_override("separation", 14);
        let this = self.to_gd();
        let mut entries = vec![("Retry", "_on_retry")];
        if self.next_level_id.is_some() {
            entries.insert(0, ("Next level", "_on_next"));
        }
        entries.push(("Level select", "_on_level_select"));
        for (text, method) in entries {
            let mut b = super::button(text);
            b.set_name(&text.replace(' ', ""));
            b.set_custom_minimum_size(Vector2::new(200.0, 56.0));
            b.connect("pressed", &this.callable(method));
            button_row.add_child(&b);
            self.buttons.push(b);
        }
        details.add_child(&button_row);
        super::fade_in(&button_row.clone().upcast(), 0.9, 0.25);
        layout.add_child(&details);

        // Left/right moves along the button row and wraps.
        let n = self.buttons.len();
        for i in 0..n {
            let prev = self.buttons[(i + n - 1) % n].get_path();
            let next = self.buttons[(i + 1) % n].get_path();
            self.buttons[i].set_focus_neighbor(godot::builtin::Side::LEFT, &prev);
            self.buttons[i].set_focus_neighbor(godot::builtin::Side::RIGHT, &next);
        }
        if let Some(first) = self.buttons.first() {
            super::grab_focus_deferred(first);
        }

        // Rank slam.
        if let Some(mut tween) = super::make_tween(&rank_letter.clone().upcast()) {
            rank_letter.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.0));
            rank_letter.set_scale(Vector2::new(2.6, 2.6));
            rank_letter.set_rotation(-0.25);
            tween.set_parallel();
            tween
                .tween_property(&rank_letter, "scale", &Vector2::ONE.to_variant(), 0.45)
                .set_delay(0.25)
                .set_trans(TransitionType::BACK)
                .set_ease(EaseType::OUT);
            tween
                .tween_property(&rank_letter, "rotation", &0.0.to_variant(), 0.45)
                .set_delay(0.25)
                .set_trans(TransitionType::BACK)
                .set_ease(EaseType::OUT);
            tween
                .tween_property(&rank_letter, "modulate:a", &1.0.to_variant(), 0.15)
                .set_delay(0.25);
        }
        self.score_label = Some(score_label);
        self.best_tag = Some(best_tag);
    }
}

#[godot_api]
impl ICanvasLayer for ResultsScreen {
    fn ready(&mut self) {
        self.base_mut().set_layer(30);
        self.base_mut().set_process_mode(ProcessMode::ALWAYS);
        self.score_target = self.stat_i64("score");
        self.record();
        self.build();
        play_sfx(if self.cleared {
            "level_clear"
        } else {
            "game_over"
        });
        // The letter's pivot needs its laid-out size.
        self.base_mut().call_deferred("_center_rank_pivot", &[]);
    }

    fn process(&mut self, delta: f64) {
        let real = delta
            / godot::classes::Engine::singleton()
                .get_time_scale()
                .max(0.001);
        self.elapsed += real;
        let t = ((self.elapsed - 0.4) / COUNT_UP_SECONDS).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        let shown = (self.score_target as f64 * eased).round() as i64;
        if let Some(mut score) = self.score_label.clone() {
            score.set_text(&format!("{shown:07}"));
        }
        if t >= 1.0 && !self.best_shown {
            self.best_shown = true;
            let new_best = self
                .outcome
                .get("new_best_score")
                .is_some_and(|v| v.try_to::<bool>().unwrap_or(false));
            if new_best && let Some(mut best) = self.best_tag.clone() {
                best.set_visible(true);
                let size = best.get_size();
                best.set_pivot_offset(size * 0.5);
                best.set_scale(Vector2::new(0.3, 0.3));
                if let Some(mut tween) = super::make_tween(&best.clone().upcast()) {
                    tween
                        .tween_property(&best, "scale", &Vector2::ONE.to_variant(), 0.35)
                        .set_trans(TransitionType::BACK)
                        .set_ease(EaseType::OUT);
                }
                play_sfx("ui_join");
            }
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if super::is_back(&event) && !super::transitioning() {
            play_sfx("ui_back");
            go_to(super::LEVEL_SELECT_SCENE);
            if let Some(mut viewport) = self.base().get_viewport() {
                viewport.set_input_as_handled();
            }
        }
    }
}
