//! Settings: `SettingsPanel` (rows bound to `SaveData` properties, used by the title's
//! Settings screen and the pause overlay) and `SettingsScreen` (the standalone scene).

use super::backdrop::add_backdrop;
use super::widgets::{OptionKind, OptionRow, nav_hints};
use super::{FontKind, go_to, label, palette, play_sfx};
use crate::core::save_model::LATENCY_LIMIT_MS;
use crate::save::{SaveData, save_data};
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{
    Button, Control, IControl, IVBoxContainer, InputEvent, Label, Time, VBoxContainer,
};
use godot::prelude::*;

/// Beats the calibration metronome plays; the first `CALIBRATION_WARMUP` are ignored.
const CALIBRATION_BEATS: usize = 12;
const CALIBRATION_WARMUP: usize = 4;
const CALIBRATION_BPM: f64 = 100.0;

#[derive(GodotClass)]
#[class(init, base = VBoxContainer)]
pub struct SettingsPanel {
    rows: Vec<(String, Gd<OptionRow>)>,
    back: Option<Gd<Button>>,
    calibration: Option<Calibration>,
    calibration_label: Option<Gd<Label>>,
    base: Base<VBoxContainer>,
}

struct Calibration {
    started_usec: u64,
    beats_played: usize,
    taps: Vec<f64>,
}

#[godot_api]
impl SettingsPanel {
    /// Back was chosen (button, Esc or B).
    #[signal]
    pub fn closed();

    #[func]
    fn _on_row_changed(&mut self, value: f64, property: GString) {
        let Some(mut save) = self.save() else {
            return;
        };
        let property = property.to_string();
        let variant = match property.as_str() {
            "latency_offset_ms" => (value.round() as i64).to_variant(),
            "screen_shake" | "reduce_flashing" | "fullscreen" | "show_fps" | "unlock_all" => {
                (value > 0.5).to_variant()
            }
            _ => value.to_variant(),
        };
        save.set(property.as_str(), &variant);
    }

    #[func]
    fn _on_back(&mut self) {
        self.close();
    }

    #[func]
    fn _on_calibrate(&mut self) {
        self.calibration = Some(Calibration {
            started_usec: Time::singleton().get_ticks_usec(),
            beats_played: 0,
            taps: Vec::new(),
        });
        self.set_calibration_text("Tap Enter / A on every click");
    }

    /// Focuses the first row (callers that show the panel).
    #[func]
    pub fn focus_first(&mut self) {
        if let Some((_, row)) = self.rows.first() {
            super::grab_focus_deferred(row);
        }
    }

    /// Row for a `SaveData` property (tests).
    #[func]
    pub fn get_row(&self, property: GString) -> Option<Gd<OptionRow>> {
        let property = property.to_string();
        self.rows
            .iter()
            .find(|(p, _)| *p == property)
            .map(|(_, row)| row.clone())
    }
}

impl SettingsPanel {
    fn save(&self) -> Option<Gd<SaveData>> {
        save_data(&self.to_gd().upcast())
    }

    fn close(&mut self) {
        if self.calibration.is_some() {
            self.calibration = None;
            self.set_calibration_text("");
            return;
        }
        if let Some(mut save) = self.save() {
            save.bind_mut().save();
        }
        play_sfx("ui_back");
        self.signals().closed().emit();
    }

    fn add_row(&mut self, title: &str, property: &str, kind: OptionKind) {
        let value = self
            .save()
            .map(|s| s.get(property))
            .map_or(0.0, |v| match v.get_type() {
                VariantType::BOOL => {
                    if v.to::<bool>() {
                        1.0
                    } else {
                        0.0
                    }
                }
                VariantType::INT => v.to::<i64>() as f64,
                _ => v.try_to::<f64>().unwrap_or(0.0),
            });
        let mut row = OptionRow::create(title, kind, value);
        row.set_name(property);
        let this = self.to_gd();
        row.connect(
            "value_changed",
            &this
                .callable("_on_row_changed")
                .bind(&[GString::from(property).to_variant()]),
        );
        self.base_mut().add_child(&row);
        self.rows.push((property.to_string(), row));
    }

    fn set_calibration_text(&mut self, text: &str) {
        if let Some(label) = self.calibration_label.as_mut() {
            label.set_text(text);
            label.set_visible(!text.is_empty());
        }
    }

    fn finish_calibration(&mut self, taps: &[f64]) {
        let mut offsets: Vec<f64> = taps.to_vec();
        if offsets.len() < 3 {
            self.set_calibration_text("Not enough taps. Try again.");
            play_sfx("ui_error");
            return;
        }
        offsets.sort_by(f64::total_cmp);
        let median = offsets[offsets.len() / 2];
        let ms = (median * 1000.0).round() as i64;
        let ms = ms.clamp(-(LATENCY_LIMIT_MS as i64), LATENCY_LIMIT_MS as i64);
        if let Some(mut save) = self.save() {
            save.bind_mut().set_latency_offset_ms(ms);
        }
        if let Some((_, row)) = self.rows.iter_mut().find(|(p, _)| p == "latency_offset_ms") {
            row.bind_mut().set_value_silently(ms as f64);
        }
        self.set_calibration_text(&format!("Offset set to {ms:+} ms"));
        play_sfx("ui_confirm");
    }
}

#[godot_api]
impl IVBoxContainer for SettingsPanel {
    fn ready(&mut self) {
        self.base_mut().add_theme_constant_override("separation", 6);
        self.base_mut().set_h_size_flags(SizeFlags::SHRINK_BEGIN);
        self.add_row("Master volume", "master_volume", OptionKind::Volume);
        self.add_row("Music", "music_volume", OptionKind::Volume);
        self.add_row("Effects", "sfx_volume", OptionKind::Volume);
        self.add_row("Screen shake", "screen_shake", OptionKind::Toggle);
        self.add_row("Reduce flashing", "reduce_flashing", OptionKind::Toggle);
        self.add_row(
            "Audio offset (press to calibrate)",
            "latency_offset_ms",
            OptionKind::Milliseconds {
                limit: LATENCY_LIMIT_MS as i64,
                step: 5,
            },
        );
        if SaveData::fullscreen_supported() {
            self.add_row("Fullscreen", "fullscreen", OptionKind::Toggle);
        }
        self.add_row("Show FPS", "show_fps", OptionKind::Toggle);
        self.add_row("Unlock all levels", "unlock_all", OptionKind::Toggle);

        let this = self.to_gd();
        if let Some((_, row)) = self.rows.iter().find(|(p, _)| p == "latency_offset_ms") {
            row.clone()
                .connect("activated", &this.callable("_on_calibrate"));
        }
        let mut calibration_label = label("", FontKind::Narrow, 20, palette::GOLD);
        calibration_label.set_visible(false);
        self.base_mut().add_child(&calibration_label);
        self.calibration_label = Some(calibration_label);

        let mut back = super::button("Back");
        back.set_name("Back");
        back.set_custom_minimum_size(Vector2::new(260.0, 46.0));
        back.set_h_size_flags(SizeFlags::SHRINK_BEGIN);
        back.connect("pressed", &this.callable("_on_back"));
        self.base_mut().add_child(&back);

        let mut focusables: Vec<Gd<Button>> =
            self.rows.iter().map(|(_, r)| r.clone().upcast()).collect();
        focusables.push(back.clone());
        super::widgets::link_vertical(&focusables);
        self.back = Some(back);
    }

    fn process(&mut self, _delta: f64) {
        let Some(calibration) = self.calibration.as_mut() else {
            return;
        };
        let spb = 60.0 / CALIBRATION_BPM;
        let elapsed = (Time::singleton().get_ticks_usec() - calibration.started_usec) as f64 / 1e6;
        // Beat k sounds at (k + 1) * spb so the first click has a lead-in.
        let due = ((elapsed / spb).floor() as usize).min(CALIBRATION_BEATS + 1);
        if due > calibration.beats_played && calibration.beats_played < CALIBRATION_BEATS {
            calibration.beats_played = due.min(CALIBRATION_BEATS);
            play_sfx("countdown_tick");
        }
        if elapsed > spb * (CALIBRATION_BEATS as f64 + 1.5) {
            let taps = std::mem::take(&mut calibration.taps);
            self.calibration = None;
            self.finish_calibration(&taps);
        }
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        if !self.base().is_visible_in_tree() {
            return;
        }
        if let Some(calibration) = self.calibration.as_mut() {
            if event.is_action_pressed("ui_accept") && !event.is_echo() {
                let spb = 60.0 / CALIBRATION_BPM;
                let elapsed =
                    (Time::singleton().get_ticks_usec() - calibration.started_usec) as f64 / 1e6;
                let beat = (elapsed / spb).round();
                if beat as usize > CALIBRATION_WARMUP && beat as usize <= CALIBRATION_BEATS {
                    calibration.taps.push(elapsed - beat * spb);
                }
                self.base_mut().accept_event();
            } else if super::is_back(&event) {
                self.close();
                self.base_mut().accept_event();
            }
            return;
        }
        if super::is_back(&event) && !super::transitioning() {
            self.close();
            if self.base().is_inside_tree() {
                self.base_mut().accept_event();
            }
        }
    }
}

/// Standalone settings screen opened from the title.
#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct SettingsScreen {
    base: Base<Control>,
}

#[godot_api]
impl SettingsScreen {
    #[func]
    fn _on_closed(&mut self) {
        go_to(super::TITLE_SCENE);
    }
}

#[godot_api]
impl IControl for SettingsScreen {
    fn ready(&mut self) {
        let mut root = self.to_gd().upcast::<Control>();
        super::full_rect(&mut root);
        add_backdrop(&mut root);
        let mut column = VBoxContainer::new_alloc();
        column.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        column.set_offset(godot::builtin::Side::LEFT, 80.0);
        column.set_offset(godot::builtin::Side::TOP, 44.0);
        column.add_theme_constant_override("separation", 12);
        column.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&column);
        column.add_child(&super::widgets::screen_header(
            "Settings",
            "Saved automatically.",
        ));
        let mut panel = SettingsPanel::new_alloc();
        panel.set_name("SettingsPanel");
        let this = self.to_gd();
        panel.connect("closed", &this.callable("_on_closed"));
        column.add_child(&panel);
        panel.bind_mut().focus_first();
        root.add_child(&nav_hints("Change", "Back"));
    }
}
