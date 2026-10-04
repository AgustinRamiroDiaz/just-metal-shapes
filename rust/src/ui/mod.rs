//! Menus, HUD, pause, results, settings and credits.
//!
//! Every screen builds its node tree in code on `ready` and relies on the project theme
//! (`godot/theme/main_theme.tres`, set as `gui/theme/custom`) for buttons, panels and
//! the default font. This module holds the shared palette, fonts and widget helpers;
//! `services.rs` is the `Ui` autoload (UI sounds, menu music, focus feedback, screen
//! transitions).

pub mod backdrop;
pub mod credits;
pub mod gym;
pub mod hud;
pub mod level_select;
pub mod level_ui;
pub mod lobby;
pub mod pause;
pub mod results;
pub mod services;
pub mod settings;
pub mod title;
pub mod widgets;

use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{
    BaseButton, Button, Control, Engine, Font, InputEvent, Label, Node, SceneTree, StyleBoxFlat,
    TextureRect, ThemeDb, Tween, texture_rect::ExpandMode, texture_rect::StretchMode,
};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

pub const TITLE_SCENE: &str = "res://scenes/ui/title.tscn";
pub const LEVEL_SELECT_SCENE: &str = "res://scenes/ui/level_select.tscn";
pub const LOBBY_SCENE: &str = "res://scenes/ui/lobby.tscn";
pub const SETTINGS_SCENE: &str = "res://scenes/ui/settings.tscn";
pub const CREDITS_SCENE: &str = "res://scenes/ui/credits.tscn";
pub const LEVEL_SCENE: &str = "res://main_level.tscn";

/// Shared colors. The accent changes per level (`set_accent`); the rest are fixed.
pub mod palette {
    use godot::prelude::Color;

    /// Background ink.
    pub const VOID: Color = Color::from_rgb(0.051, 0.043, 0.118);
    pub const PANEL: Color = Color::from_rgb(0.102, 0.090, 0.212);
    /// Borders, inactive pips, empty slots.
    pub const STEEL: Color = Color::from_rgb(0.204, 0.188, 0.369);
    /// Secondary text.
    pub const MIST: Color = Color::from_rgb(0.604, 0.588, 0.784);
    pub const TEXT: Color = Color::from_rgb(0.949, 0.941, 1.0);
    /// Default accent (title, credits, settings).
    pub const ACCENT: Color = Color::from_rgb(0.239, 0.910, 1.0);
    /// Danger, records, rewinds.
    pub const HOT: Color = Color::from_rgb(1.0, 0.239, 0.498);
    pub const GOLD: Color = Color::from_rgb(1.0, 0.847, 0.290);
    pub const GOOD: Color = Color::from_rgb(0.45, 1.0, 0.55);
}

#[derive(Clone, Copy, Debug)]
pub enum FontKind {
    /// Titles and the rank letter.
    Display,
    /// Default UI text.
    Ui,
    /// Dense HUD text.
    Narrow,
    /// Digits: scores, timers, BPM.
    Mono,
}

impl FontKind {
    fn path(self) -> &'static str {
        match self {
            FontKind::Display => "res://assets/fonts/kenney_rocket_square.ttf",
            FontKind::Ui => "res://assets/fonts/kenney_future.ttf",
            FontKind::Narrow => "res://assets/fonts/kenney_future_narrow.ttf",
            FontKind::Mono => "res://assets/fonts/kenney_mini_square_mono.ttf",
        }
    }
}

pub fn font(kind: FontKind) -> Option<Gd<Font>> {
    try_load::<Font>(kind.path()).ok()
}

pub fn label(text: &str, kind: FontKind, size: i32, color: Color) -> Gd<Label> {
    let mut label = Label::new_alloc();
    label.set_text(text);
    style_label(&mut label, kind, size, color);
    label
}

pub fn style_label(label: &mut Gd<Label>, kind: FontKind, size: i32, color: Color) {
    if let Some(font) = font(kind) {
        label.add_theme_font_override("font", &font);
    }
    label.add_theme_font_size_override("font_size", size);
    label.add_theme_color_override("font_color", color);
    label.set_mouse_filter(MouseFilter::IGNORE);
}

pub fn centered(mut label: Gd<Label>) -> Gd<Label> {
    label.set_horizontal_alignment(HorizontalAlignment::CENTER);
    label
}

/// Themed button that plays the confirm sound and dips on press.
pub fn button(text: &str) -> Gd<Button> {
    let mut button = Button::new_alloc();
    button.set_text(text);
    button.set_custom_minimum_size(Vector2::new(300.0, 0.0));
    wire_button_feedback(button.clone().upcast());
    button
}

pub fn wire_button_feedback(button: Gd<BaseButton>) {
    let mut target = button.clone();
    let pressed = button.clone();
    target.connect(
        "pressed",
        &Callable::from_fn("ui_button_pressed", move |_| {
            play_sfx("ui_confirm");
            press_bump(pressed.clone().upcast());
            Variant::nil()
        }),
    );
}

/// Quick squash on press.
pub fn press_bump(control: Gd<Control>) {
    if !control.is_inside_tree() {
        return;
    }
    let mut control = control;
    let size = control.get_size();
    control.set_pivot_offset(size * 0.5);
    let Some(mut tween) = make_tween(&control.clone().upcast()) else {
        return;
    };
    control.set_scale(Vector2::new(0.92, 0.92));
    tween
        .tween_property(
            &control,
            "scale",
            &Vector2::new(1.06, 1.06).to_variant(),
            0.14,
        )
        .set_trans(godot::classes::tween::TransitionType::BACK)
        .set_ease(godot::classes::tween::EaseType::OUT);
}

/// Tween bound to `node` that ignores `Engine.time_scale` and runs while paused.
pub fn make_tween(node: &Gd<Node>) -> Option<Gd<Tween>> {
    if !node.is_inside_tree() {
        return None;
    }
    let mut tween = node.clone().create_tween();
    tween.set_ignore_time_scale();
    tween.set_pause_mode(godot::classes::tween::TweenPauseMode::PROCESS);
    Some(tween)
}

/// Fades `control` in from transparent and `offset` pixels away, after `delay` seconds.
pub fn slide_in(control: &Gd<Control>, offset: Vector2, delay: f64, duration: f64) {
    let Some(mut tween) = make_tween(&control.clone().upcast()) else {
        return;
    };
    let mut control = control.clone();
    let mut modulate = control.get_modulate();
    modulate.a = 0.0;
    control.set_modulate(modulate);
    let target = control.get_position();
    control.set_position(target + offset);
    tween.set_parallel();
    tween
        .tween_property(&control, "modulate:a", &1.0.to_variant(), duration)
        .set_delay(delay);
    tween
        .tween_property(&control, "position", &target.to_variant(), duration)
        .set_delay(delay)
        .set_trans(godot::classes::tween::TransitionType::CUBIC)
        .set_ease(godot::classes::tween::EaseType::OUT);
}

/// Fades a control's alpha in after `delay` (use inside containers, where position is
/// owned by the layout).
pub fn fade_in(control: &Gd<Control>, delay: f64, duration: f64) {
    let Some(mut tween) = make_tween(&control.clone().upcast()) else {
        return;
    };
    let mut control = control.clone();
    let mut modulate = control.get_modulate();
    modulate.a = 0.0;
    control.set_modulate(modulate);
    tween
        .tween_property(&control, "modulate:a", &1.0.to_variant(), duration)
        .set_delay(delay);
}

pub fn full_rect(control: &mut Gd<impl Inherits<Control>>) {
    control
        .clone()
        .upcast::<Control>()
        .set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
}

pub fn spacer(height: f32) -> Gd<Control> {
    let mut spacer = Control::new_alloc();
    spacer.set_custom_minimum_size(Vector2::new(0.0, height));
    spacer.set_mouse_filter(MouseFilter::IGNORE);
    spacer
}

pub fn expand_h(control: &mut Gd<impl Inherits<Control>>) {
    control
        .clone()
        .upcast::<Control>()
        .set_h_size_flags(SizeFlags::EXPAND_FILL);
}

pub fn texture_rect(path: &str, size: f32) -> Gd<TextureRect> {
    let mut rect = TextureRect::new_alloc();
    if let Ok(texture) = try_load::<godot::classes::Texture2D>(path) {
        rect.set_texture(&texture);
    }
    rect.set_expand_mode(ExpandMode::IGNORE_SIZE);
    rect.set_stretch_mode(StretchMode::KEEP_ASPECT_CENTERED);
    rect.set_custom_minimum_size(Vector2::new(size, size));
    rect.set_mouse_filter(MouseFilter::IGNORE);
    rect
}

pub const KEYBOARD_GLYPHS: &str = "res://assets/kenney_input-prompts/keyboard/";
pub const XBOX_GLYPHS: &str = "res://assets/kenney_input-prompts/xbox/";

pub fn keyboard_glyph(name: &str, size: f32) -> Gd<TextureRect> {
    texture_rect(&format!("{KEYBOARD_GLYPHS}keyboard_{name}.png"), size)
}

pub fn pad_glyph(name: &str, size: f32) -> Gd<TextureRect> {
    texture_rect(&format!("{XBOX_GLYPHS}xbox_{name}.png"), size)
}

/// Recolors the theme's focus/hover/pressed accents (one accent per level palette).
pub fn set_accent(color: Color) {
    let Some(theme) = ThemeDb::singleton().get_project_theme() else {
        return;
    };
    for (style, set_bg, glow) in [
        ("focus", false, true),
        ("hover", false, false),
        ("pressed", true, false),
        ("hover_pressed", true, false),
    ] {
        let Some(stylebox) = theme.get_stylebox(style, "Button") else {
            continue;
        };
        let Ok(mut flat) = stylebox.try_cast::<StyleBoxFlat>() else {
            continue;
        };
        if set_bg {
            flat.set_bg_color(color);
        } else {
            flat.set_border_color(color);
        }
        if glow {
            flat.set_shadow_color(color.with_alpha(0.3));
        }
    }
    for (style, kind) in [("fill", "ProgressBar"), ("grabber_highlight", "VScrollBar")] {
        if let Some(Ok(mut flat)) = theme
            .get_stylebox(style, kind)
            .map(|s| s.try_cast::<StyleBoxFlat>())
        {
            flat.set_bg_color(color);
        }
    }
}

pub fn scene_tree() -> Option<Gd<SceneTree>> {
    Engine::singleton()
        .get_main_loop()
        .and_then(|l| l.try_cast::<SceneTree>().ok())
}

/// The `Ui` autoload, if registered.
pub fn services() -> Option<Gd<services::UiServices>> {
    scene_tree()?
        .get_root()?
        .get_node_or_null(services::AUTOLOAD_PATH)?
        .try_cast::<services::UiServices>()
        .ok()
}

/// Plays `res://assets/sfx/<name>.ogg` on the SFX bus (no-op without the autoload).
pub fn play_sfx(name: &str) {
    if let Some(mut ui) = services() {
        ui.bind_mut().play_sfx(name.into());
    }
}

/// Fades to `scene_path` through the `Ui` autoload (plain scene change without it).
pub fn go_to(scene_path: &str) {
    if let Some(mut ui) = services() {
        ui.bind_mut().go_to(scene_path.into());
    } else if let Some(mut tree) = scene_tree() {
        tree.set_pause(false);
        tree.change_scene_to_file(scene_path);
    }
}

/// Sets `GameConfig.gym`: whether the next level started from the lobby is the gym.
pub fn set_gym_mode(gym: bool) {
    if let Some(mut config) = scene_tree()
        .and_then(|tree| tree.get_root())
        .and_then(|root| root.get_node_or_null("GameConfig"))
        .and_then(|node| node.try_cast::<crate::game_config::GameConfig>().ok())
    {
        config.bind_mut().gym = gym;
    }
}

/// `GameConfig.gym`.
pub fn gym_mode() -> bool {
    scene_tree()
        .and_then(|tree| tree.get_root())
        .and_then(|root| root.get_node_or_null("GameConfig"))
        .and_then(|node| node.try_cast::<crate::game_config::GameConfig>().ok())
        .is_some_and(|config| config.bind().gym)
}

/// True while a screen transition runs (screens ignore input then).
pub fn transitioning() -> bool {
    services().is_some_and(|ui| ui.bind().is_transitioning())
}

pub fn is_back(event: &Gd<InputEvent>) -> bool {
    event
        .is_action_pressed_ex("ui_cancel")
        .exact_match(false)
        .done()
        && !event.is_echo()
}

pub fn grab_focus_deferred(control: &Gd<impl Inherits<Control>>) {
    let control = control.clone().upcast::<Control>();
    control.clone().call_deferred("grab_focus", &[]);
}

/// Linear interpolation between two colors.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    a.lerp(b, t as f64)
}

/// `mm:ss`.
pub fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as i64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Mode names as shown in menus.
pub fn mode_name(mode: i32) -> &'static str {
    match mode {
        0 => "Casual",
        2 => "Hardcore",
        _ => "Normal",
    }
}

pub fn rank_color(rank: &str) -> Color {
    match rank {
        "S" => palette::GOLD,
        "A" => palette::GOOD,
        "B" => palette::ACCENT,
        "C" => palette::MIST,
        _ => palette::HOT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(65.9), "1:05");
        assert_eq!(format_time(-3.0), "0:00");
    }
}
