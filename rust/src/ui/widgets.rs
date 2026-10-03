//! Small reusable controls: settings rows, the hold-to-start ring, difficulty pips,
//! skewed tags and player shape icons.

use super::{FontKind, label, palette, play_sfx, style_label};
use godot::classes::control::{LayoutPreset, MouseFilter};
use godot::classes::{
    Button, Control, IButton, IControl, InputEvent, InputEventMouseButton, Label, PanelContainer,
    Shader, ShaderMaterial, StyleBoxFlat, TextureRect,
};
use godot::global::{HorizontalAlignment, MouseButton};
use godot::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum OptionKind {
    /// `0..=1` in steps of 0.05, shown as a segmented bar.
    #[default]
    Volume,
    Toggle,
    /// Integer milliseconds within `-limit..=limit`.
    Milliseconds {
        limit: i64,
        step: i64,
    },
}

/// One settings line: focusable, left/right adjusts, accept toggles, mouse clicks on
/// the value bar set it directly. Emits `value_changed`.
#[derive(GodotClass)]
#[class(init, base = Button)]
pub struct OptionRow {
    kind: OptionKind,
    value: f64,
    value_label: Option<Gd<Label>>,
    bar: Option<Gd<ValueBar>>,
    base: Base<Button>,
}

#[godot_api]
impl OptionRow {
    #[signal]
    pub fn value_changed(value: f64);
    /// Accept/click on a row that is not a toggle.
    #[signal]
    pub fn activated();

    #[func]
    pub fn get_value(&self) -> f64 {
        self.value
    }

    /// Sets the value without emitting `value_changed`.
    #[func]
    pub fn set_value_silently(&mut self, value: f64) {
        self.value = self.clamp(value);
        self.refresh();
    }

    #[func]
    pub fn adjust(&mut self, direction: i32) {
        let next = match self.kind {
            OptionKind::Volume => ((self.value * 20.0).round() + direction as f64) / 20.0,
            OptionKind::Toggle => {
                if self.value > 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
            OptionKind::Milliseconds { step, .. } => self.value + (direction as i64 * step) as f64,
        };
        self.set_value(next);
    }

    #[func]
    fn _on_bar_clicked(&mut self, fraction: f64) {
        if self.kind == OptionKind::Volume {
            self.set_value((fraction * 20.0).round() / 20.0);
        }
    }
}

impl OptionRow {
    pub fn create(title: &str, kind: OptionKind, value: f64) -> Gd<Self> {
        let mut row = Gd::from_init_fn(|base| Self {
            kind,
            value: 0.0,
            value_label: None,
            bar: None,
            base,
        });
        row.set_text(title);
        row.set_text_alignment(HorizontalAlignment::LEFT);
        row.set_custom_minimum_size(Vector2::new(760.0, 44.0));
        let clamped = row.bind().clamp(value);
        row.bind_mut().value = clamped;
        row
    }

    fn clamp(&self, value: f64) -> f64 {
        match self.kind {
            OptionKind::Volume => value.clamp(0.0, 1.0),
            OptionKind::Toggle => {
                if value > 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            OptionKind::Milliseconds { limit, .. } => {
                value.round().clamp(-limit as f64, limit as f64)
            }
        }
    }

    fn set_value(&mut self, value: f64) {
        let value = self.clamp(value);
        if (value - self.value).abs() < 1e-9 {
            play_sfx("ui_error");
            return;
        }
        self.value = value;
        self.refresh();
        play_sfx("ui_move");
        self.signals().value_changed().emit(value);
    }

    fn refresh(&mut self) {
        let text = match self.kind {
            OptionKind::Volume => format!("{:>3}%", (self.value * 100.0).round() as i64),
            OptionKind::Toggle => {
                if self.value > 0.5 {
                    "On".into()
                } else {
                    "Off".into()
                }
            }
            OptionKind::Milliseconds { .. } => format!("{:+} ms", self.value as i64),
        };
        if let Some(label) = self.value_label.as_mut() {
            label.set_text(&text);
            let color = if self.kind == OptionKind::Toggle && self.value < 0.5 {
                palette::MIST
            } else {
                palette::TEXT
            };
            label.add_theme_color_override("font_color", color);
        }
        if let Some(bar) = self.bar.as_mut() {
            bar.bind_mut().fraction = self.value as f32;
            bar.queue_redraw();
        }
    }
}

#[godot_api]
impl IButton for OptionRow {
    fn ready(&mut self) {
        let mut value_box = godot::classes::HBoxContainer::new_alloc();
        value_box.set_anchors_and_offsets_preset(LayoutPreset::RIGHT_WIDE);
        value_box.set_offset(godot::builtin::Side::LEFT, -340.0);
        value_box.set_offset(godot::builtin::Side::RIGHT, -24.0);
        value_box.set_alignment(godot::classes::box_container::AlignmentMode::END);
        value_box.add_theme_constant_override("separation", 16);
        value_box.set_mouse_filter(MouseFilter::IGNORE);

        if self.kind == OptionKind::Volume {
            let mut bar = Gd::<ValueBar>::from_init_fn(|base| ValueBar {
                fraction: 0.0,
                base,
            });
            bar.set_custom_minimum_size(Vector2::new(200.0, 22.0));
            bar.set_v_size_flags(godot::classes::control::SizeFlags::SHRINK_CENTER);
            let this = self.to_gd();
            bar.connect("clicked", &this.callable("_on_bar_clicked"));
            value_box.add_child(&bar);
            self.bar = Some(bar);
        }
        let mut value_label = label("", FontKind::Mono, 26, palette::TEXT);
        value_label.set_custom_minimum_size(Vector2::new(90.0, 0.0));
        value_label.set_horizontal_alignment(HorizontalAlignment::RIGHT);
        value_label.set_v_size_flags(godot::classes::control::SizeFlags::SHRINK_CENTER);
        value_box.add_child(&value_label);
        self.value_label = Some(value_label);
        self.base_mut().add_child(&value_box);
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
            self.adjust(if right { 1 } else { -1 });
            self.base_mut().accept_event();
        }
    }

    fn pressed(&mut self) {
        if self.kind == OptionKind::Toggle {
            self.adjust(1);
        } else {
            self.signals().activated().emit();
        }
    }
}

/// Segmented horizontal bar; clicking sets the fraction.
#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct ValueBar {
    pub fraction: f32,
    base: Base<Control>,
}

#[godot_api]
impl ValueBar {
    #[signal]
    pub fn clicked(fraction: f64);
}

#[godot_api]
impl IControl for ValueBar {
    fn draw(&mut self) {
        const SEGMENTS: i32 = 20;
        let size = self.base().get_size();
        let gap = 3.0;
        let width = (size.x - gap * (SEGMENTS - 1) as f32) / SEGMENTS as f32;
        let filled = (self.fraction * SEGMENTS as f32).round() as i32;
        let accent = super::services()
            .map(|ui| ui.bind().get_accent())
            .unwrap_or(palette::ACCENT);
        for i in 0..SEGMENTS {
            let x = i as f32 * (width + gap);
            // Segments grow taller left to right, like a volume wedge.
            let h = size.y * (0.45 + 0.55 * (i as f32 + 1.0) / SEGMENTS as f32);
            let rect = Rect2::new(Vector2::new(x, size.y - h), Vector2::new(width, h));
            let color = if i < filled { accent } else { palette::STEEL };
            self.base_mut().draw_rect(rect, color);
        }
    }

    fn gui_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mouse) = event.try_cast::<InputEventMouseButton>()
            && mouse.is_pressed()
            && mouse.get_button_index() == MouseButton::LEFT
        {
            let width = self.base().get_size().x.max(1.0);
            let fraction = (mouse.get_position().x / width).clamp(0.0, 1.0) as f64;
            self.signals().clicked().emit(fraction);
            self.base_mut().accept_event();
        }
    }
}

/// Circular progress ring (hold-to-start).
#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct HoldRing {
    #[var]
    pub progress: f64,
    #[var]
    pub color: Color,
    base: Base<Control>,
}

#[godot_api]
impl IControl for HoldRing {
    fn ready(&mut self) {
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
    }

    fn draw(&mut self) {
        let size = self.base().get_size();
        let center = size * 0.5;
        let radius = size.x.min(size.y) * 0.5 - 4.0;
        let tau = std::f32::consts::TAU;
        let start = -std::f32::consts::FRAC_PI_2;
        self.base_mut()
            .draw_arc_ex(center, radius, 0.0, tau, 48, palette::STEEL)
            .width(5.0)
            .antialiased(true)
            .done();
        let progress = self.progress.clamp(0.0, 1.0) as f32;
        if progress > 0.0 {
            let color = self.color;
            self.base_mut()
                .draw_arc_ex(center, radius, start, start + tau * progress, 48, color)
                .width(7.0)
                .antialiased(true)
                .done();
        }
    }
}

/// Five difficulty pips, `level` of them lit.
#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct DifficultyPips {
    #[var]
    pub level: i64,
    #[var]
    pub color: Color,
    base: Base<Control>,
}

#[godot_api]
impl IControl for DifficultyPips {
    fn ready(&mut self) {
        self.base_mut()
            .set_custom_minimum_size(Vector2::new(5.0 * 22.0, 18.0));
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
    }

    fn draw(&mut self) {
        let h = self.base().get_size().y;
        for i in 0..5 {
            let x = i as f32 * 22.0;
            // Skewed parallelogram pips echo the button shape.
            let points = PackedVector2Array::from(&[
                Vector2::new(x + 5.0, 0.0),
                Vector2::new(x + 18.0, 0.0),
                Vector2::new(x + 13.0, h),
                Vector2::new(x, h),
            ]);
            let color = if (i as i64) < self.level {
                self.color
            } else {
                palette::STEEL
            };
            self.base_mut().draw_colored_polygon(&points, color);
        }
    }
}

/// A short skewed label on a solid color (badges such as "New best" or "Locked").
pub fn tag(text: &str, bg: Color, fg: Color, size: i32) -> Gd<PanelContainer> {
    let mut panel = PanelContainer::new_alloc();
    let mut style = StyleBoxFlat::new_gd();
    style.set_bg_color(bg);
    style.set_skew(Vector2::new(0.22, 0.0));
    style.set_content_margin_all(4.0);
    style.set_content_margin(godot::builtin::Side::LEFT, 14.0);
    style.set_content_margin(godot::builtin::Side::RIGHT, 14.0);
    panel.add_theme_stylebox_override("panel", &style);
    panel.set_mouse_filter(MouseFilter::IGNORE);
    let mut text_label = label(text, FontKind::Ui, size, fg);
    text_label.set_horizontal_alignment(HorizontalAlignment::CENTER);
    panel.add_child(&text_label);
    panel
}

/// Changes a `tag`'s text and colors.
pub fn retag(panel: &mut Gd<PanelContainer>, text: &str, bg: Color, fg: Color) {
    if let Some(Ok(mut style)) = panel
        .get_theme_stylebox("panel")
        .map(|s| s.try_cast::<StyleBoxFlat>())
    {
        style.set_bg_color(bg);
    }
    if let Some(Ok(mut text_label)) = panel.get_child(0).map(|c| c.try_cast::<Label>()) {
        text_label.set_text(text);
        text_label.add_theme_color_override("font_color", fg);
    }
}

pub const SHAPES: [&str; 4] = ["squircle", "circle", "square", "rhombus"];

/// A Kenney shape body tinted like the in-game player (`player_color.gdshader`).
pub fn shape_icon(shape: &str, color: Color, size: f32) -> Gd<TextureRect> {
    let path = format!("res://assets/kenney_shape-characters/PNG/Default/blue_body_{shape}.png");
    let mut icon = super::texture_rect(&path, size);
    if let Ok(shader) = try_load::<Shader>("res://shaders/player_color.gdshader") {
        let mut material = ShaderMaterial::new_gd();
        material.set_shader(&shader);
        material.set_shader_parameter("player_color", &color.to_variant());
        icon.set_material(&material);
    }
    icon
}

pub fn set_shape_color(icon: &mut Gd<TextureRect>, color: Color) {
    if let Some(Ok(mut material)) = icon.get_material().map(|m| m.try_cast::<ShaderMaterial>()) {
        material.set_shader_parameter("player_color", &color.to_variant());
    }
}

/// Header row used by every menu screen: big title on the left, optional subtitle.
pub fn screen_header(title: &str, subtitle: &str) -> Gd<Control> {
    let mut column = godot::classes::VBoxContainer::new_alloc();
    column.add_theme_constant_override("separation", 0);
    column.set_mouse_filter(MouseFilter::IGNORE);
    column.add_child(&label(title, FontKind::Display, 64, palette::TEXT));
    if !subtitle.is_empty() {
        let mut sub = label(subtitle, FontKind::Ui, 20, palette::MIST);
        style_label(&mut sub, FontKind::Ui, 20, palette::MIST);
        column.add_child(&sub);
    }
    column.upcast()
}

/// Bottom-bar hint: a glyph per device followed by an action name.
pub fn hint(glyphs: &[Gd<TextureRect>], text: &str) -> Gd<Control> {
    let mut row = godot::classes::HBoxContainer::new_alloc();
    row.add_theme_constant_override("separation", 4);
    row.set_mouse_filter(MouseFilter::IGNORE);
    for glyph in glyphs {
        row.add_child(glyph);
    }
    let mut text_label = label(text, FontKind::Narrow, 20, palette::MIST);
    text_label.set_v_size_flags(godot::classes::control::SizeFlags::SHRINK_CENTER);
    row.add_child(&text_label);
    row.upcast()
}

/// Bottom hint bar anchored to the screen's lower edge.
pub fn hint_bar(hints: Vec<Gd<Control>>) -> Gd<Control> {
    let mut bar = godot::classes::HBoxContainer::new_alloc();
    bar.set_anchors_and_offsets_preset(LayoutPreset::BOTTOM_WIDE);
    bar.set_offset(godot::builtin::Side::TOP, -64.0);
    bar.set_offset(godot::builtin::Side::BOTTOM, -18.0);
    bar.set_offset(godot::builtin::Side::LEFT, 64.0);
    bar.set_offset(godot::builtin::Side::RIGHT, -64.0);
    bar.add_theme_constant_override("separation", 36);
    bar.set_mouse_filter(MouseFilter::IGNORE);
    for hint in hints {
        bar.add_child(&hint);
    }
    bar.upcast()
}

/// Standard hints for focus-driven screens.
pub fn nav_hints(confirm: &str, back: &str) -> Gd<Control> {
    let mut hints = vec![hint(
        &[
            super::keyboard_glyph("arrows", 34.0),
            super::pad_glyph("dpad", 34.0),
        ],
        "Move",
    )];
    if !confirm.is_empty() {
        hints.push(hint(
            &[
                super::keyboard_glyph("enter", 34.0),
                super::pad_glyph("button_color_a", 34.0),
            ],
            confirm,
        ));
    }
    if !back.is_empty() {
        hints.push(hint(
            &[
                super::keyboard_glyph("escape", 34.0),
                super::pad_glyph("button_color_b", 34.0),
            ],
            back,
        ));
    }
    hint_bar(hints)
}

/// Builds a vertical list of buttons with wrap-around focus and returns them.
pub fn link_vertical(buttons: &[Gd<Button>]) {
    let n = buttons.len();
    if n < 2 {
        return;
    }
    for (i, button) in buttons.iter().enumerate() {
        let mut button = button.clone();
        let prev = buttons[(i + n - 1) % n].get_path();
        let next = buttons[(i + 1) % n].get_path();
        button.set_focus_neighbor(godot::builtin::Side::TOP, &prev);
        button.set_focus_neighbor(godot::builtin::Side::BOTTOM, &next);
    }
}
