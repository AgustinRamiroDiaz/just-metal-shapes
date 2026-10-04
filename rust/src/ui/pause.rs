//! `PauseMenu`: the `pause` action (Esc / Start, any device) pauses the tree and the
//! Conductor and opens Resume / Restart / Settings / Quit to level select (to title in
//! the gym). Resuming counts three beats on the HUD before the song continues.

use super::hud::Hud;
use super::settings::SettingsPanel;
use super::widgets::link_vertical;
use super::{FontKind, go_to, label, palette, play_sfx};
use crate::conductor::Conductor;
use godot::classes::control::{LayoutPreset, MouseFilter};
use godot::classes::node::ProcessMode;
use godot::classes::{
    Button, CanvasLayer, ColorRect, Control, ICanvasLayer, InputEvent, Node, VBoxContainer,
};
use godot::prelude::*;

const RESUME_BEATS: i64 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PauseState {
    Running,
    Menu,
    Settings,
    /// Counting beats back in; `left` seconds to the next beat, `beats` remaining.
    Resuming {
        left: f64,
        beats: i64,
    },
}

#[derive(GodotClass)]
#[class(init, base = CanvasLayer)]
pub struct PauseMenu {
    #[init(val = PauseState::Running)]
    state: PauseState,
    manager: Option<Gd<Node>>,
    conductor: Option<Gd<Conductor>>,
    hud: Option<Gd<Hud>>,
    dim: Option<Gd<ColorRect>>,
    menu: Option<Gd<VBoxContainer>>,
    settings: Option<Gd<SettingsPanel>>,
    buttons: Vec<Gd<Button>>,
    base: Base<CanvasLayer>,
}

#[godot_api]
impl PauseMenu {
    #[signal]
    pub fn pause_changed(paused: bool);

    /// Opens the menu if the level is running.
    #[func]
    pub fn open(&mut self) {
        if self.state != PauseState::Running || !self.level_running() || super::transitioning() {
            return;
        }
        self.base().get_tree().set_pause(true);
        if let Some(mut conductor) = self.conductor.clone() {
            conductor.bind_mut().pause();
        }
        play_sfx("pause");
        self.state = PauseState::Menu;
        self.base_mut().set_visible(true);
        if let Some(hud) = self.hud.as_mut() {
            hud.bind_mut().hide_count();
        }
        self.show_menu(0);
        self.signals().pause_changed().emit(true);
    }

    /// Closes the menu and counts back in.
    #[func]
    pub fn resume(&mut self) {
        if !matches!(self.state, PauseState::Menu | PauseState::Settings) {
            return;
        }
        if let Some(mut menu) = self.menu.clone() {
            menu.set_visible(false);
        }
        if let Some(mut settings) = self.settings.clone() {
            settings.set_visible(false);
        }
        if let Some(mut dim) = self.dim.clone() {
            dim.set_color(palette::VOID.with_alpha(0.35));
        }
        self.state = PauseState::Resuming {
            left: 0.0,
            beats: RESUME_BEATS,
        };
    }

    #[func]
    pub fn is_open(&self) -> bool {
        self.state != PauseState::Running
    }

    /// `running`, `menu`, `settings` or `resuming` (tests).
    #[func]
    pub fn get_state(&self) -> GString {
        GString::from(match self.state {
            PauseState::Running => "running",
            PauseState::Menu => "menu",
            PauseState::Settings => "settings",
            PauseState::Resuming { .. } => "resuming",
        })
    }

    #[func]
    fn _on_resume(&mut self) {
        self.resume();
    }

    #[func]
    fn _on_restart(&mut self) {
        go_to(super::LEVEL_SCENE);
    }

    #[func]
    fn _on_settings(&mut self) {
        if self.state != PauseState::Menu {
            return;
        }
        self.state = PauseState::Settings;
        if let Some(mut menu) = self.menu.clone() {
            menu.set_visible(false);
        }
        if let Some(mut settings) = self.settings.clone() {
            settings.set_visible(true);
            super::fade_in(&settings.clone().upcast(), 0.0, 0.15);
            settings.bind_mut().focus_first();
        }
    }

    #[func]
    fn _on_settings_closed(&mut self) {
        if let Some(mut settings) = self.settings.clone() {
            settings.set_visible(false);
        }
        if let (Some(mut conductor), Some(save)) = (
            self.conductor.clone(),
            crate::save::save_data(&self.to_gd().upcast()),
        ) {
            conductor.bind_mut().latency_offset = save.bind().latency_offset_seconds();
        }
        self.state = PauseState::Menu;
        self.show_menu(2);
    }

    #[func]
    fn _on_quit(&mut self) {
        if super::gym_mode() {
            go_to(super::TITLE_SCENE);
        } else {
            go_to(super::LEVEL_SELECT_SCENE);
        }
    }
}

impl PauseMenu {
    fn level_running(&self) -> bool {
        let Some(manager) = self.manager.clone() else {
            return false;
        };
        let state = manager.clone().call("get_state", &[]).to_string();
        state == "playing" || state == "countdown"
    }

    fn show_menu(&mut self, focus: usize) {
        if let Some(mut dim) = self.dim.clone() {
            dim.set_color(palette::VOID.with_alpha(0.72));
        }
        if let Some(mut menu) = self.menu.clone() {
            menu.set_visible(true);
            super::slide_in(&menu.clone().upcast(), Vector2::new(-40.0, 0.0), 0.0, 0.2);
        }
        if let Some(button) = self.buttons.get(focus) {
            super::grab_focus_deferred(button);
        }
    }

    fn finish_resume(&mut self) {
        self.state = PauseState::Running;
        self.base_mut().set_visible(false);
        self.base().get_tree().set_pause(false);
        if let Some(mut conductor) = self.conductor.clone() {
            conductor.bind_mut().resume();
        }
        play_sfx("unpause");
        self.signals().pause_changed().emit(false);
    }

    fn build(&mut self) {
        let mut root = Control::new_alloc();
        root.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);
        self.base_mut().add_child(&root);

        let mut dim = ColorRect::new_alloc();
        dim.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        dim.set_color(palette::VOID.with_alpha(0.72));
        root.add_child(&dim);

        let mut menu = VBoxContainer::new_alloc();
        menu.set_anchors_and_offsets_preset(LayoutPreset::LEFT_WIDE);
        menu.set_offset(godot::builtin::Side::LEFT, 110.0);
        menu.set_offset(godot::builtin::Side::RIGHT, 700.0);
        menu.set_offset(godot::builtin::Side::TOP, 150.0);
        menu.add_theme_constant_override("separation", 14);
        menu.set_mouse_filter(MouseFilter::IGNORE);
        menu.add_child(&label("Paused", FontKind::Display, 80, palette::TEXT));
        menu.add_child(&super::spacer(10.0));
        let this = self.to_gd();
        for (text, method) in [
            ("Resume", "_on_resume"),
            ("Restart", "_on_restart"),
            ("Settings", "_on_settings"),
            ("Quit to level select", "_on_quit"),
        ] {
            let mut b = super::button(if method == "_on_quit" && super::gym_mode() {
                "Quit to title"
            } else {
                text
            });
            b.set_name(&text.replace(' ', ""));
            b.set_custom_minimum_size(Vector2::new(380.0, 56.0));
            b.set_h_size_flags(godot::classes::control::SizeFlags::SHRINK_BEGIN);
            b.set_text_alignment(godot::global::HorizontalAlignment::LEFT);
            b.connect("pressed", &this.callable(method));
            menu.add_child(&b);
            self.buttons.push(b);
        }
        root.add_child(&menu);
        link_vertical(&self.buttons);

        let mut settings = SettingsPanel::new_alloc();
        settings.set_name("SettingsPanel");
        settings.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        settings.set_offset(godot::builtin::Side::LEFT, 110.0);
        settings.set_offset(godot::builtin::Side::TOP, 60.0);
        settings.set_visible(false);
        settings.connect("closed", &this.callable("_on_settings_closed"));
        root.add_child(&settings);

        self.dim = Some(dim);
        self.menu = Some(menu);
        self.settings = Some(settings);
    }
}

#[godot_api]
impl ICanvasLayer for PauseMenu {
    fn ready(&mut self) {
        self.base_mut().set_layer(20);
        self.base_mut().set_process_mode(ProcessMode::ALWAYS);
        self.base_mut().set_visible(false);
        let level_ui = self.base().get_parent();
        self.manager = level_ui.and_then(|p| p.get_parent());
        self.conductor = self
            .manager
            .as_ref()
            .and_then(|m| m.try_get_node_as::<Conductor>("Conductor"));
        self.hud = self
            .base()
            .get_parent()
            .and_then(|p| p.try_get_node_as::<Hud>("Hud"));
        self.build();
    }

    fn process(&mut self, delta: f64) {
        let PauseState::Resuming { left, beats } = self.state else {
            return;
        };
        let left = left - delta;
        if left > 0.0 {
            self.state = PauseState::Resuming { left, beats };
            return;
        }
        if beats == 0 {
            if let Some(hud) = self.hud.as_mut() {
                hud.bind_mut().hide_count();
            }
            self.finish_resume();
            return;
        }
        let spb = self
            .conductor
            .as_ref()
            .map_or(0.5, |c| c.bind().seconds_per_beat())
            .clamp(0.3, 0.8);
        play_sfx("countdown_tick");
        if let Some(hud) = self.hud.as_mut() {
            hud.bind_mut()
                .show_count(beats.to_string().as_str().into(), spb + 0.1);
        }
        self.state = PauseState::Resuming {
            left: left + spb,
            beats: beats - 1,
        };
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if event.is_echo() {
            return;
        }
        let pause = event.is_action_pressed("pause");
        match self.state {
            PauseState::Running if pause => {
                self.open();
                if let Some(mut viewport) = self.base().get_viewport() {
                    viewport.set_input_as_handled();
                }
            }
            PauseState::Menu if pause || super::is_back(&event) => {
                play_sfx("ui_back");
                self.resume();
                if let Some(mut viewport) = self.base().get_viewport() {
                    viewport.set_input_as_handled();
                }
            }
            _ => {}
        }
    }
}
