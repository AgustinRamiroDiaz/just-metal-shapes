//! `TitleScreen`: beat-pulsing logo and the main menu (Play, Settings, Credits, Quit).

use super::backdrop::add_backdrop;
use super::widgets::{link_vertical, nav_hints};
use super::{FontKind, button, go_to, label, palette, slide_in};
use godot::classes::control::{LayoutPreset, MouseFilter};
use godot::classes::{Button, Control, HBoxContainer, IControl, Label, Os, VBoxContainer};
use godot::prelude::*;

const TITLE_LINES: [&str; 3] = ["JUST", "METAL", "SHAPES"];

#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct TitleScreen {
    letters: Vec<Gd<Label>>,
    buttons: Vec<Gd<Button>>,
    base: Base<Control>,
}

#[godot_api]
impl TitleScreen {
    #[func]
    fn _on_play(&mut self) {
        go_to(super::LEVEL_SELECT_SCENE);
    }

    #[func]
    fn _on_settings(&mut self) {
        go_to(super::SETTINGS_SCENE);
    }

    #[func]
    fn _on_credits(&mut self) {
        go_to(super::CREDITS_SCENE);
    }

    #[func]
    fn _on_quit(&mut self) {
        self.base().get_tree().quit();
    }

    /// Names of the menu buttons in focus order (tests).
    #[func]
    pub fn get_button_names(&self) -> PackedStringArray {
        self.buttons.iter().map(|b| b.get_text()).collect()
    }
}

#[godot_api]
impl IControl for TitleScreen {
    fn ready(&mut self) {
        let mut root = self.to_gd().upcast::<Control>();
        super::full_rect(&mut root);
        add_backdrop(&mut root);
        if let Some(mut ui) = super::services() {
            ui.bind_mut().set_accent(palette::ACCENT);
            ui.bind_mut().play_menu_music();
        }

        let mut column = VBoxContainer::new_alloc();
        column.set_anchors_and_offsets_preset(LayoutPreset::LEFT_WIDE);
        column.set_offset(godot::builtin::Side::LEFT, 96.0);
        column.set_offset(godot::builtin::Side::RIGHT, 760.0);
        column.set_offset(godot::builtin::Side::TOP, 48.0);
        column.add_theme_constant_override("separation", 0);
        column.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&column);

        for (line_index, line) in TITLE_LINES.iter().enumerate() {
            let mut row = HBoxContainer::new_alloc();
            row.add_theme_constant_override("separation", 2);
            row.set_mouse_filter(MouseFilter::IGNORE);
            for ch in line.chars() {
                let letter = label(&ch.to_string(), FontKind::Display, 66, palette::TEXT);
                row.add_child(&letter);
                self.letters.push(letter);
            }
            column.add_child(&row);
            super::fade_in(&row.clone().upcast(), 0.05 + line_index as f64 * 0.08, 0.3);
        }

        let mut tagline = label(
            "Dodge to the beat. Together.",
            FontKind::Ui,
            22,
            palette::MIST,
        );
        tagline.set_custom_minimum_size(Vector2::new(0.0, 48.0));
        tagline.set_vertical_alignment(godot::global::VerticalAlignment::CENTER);
        column.add_child(&tagline);
        column.add_child(&super::spacer(18.0));

        let this = self.to_gd();
        let mut entries = vec![
            ("Play", "_on_play"),
            ("Settings", "_on_settings"),
            ("Credits", "_on_credits"),
        ];
        if !Os::singleton().has_feature("web") {
            entries.push(("Quit", "_on_quit"));
        }
        let mut list = VBoxContainer::new_alloc();
        list.add_theme_constant_override("separation", 10);
        list.set_h_size_flags(godot::classes::control::SizeFlags::SHRINK_BEGIN);
        column.add_child(&list);
        for (i, (text, method)) in entries.into_iter().enumerate() {
            let mut b = button(text);
            b.set_name(text);
            b.set_custom_minimum_size(Vector2::new(340.0, 52.0));
            b.set_text_alignment(godot::global::HorizontalAlignment::LEFT);
            b.connect("pressed", &this.callable(method));
            list.add_child(&b);
            super::fade_in(&b.clone().upcast(), 0.3 + i as f64 * 0.06, 0.25);
            self.buttons.push(b);
        }
        link_vertical(&self.buttons);
        slide_in(&column.clone().upcast(), Vector2::new(-60.0, 0.0), 0.0, 0.4);
        root.add_child(&nav_hints("Select", ""));
        if let Some(first) = self.buttons.first() {
            super::grab_focus_deferred(first);
        }
    }

    fn process(&mut self, _delta: f64) {
        let (beat, pulse) = super::services()
            .map(|ui| (ui.bind().music_beat(), ui.bind().beat_pulse()))
            .unwrap_or((0.0, 0.0));
        let count = self.letters.len().max(1) as f64;
        // A highlight runs across the logo once per bar; every beat kicks all letters.
        let head = (beat / 4.0).rem_euclid(1.0) * count;
        let accent = super::services()
            .map(|ui| ui.bind().get_accent())
            .unwrap_or(palette::ACCENT);
        for (i, letter) in self.letters.iter_mut().enumerate() {
            let distance = (i as f64 - head).abs().min(count - (i as f64 - head).abs());
            let wave = (1.0 - distance / 2.5).max(0.0);
            let size = letter.get_size();
            letter.set_pivot_offset(Vector2::new(size.x * 0.5, size.y));
            let scale = 1.0 + 0.10 * pulse as f32 + 0.12 * wave as f32 * pulse as f32;
            letter.set_scale(Vector2::new(1.0, scale));
            let color = palette::TEXT.lerp(accent, (wave * 0.9).min(1.0));
            letter.add_theme_color_override("font_color", color);
        }
    }
}
