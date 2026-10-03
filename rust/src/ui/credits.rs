//! `CreditsScreen`: every music, art, sound and engine credit from `core::credits`,
//! including the CC BY attribution text for the Kevin MacLeod tracks. Scrolls on its
//! own; up/down scroll manually.

use super::backdrop::add_backdrop;
use super::widgets::{hint, hint_bar, tag};
use super::{FontKind, go_to, keyboard_glyph, label, pad_glyph, palette, play_sfx};
use crate::core::credits::{CREDITS, SECTION_ORDER};
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{Control, IControl, InputEvent, ScrollContainer, VBoxContainer};
use godot::prelude::*;

const AUTO_SCROLL_SPEED: f64 = 40.0;
const MANUAL_STEP: f64 = 80.0;
/// Seconds of no input before auto-scroll resumes.
const IDLE_BEFORE_SCROLL: f64 = 2.5;

#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct CreditsScreen {
    scroll: Option<Gd<ScrollContainer>>,
    position: f64,
    idle: f64,
    text: String,
    base: Base<Control>,
}

#[godot_api]
impl CreditsScreen {
    /// All credit text shown on screen (tests).
    #[func]
    pub fn get_credits_text(&self) -> GString {
        GString::from(&self.text)
    }
}

impl CreditsScreen {
    fn add_text(
        &mut self,
        parent: &mut Gd<VBoxContainer>,
        text: &str,
        kind: FontKind,
        size: i32,
        color: Color,
    ) {
        let mut line = label(text, kind, size, color);
        line.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD);
        parent.add_child(&line);
        self.text.push_str(text);
        self.text.push('\n');
    }
}

#[godot_api]
impl IControl for CreditsScreen {
    fn ready(&mut self) {
        let mut root = self.to_gd().upcast::<Control>();
        super::full_rect(&mut root);
        add_backdrop(&mut root);

        let mut column = VBoxContainer::new_alloc();
        column.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        column.set_offset(godot::builtin::Side::LEFT, 80.0);
        column.set_offset(godot::builtin::Side::RIGHT, -80.0);
        column.set_offset(godot::builtin::Side::TOP, 44.0);
        column.set_offset(godot::builtin::Side::BOTTOM, -80.0);
        column.add_theme_constant_override("separation", 16);
        column.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&column);
        column.add_child(&super::widgets::screen_header(
            "Credits",
            "Just Metal Shapes is built from these works. Thank you.",
        ));

        let mut scroll = ScrollContainer::new_alloc();
        scroll.set_v_size_flags(SizeFlags::EXPAND_FILL);
        scroll.set_horizontal_scroll_mode(godot::classes::scroll_container::ScrollMode::DISABLED);
        column.add_child(&scroll);
        let mut content = VBoxContainer::new_alloc();
        content.set_h_size_flags(SizeFlags::EXPAND_FILL);
        content.add_theme_constant_override("separation", 6);
        scroll.add_child(&content);

        for section in SECTION_ORDER {
            content.add_child(&super::spacer(14.0));
            self.add_text(
                &mut content,
                section.title(),
                FontKind::Display,
                34,
                palette::ACCENT,
            );
            for credit in CREDITS.iter().filter(|c| c.section == section) {
                content.add_child(&super::spacer(6.0));
                self.add_text(&mut content, credit.title, FontKind::Ui, 26, palette::TEXT);
                let byline = format!(
                    "{}  /  {}  /  {}",
                    credit.author, credit.license, credit.source
                );
                self.add_text(&mut content, &byline, FontKind::Narrow, 20, palette::MIST);
                if !credit.attribution.is_empty() {
                    let mut box_ = VBoxContainer::new_alloc();
                    box_.add_theme_constant_override("separation", 2);
                    let badge = tag("Attribution", palette::GOLD, palette::VOID, 16);
                    badge.clone().set_h_size_flags(SizeFlags::SHRINK_BEGIN);
                    box_.add_child(&badge);
                    for line in credit.attribution.lines() {
                        self.add_text(&mut box_, line, FontKind::Narrow, 20, palette::TEXT);
                    }
                    content.add_child(&box_);
                }
            }
        }
        content.add_child(&super::spacer(200.0));
        self.scroll = Some(scroll);

        let glyph = 34.0;
        root.add_child(&hint_bar(vec![
            hint(
                &[keyboard_glyph("arrows", glyph), pad_glyph("dpad", glyph)],
                "Scroll",
            ),
            hint(
                &[
                    keyboard_glyph("escape", glyph),
                    pad_glyph("button_color_b", glyph),
                ],
                "Back",
            ),
        ]));
        self.idle = IDLE_BEFORE_SCROLL - 1.0;
    }

    fn process(&mut self, delta: f64) {
        let Some(mut scroll) = self.scroll.clone() else {
            return;
        };
        let input = godot::classes::Input::singleton();
        let manual = input.get_axis("ui_up", "ui_down") as f64;
        if manual.abs() > 0.1 {
            self.idle = 0.0;
            self.position += manual * MANUAL_STEP * 6.0 * delta;
        } else {
            self.idle += delta;
            if self.idle > IDLE_BEFORE_SCROLL {
                self.position += AUTO_SCROLL_SPEED * delta;
            }
        }
        let max = scroll
            .get_v_scroll_bar()
            .map_or(0.0, |bar| (bar.get_max() - bar.get_page()).max(0.0));
        self.position = self.position.clamp(0.0, max);
        scroll.set_v_scroll(self.position.round() as i32);
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if super::is_back(&event) && !super::transitioning() {
            play_sfx("ui_back");
            go_to(super::TITLE_SCENE);
            self.base_mut().accept_event();
        }
    }
}
