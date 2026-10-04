//! `GymPanel`: the enemy gym's tools, added by `LevelUi` when `GameConfig.gym` is set.
//!
//! A sidebar on the right lists every enemy type (`level_catalog::ENEMY_KINDS`) as a
//! card in a scrolling column; dragging a card onto the arena (`GymDropZone`, the rest of
//! the screen) spawns that enemy at the drop point through `GameManager.gym_spawn`. The
//! header holds Clear, God mode, Hide (also H) and the song switch.

use super::{FontKind, label, palette, play_sfx};
use crate::groups;
use crate::level_catalog::{ENEMY_KINDS, all_levels};
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::scroll_container::ScrollMode;
use godot::classes::{
    Button, CanvasLayer, Control, HBoxContainer, ICanvasLayer, IControl, IPanelContainer,
    InputEvent, InputEventKey, Label, Node, PanelContainer, ScrollContainer, Shader,
    ShaderMaterial, StyleBoxFlat, TextureRect, VBoxContainer,
};
use godot::global::Key;
use godot::prelude::*;

pub const SIDEBAR_WIDTH: f32 = 300.0;
/// Drag payload key holding the `ENEMY_KINDS` index.
const DRAG_KEY: &str = "gym_enemy";
const ICON_SIZE: f32 = 52.0;
const AMBER: Color = Color::from_rgb(1.0, 0.68, 0.2);

#[derive(GodotClass)]
#[class(init, base = CanvasLayer)]
pub struct GymPanel {
    manager: Option<Gd<Node>>,
    sidebar: Option<Gd<PanelContainer>>,
    drop_zone: Option<Gd<GymDropZone>>,
    show_tab: Option<Gd<Button>>,
    cards: Vec<Gd<GymCard>>,
    count_label: Option<Gd<Label>>,
    god_button: Option<Gd<Button>>,
    song_button: Option<Gd<Button>>,
    god_mode: bool,
    base: Base<CanvasLayer>,
}

#[godot_api]
impl GymPanel {
    /// Spawns `ENEMY_KINDS[kind]` under the screen point `screen_position` (what a drop
    /// does). False for an unknown kind.
    #[func]
    pub fn drop_enemy(&mut self, kind: i64, screen_position: Vector2) -> bool {
        let Some(mut manager) = self.manager.clone() else {
            return false;
        };
        let world = self
            .base()
            .get_viewport()
            .map_or(screen_position, |viewport| {
                viewport.get_canvas_transform().affine_inverse() * screen_position
            });
        let spawned = manager
            .call("gym_spawn", &[kind.to_variant(), world.to_variant()])
            .try_to::<bool>()
            .unwrap_or(false);
        if spawned {
            play_sfx("ui_confirm");
        }
        spawned
    }

    #[func]
    pub fn get_card_count(&self) -> i64 {
        self.cards.len() as i64
    }

    #[func]
    pub fn get_card(&self, index: i64) -> Option<Gd<Control>> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.cards.get(i))
            .map(|card| card.clone().upcast())
    }

    #[func]
    pub fn get_drop_zone(&self) -> Option<Gd<Control>> {
        self.drop_zone.clone().map(|zone| zone.upcast())
    }

    #[func]
    pub fn is_god_mode(&self) -> bool {
        self.god_mode
    }

    #[func]
    pub fn is_sidebar_visible(&self) -> bool {
        self.sidebar.as_ref().is_some_and(|s| s.is_visible())
    }

    #[func]
    pub fn toggle_sidebar(&mut self) {
        let show = !self.is_sidebar_visible();
        if let Some(mut sidebar) = self.sidebar.clone() {
            sidebar.set_visible(show);
            if show {
                super::slide_in(&sidebar.upcast(), Vector2::new(60.0, 0.0), 0.0, 0.18);
            }
        }
        if let Some(mut tab) = self.show_tab.clone() {
            tab.set_visible(!show);
        }
        if let Some(mut zone) = self.drop_zone.clone() {
            let right = if show { -SIDEBAR_WIDTH } else { 0.0 };
            zone.set_offset(godot::builtin::Side::RIGHT, right);
        }
    }

    /// Song label and player god mode need the level loaded and players spawned.
    #[func]
    fn _refresh_deferred(&mut self) {
        self.refresh_song();
        let on = self.god_mode;
        self.set_god_mode(on);
    }

    #[func]
    fn _on_clear(&mut self) {
        if let Some(mut manager) = self.manager.clone() {
            manager.call("gym_clear", &[]);
        }
    }

    #[func]
    pub fn set_god_mode(&mut self, on: bool) {
        self.god_mode = on;
        for mut player in self
            .base()
            .get_tree()
            .get_nodes_in_group(groups::PLAYERS)
            .iter_shared()
        {
            player.set("god_mode", &on.to_variant());
        }
        if let Some(mut button) = self.god_button.clone() {
            button.set_text(if on { "God: on" } else { "God: off" });
        }
    }

    #[func]
    fn _on_god(&mut self) {
        self.set_god_mode(!self.god_mode);
    }

    /// Loads the next catalog level's song.
    #[func]
    pub fn next_song(&mut self) {
        let Some(mut manager) = self.manager.clone() else {
            return;
        };
        let levels = all_levels();
        let current = self.current_level_id();
        let index = levels
            .iter()
            .position(|l| l.id == current)
            .map_or(0, |i| i + 1);
        let next = &levels[index % levels.len()];
        manager.call("gym_set_song", &[GString::from(&next.id).to_variant()]);
        self.refresh_song();
    }
}

impl GymPanel {
    fn current_level_id(&self) -> String {
        self.manager
            .as_ref()
            .and_then(|m| m.try_get_node_as::<crate::director::LevelDirector>("LevelDirector"))
            .map(|d| d.bind().get_level_id().to_string())
            .unwrap_or_default()
    }

    fn refresh_song(&mut self) {
        let id = self.current_level_id();
        let title = all_levels()
            .into_iter()
            .find(|l| l.id == id)
            .map_or(id, |l| l.title);
        if let Some(mut button) = self.song_button.clone() {
            button.set_text(&format!("Song: {title}"));
        }
    }

    fn tool_button(&self, text: &str, method: &str) -> Gd<Button> {
        let mut b = super::button(text);
        b.set_custom_minimum_size(Vector2::new(0.0, 34.0));
        if let Some(font) = super::font(FontKind::Narrow) {
            b.add_theme_font_override("font", &font);
        }
        b.add_theme_font_size_override("font_size", 15);
        b.set_h_size_flags(SizeFlags::EXPAND_FILL);
        b.set_clip_text(true);
        // The theme's menu buttons pad wide; three of these share one row.
        for state in ["normal", "hover", "pressed", "focus", "disabled"] {
            if let Some(style) = b.get_theme_stylebox(state) {
                let mut style = style.duplicate_resource();
                style.set_content_margin(godot::builtin::Side::LEFT, 8.0);
                style.set_content_margin(godot::builtin::Side::RIGHT, 8.0);
                b.add_theme_stylebox_override(state, &style);
            }
        }
        b.set_focus_mode(godot::classes::control::FocusMode::NONE);
        b.connect("pressed", &self.to_gd().callable(method));
        b
    }

    fn build(&mut self) {
        let mut zone = GymDropZone::new_alloc();
        zone.set_name("DropZone");
        zone.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        zone.set_offset(godot::builtin::Side::RIGHT, -SIDEBAR_WIDTH);
        zone.bind_mut().panel = Some(self.to_gd());
        self.base_mut().add_child(&zone);

        let mut sidebar = PanelContainer::new_alloc();
        sidebar.set_name("Sidebar");
        sidebar.set_anchors_and_offsets_preset(LayoutPreset::RIGHT_WIDE);
        sidebar.set_offset(godot::builtin::Side::LEFT, -SIDEBAR_WIDTH);
        let mut style = StyleBoxFlat::new_gd();
        style.set_bg_color(palette::VOID.with_alpha(0.88));
        style.set_border_width(godot::builtin::Side::LEFT, 2);
        style.set_border_color(palette::STEEL);
        style.set_content_margin_all(10.0);
        sidebar.add_theme_stylebox_override("panel", &style);
        self.base_mut().add_child(&sidebar);

        let mut column = VBoxContainer::new_alloc();
        column.add_theme_constant_override("separation", 8);
        sidebar.add_child(&column);

        column.add_child(&label("Enemy gym", FontKind::Display, 24, palette::TEXT));
        let mut help = label(
            "Drag an enemy into the arena",
            FontKind::Narrow,
            15,
            palette::MIST,
        );
        help.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD);
        column.add_child(&help);

        let mut tools = HBoxContainer::new_alloc();
        tools.add_theme_constant_override("separation", 6);
        let god = self.tool_button("God: off", "_on_god");
        tools.add_child(&self.tool_button("Clear", "_on_clear"));
        tools.add_child(&god);
        tools.add_child(&self.tool_button("Hide", "toggle_sidebar"));
        column.add_child(&tools);
        let song = self.tool_button("Song", "next_song");
        column.add_child(&song);

        let count = label("Enemies: 0", FontKind::Narrow, 15, palette::MIST);
        column.add_child(&count);

        let mut scroll = ScrollContainer::new_alloc();
        scroll.set_name("Cards");
        scroll.set_v_size_flags(SizeFlags::EXPAND_FILL);
        scroll.set_horizontal_scroll_mode(ScrollMode::DISABLED);
        column.add_child(&scroll);
        let mut list = VBoxContainer::new_alloc();
        list.add_theme_constant_override("separation", 6);
        list.set_h_size_flags(SizeFlags::EXPAND_FILL);
        scroll.add_child(&list);
        for (index, kind) in ENEMY_KINDS.iter().enumerate() {
            let mut card = GymCard::new_alloc();
            card.bind_mut().kind = index;
            card.set_name(kind.id);
            list.add_child(&card);
            self.cards.push(card);
        }

        let mut tab = super::button("Gym (H)");
        tab.set_name("ShowTab");
        tab.set_anchors_and_offsets_preset(LayoutPreset::TOP_RIGHT);
        tab.set_custom_minimum_size(Vector2::new(120.0, 36.0));
        tab.add_theme_font_size_override("font_size", 15);
        tab.set_offset(godot::builtin::Side::LEFT, -132.0);
        tab.set_offset(godot::builtin::Side::TOP, 96.0);
        tab.set_focus_mode(godot::classes::control::FocusMode::NONE);
        tab.set_visible(false);
        tab.connect("pressed", &self.to_gd().callable("toggle_sidebar"));
        self.base_mut().add_child(&tab);

        self.sidebar = Some(sidebar);
        self.drop_zone = Some(zone);
        self.show_tab = Some(tab);
        self.count_label = Some(count);
        self.god_button = Some(god);
        self.song_button = Some(song);
    }
}

#[godot_api]
impl ICanvasLayer for GymPanel {
    fn ready(&mut self) {
        // Above the HUD (10), below pause (20).
        self.base_mut().set_layer(12);
        self.manager = self.base().get_parent().and_then(|ui| ui.get_parent());
        // The debug overlay normally sits where the sidebar goes.
        if let Some(mut debug) = self
            .manager
            .as_ref()
            .and_then(|m| m.try_get_node_as::<Control>("DebugLabel"))
        {
            let at = debug.get_position() - Vector2::new(SIDEBAR_WIDTH, 0.0);
            debug.set_position(at);
        }
        self.build();
        self.base_mut().call_deferred("_refresh_deferred", &[]);
    }

    fn process(&mut self, _delta: f64) {
        let count = self
            .base()
            .get_tree()
            .get_nodes_in_group(groups::ENEMIES)
            .len();
        if let Some(mut label) = self.count_label.clone() {
            label.set_text(&format!("Enemies: {count}"));
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        let Ok(key) = event.try_cast::<InputEventKey>() else {
            return;
        };
        if key.is_pressed() && !key.is_echo() && key.get_keycode() == Key::H {
            self.toggle_sidebar();
            if let Some(mut viewport) = self.base().get_viewport() {
                viewport.set_input_as_handled();
            }
        }
    }
}

/// One enemy type in the sidebar; dragging it carries its `ENEMY_KINDS` index.
#[derive(GodotClass)]
#[class(init, base = PanelContainer)]
pub struct GymCard {
    pub kind: usize,
    style: Option<Gd<StyleBoxFlat>>,
    base: Base<PanelContainer>,
}

#[godot_api]
impl GymCard {
    #[func]
    fn _on_hover(&mut self, hovered: bool) {
        if let Some(mut style) = self.style.clone() {
            style.set_border_color(if hovered { AMBER } else { palette::STEEL });
        }
    }
}

/// The enemy's sprite through the in-game metal body shader.
fn enemy_icon(kind: usize, size: f32) -> Gd<TextureRect> {
    let mut icon = super::texture_rect(&ENEMY_KINDS[kind].sprite_path(), size);
    if let Ok(shader) = try_load::<Shader>("res://shaders/enemy_body.gdshader") {
        let mut material = ShaderMaterial::new_gd();
        material.set_shader(&shader);
        icon.set_material(&material);
    }
    icon
}

#[godot_api]
impl IPanelContainer for GymCard {
    fn ready(&mut self) {
        let kind = &ENEMY_KINDS[self.kind];
        self.base_mut().set_mouse_filter(MouseFilter::PASS);
        self.base_mut()
            .set_default_cursor_shape(godot::classes::control::CursorShape::DRAG);
        self.base_mut()
            .set_tooltip_text(&format!("{}: {}", kind.name, kind.rule));
        let mut style = StyleBoxFlat::new_gd();
        style.set_bg_color(palette::PANEL.with_alpha(0.92));
        style.set_border_width(godot::builtin::Side::LEFT, 3);
        style.set_border_color(palette::STEEL);
        style.set_content_margin_all(6.0);
        self.base_mut().add_theme_stylebox_override("panel", &style);
        self.style = Some(style);

        let mut row = HBoxContainer::new_alloc();
        row.add_theme_constant_override("separation", 10);
        row.set_mouse_filter(MouseFilter::IGNORE);
        row.add_child(&enemy_icon(self.kind, ICON_SIZE));
        let mut text = VBoxContainer::new_alloc();
        text.add_theme_constant_override("separation", 0);
        text.set_h_size_flags(SizeFlags::EXPAND_FILL);
        text.set_mouse_filter(MouseFilter::IGNORE);
        let mut name = label(kind.name, FontKind::Narrow, 18, palette::TEXT);
        name.set_clip_text(true);
        text.add_child(&name);
        let mut rule = label(kind.rule, FontKind::Narrow, 14, palette::MIST);
        rule.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD);
        text.add_child(&rule);
        row.add_child(&text);
        self.base_mut().add_child(&row);

        let this = self.to_gd();
        self.base_mut().connect(
            "mouse_entered",
            &this.callable("_on_hover").bind(&[true.to_variant()]),
        );
        self.base_mut().connect(
            "mouse_exited",
            &this.callable("_on_hover").bind(&[false.to_variant()]),
        );
    }

    fn get_drag_data(&mut self, _at_position: Vector2) -> Variant {
        // Preview centered on the cursor, about the enemy's in-game size.
        let mut preview = Control::new_alloc();
        let mut icon = enemy_icon(self.kind, 64.0);
        icon.set_position(Vector2::new(-32.0, -32.0));
        icon.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.8));
        preview.add_child(&icon);
        self.base_mut().set_drag_preview(&preview);
        play_sfx("ui_move");
        let mut data = VarDictionary::new();
        data.set(DRAG_KEY, self.kind as i64);
        data.to_variant()
    }
}

/// The arena area left of the sidebar: accepts dropped enemy cards and marks the drop
/// point while a card is dragged over it.
#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct GymDropZone {
    panel: Option<Gd<GymPanel>>,
    hover: Option<Vector2>,
    base: Base<Control>,
}

fn dragged_kind(data: &Variant) -> Option<i64> {
    data.try_to::<VarDictionary>()
        .ok()?
        .get(DRAG_KEY)?
        .try_to::<i64>()
        .ok()
}

#[godot_api]
impl IControl for GymDropZone {
    fn ready(&mut self) {
        self.base_mut().set_mouse_filter(MouseFilter::PASS);
    }

    fn process(&mut self, _delta: f64) {
        let dragging = self
            .base()
            .get_viewport()
            .is_some_and(|viewport| viewport.gui_is_dragging());
        let hover = if dragging {
            let mouse = self.base().get_local_mouse_position();
            let size = self.base().get_size();
            (mouse.x >= 0.0 && mouse.y >= 0.0 && mouse.x <= size.x && mouse.y <= size.y)
                .then_some(mouse)
        } else {
            None
        };
        if hover != self.hover {
            self.hover = hover;
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let Some(at) = self.hover else {
            return;
        };
        let color = AMBER.with_alpha(0.8);
        self.base_mut()
            .draw_arc_ex(at, 34.0, 0.0, std::f32::consts::TAU, 40, color)
            .width(2.0)
            .done();
        for (from, to) in [
            (Vector2::new(-46.0, 0.0), Vector2::new(-24.0, 0.0)),
            (Vector2::new(24.0, 0.0), Vector2::new(46.0, 0.0)),
            (Vector2::new(0.0, -46.0), Vector2::new(0.0, -24.0)),
            (Vector2::new(0.0, 24.0), Vector2::new(0.0, 46.0)),
        ] {
            self.base_mut()
                .draw_line_ex(at + from, at + to, color)
                .width(2.0)
                .done();
        }
    }

    fn can_drop_data(&self, _at_position: Vector2, data: Variant) -> bool {
        dragged_kind(&data).is_some()
    }

    fn drop_data(&mut self, at_position: Vector2, data: Variant) {
        let Some(kind) = dragged_kind(&data) else {
            return;
        };
        let screen = self.base().get_global_transform() * at_position;
        if let Some(mut panel) = self.panel.clone() {
            panel.bind_mut().drop_enemy(kind, screen);
        }
    }
}
