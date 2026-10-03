//! `Lobby`: devices join (keyboard and every gamepad, each optionally split into two
//! players), bots fill seats, and holding start launches the level.
//!
//! Keyboard: Enter join, Left/Right split, B add bot, Backspace remove bot, Esc leave
//! (or back when not joined), hold Space or Enter to start.
//! Gamepad: A join, D-pad/stick left/right split, X add bot, Y remove bot, B leave (or
//! back), hold Start or A to start. Any device may add bots, so a bots-only run can be
//! started from an unjoined keyboard or pad.

use super::backdrop::add_backdrop;
use super::widgets::{HoldRing, SHAPES, hint, hint_bar, shape_icon, tag};
use super::{FontKind, go_to, keyboard_glyph, label, pad_glyph, palette, play_sfx};
use crate::game_config::{GameConfig, PlayerConfig};
use crate::level_catalog::{find_level, rgb_to_color};
use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::{
    Control, HBoxContainer, IControl, Input, InputEvent, InputEventJoypadButton,
    InputEventJoypadMotion, InputEventKey, Label, PanelContainer, TextureRect, VBoxContainer,
};
use godot::global::{JoyAxis, JoyButton, Key};
use godot::prelude::*;

pub const MAX_SEATS: usize = 8;
pub const HOLD_SECONDS: f64 = 1.0;
const STICK_THRESHOLD: f32 = 0.5;
const STICK_RESET: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceKind {
    Keyboard,
    Pad(i32),
}

struct Device {
    kind: DeviceKind,
    joined: bool,
    split: bool,
    stick_dir: i32,
    status: Option<Gd<Label>>,
    swatches: Option<Gd<HBoxContainer>>,
}

impl Device {
    fn new(kind: DeviceKind) -> Self {
        Self {
            kind,
            joined: false,
            split: false,
            stick_dir: 0,
            status: None,
            swatches: None,
        }
    }

    fn input_types(&self) -> Vec<i32> {
        if !self.joined {
            return Vec::new();
        }
        let (first, second) = match self.kind {
            DeviceKind::Keyboard => (GameConfig::KEYBOARD1, GameConfig::KEYBOARD2),
            DeviceKind::Pad(index) => (
                GameConfig::GAMEPAD_LEFT_0 + index,
                GameConfig::GAMEPAD_RIGHT_0 + index,
            ),
        };
        if self.split {
            vec![first, second]
        } else {
            vec![first]
        }
    }

    fn name(&self) -> String {
        match self.kind {
            DeviceKind::Keyboard => "Keyboard".into(),
            DeviceKind::Pad(index) => format!("Gamepad {}", index + 1),
        }
    }
}

/// One seat in the strip (a joined player or bot).
#[derive(Clone, Debug, PartialEq)]
pub struct Seat {
    pub input_type: i32,
    pub name: String,
    pub detail: String,
}

/// Seat names: humans `P1`, `P2`, ... in seat order; bots `BOT 1`, `BOT 2`, ...
pub fn seat_names(input_types: &[i32]) -> Vec<String> {
    let mut humans = 0;
    let mut bots = 0;
    input_types
        .iter()
        .map(|t| {
            if *t == GameConfig::BOT {
                bots += 1;
                format!("BOT {bots}")
            } else {
                humans += 1;
                format!("P{humans}")
            }
        })
        .collect()
}

/// Which controls drive a seat, for the seat card.
pub fn input_detail(input_type: i32) -> String {
    match input_type {
        GameConfig::KEYBOARD1 => "WASD / Arrows".into(),
        GameConfig::KEYBOARD2 => "IJKL".into(),
        GameConfig::BOT => "AI".into(),
        t if (GameConfig::GAMEPAD_RIGHT_0..GameConfig::GAMEPAD_RIGHT_0 + 8).contains(&t) => {
            format!("Pad {} right", t - GameConfig::GAMEPAD_RIGHT_0 + 1)
        }
        t if (GameConfig::GAMEPAD_LEFT_0..GameConfig::GAMEPAD_LEFT_0 + 8).contains(&t) => {
            format!("Pad {} left", t - GameConfig::GAMEPAD_LEFT_0 + 1)
        }
        _ => String::new(),
    }
}

/// `PlayerConfig` for a bot seat. Kept separate so a shared `GameConfig` bot helper can
/// replace it without touching the lobby flow.
fn bot_config(color: Color) -> Gd<PlayerConfig> {
    PlayerConfig::new_config(GameConfig::BOT, color)
}

struct SeatCard {
    icon: Gd<TextureRect>,
    name: Gd<Label>,
    detail: Gd<Label>,
    root: Gd<PanelContainer>,
    filled: Option<i32>,
}

#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct Lobby {
    devices: Vec<Device>,
    bots: usize,
    /// Devices currently holding start (index into `devices`, or -1 for an unjoined
    /// keyboard's Space).
    holding: Vec<i64>,
    hold_time: f64,
    started: bool,
    seat_cards: Vec<SeatCard>,
    device_list: Option<Gd<VBoxContainer>>,
    bot_status: Option<Gd<Label>>,
    bot_swatches: Option<Gd<HBoxContainer>>,
    ring: Option<Gd<HoldRing>>,
    start_label: Option<Gd<Label>>,
    accent: Color,
    base: Base<Control>,
}

#[godot_api]
impl Lobby {
    /// Emitted with the seat count whenever seats change.
    #[signal]
    fn seats_changed(count: i64);

    #[func]
    fn _on_joy_connection_changed(&mut self, _device: i64, _connected: bool) {
        self.sync_devices();
        self.rebuild_device_rows();
        self.refresh();
    }

    /// Input types of every seat in order (tests).
    #[func]
    pub fn get_seat_input_types(&self) -> PackedInt32Array {
        PackedInt32Array::from(self.seat_types().as_slice())
    }

    #[func]
    pub fn get_seat_count(&self) -> i64 {
        self.seat_types().len() as i64
    }

    /// Hold-to-start progress `0..=1`.
    #[func]
    pub fn get_hold_progress(&self) -> f64 {
        (self.hold_time / HOLD_SECONDS).clamp(0.0, 1.0)
    }

    #[func]
    pub fn add_bot(&mut self) -> bool {
        if self.seat_types().len() >= MAX_SEATS {
            play_sfx("ui_error");
            return false;
        }
        self.bots += 1;
        play_sfx("bot_join");
        self.refresh();
        true
    }

    #[func]
    pub fn remove_bot(&mut self) -> bool {
        if self.bots == 0 {
            play_sfx("ui_error");
            return false;
        }
        self.bots -= 1;
        play_sfx("ui_leave");
        self.refresh();
        true
    }
}

impl Lobby {
    fn seat_types(&self) -> Vec<i32> {
        let mut types: Vec<i32> = self.devices.iter().flat_map(Device::input_types).collect();
        types.extend(std::iter::repeat_n(GameConfig::BOT, self.bots));
        types
    }

    fn colors() -> Vec<Color> {
        GameConfig::get_player_colors().iter_shared().collect()
    }

    fn sync_devices(&mut self) {
        if self.devices.is_empty() {
            self.devices.push(Device::new(DeviceKind::Keyboard));
        }
        let connected: Vec<i32> = Input::singleton()
            .get_connected_joypads()
            .iter_shared()
            .map(|i| i as i32)
            .collect();
        self.devices.retain(|d| match d.kind {
            DeviceKind::Keyboard => true,
            DeviceKind::Pad(index) => connected.contains(&index),
        });
        for index in connected {
            if !self
                .devices
                .iter()
                .any(|d| d.kind == DeviceKind::Pad(index))
            {
                self.devices.push(Device::new(DeviceKind::Pad(index)));
            }
        }
    }

    /// Seats from a previous lobby visit (`GameConfig.players`).
    fn restore_from_config(&mut self) {
        let Some(config) = self
            .base()
            .get_node_or_null("/root/GameConfig")
            .and_then(|n| n.try_cast::<GameConfig>().ok())
        else {
            return;
        };
        let players: Vec<i32> = config
            .bind()
            .players
            .iter_shared()
            .map(|p| p.bind().input_type)
            .collect();
        for input_type in players {
            let (kind, second) = match input_type {
                GameConfig::KEYBOARD1 => (DeviceKind::Keyboard, false),
                GameConfig::KEYBOARD2 => (DeviceKind::Keyboard, true),
                GameConfig::BOT => {
                    self.bots += 1;
                    continue;
                }
                t if t >= GameConfig::GAMEPAD_RIGHT_0 => {
                    (DeviceKind::Pad(t - GameConfig::GAMEPAD_RIGHT_0), true)
                }
                t if t >= GameConfig::GAMEPAD_LEFT_0 => {
                    (DeviceKind::Pad(t - GameConfig::GAMEPAD_LEFT_0), false)
                }
                _ => continue,
            };
            if let Some(device) = self.devices.iter_mut().find(|d| d.kind == kind) {
                device.joined = true;
                device.split |= second;
            }
        }
    }

    fn device_index(&self, kind: DeviceKind) -> Option<usize> {
        self.devices.iter().position(|d| d.kind == kind)
    }

    fn join(&mut self, index: usize) {
        if self.seat_types().len() >= MAX_SEATS {
            play_sfx("ui_error");
            return;
        }
        let device = &mut self.devices[index];
        device.joined = true;
        device.split = false;
        play_sfx("ui_join");
        self.refresh();
    }

    fn leave(&mut self, index: usize) {
        let device = &mut self.devices[index];
        device.joined = false;
        device.split = false;
        self.holding.retain(|h| *h != index as i64);
        play_sfx("ui_leave");
        self.refresh();
    }

    fn set_split(&mut self, index: usize, split: bool) {
        let device = &self.devices[index];
        if !device.joined || device.split == split {
            return;
        }
        if split && self.seat_types().len() >= MAX_SEATS {
            play_sfx("ui_error");
            return;
        }
        self.devices[index].split = split;
        play_sfx(if split { "ui_join" } else { "ui_leave" });
        self.refresh();
    }

    fn hold(&mut self, source: i64, held: bool) {
        if held {
            if !self.holding.contains(&source) {
                self.holding.push(source);
            }
        } else {
            self.holding.retain(|h| *h != source);
        }
    }

    fn back(&mut self) {
        play_sfx("ui_back");
        go_to(super::LEVEL_SELECT_SCENE);
    }

    fn start_game(&mut self) {
        let types = self.seat_types();
        if types.is_empty() || self.started {
            return;
        }
        self.started = true;
        let colors = Self::colors();
        let mut players = Array::<Gd<PlayerConfig>>::new();
        for (i, input_type) in types.iter().enumerate() {
            let color = colors[i % colors.len()];
            let cfg = if *input_type == GameConfig::BOT {
                bot_config(color)
            } else {
                PlayerConfig::new_config(*input_type, color)
            };
            players.push(&cfg);
        }
        if let Some(mut config) = self
            .base()
            .get_node_or_null("/root/GameConfig")
            .and_then(|n| n.try_cast::<GameConfig>().ok())
        {
            config.bind_mut().players = players;
        }
        play_sfx("countdown_go");
        go_to(super::LEVEL_SCENE);
    }

    fn handle_key(&mut self, key: Gd<InputEventKey>) {
        if key.is_echo() {
            return;
        }
        let pressed = key.is_pressed();
        let mut code = key.get_keycode();
        if code == Key::NONE {
            code = key.get_physical_keycode();
        }
        let Some(kb) = self.device_index(DeviceKind::Keyboard) else {
            return;
        };
        let joined = self.devices[kb].joined;
        match code {
            Key::ENTER | Key::KP_ENTER => {
                if pressed && !joined {
                    self.join(kb);
                } else if !pressed || joined {
                    self.hold(kb as i64, pressed);
                }
            }
            Key::SPACE => self.hold(-1, pressed),
            Key::LEFT | Key::A if pressed => self.set_split(kb, false),
            Key::RIGHT | Key::D if pressed => self.set_split(kb, true),
            Key::B if pressed => {
                self.add_bot();
            }
            Key::BACKSPACE if pressed => {
                self.remove_bot();
            }
            Key::ESCAPE if pressed => {
                if joined {
                    self.leave(kb);
                } else {
                    self.back();
                }
            }
            _ => {}
        }
    }

    fn handle_pad_button(&mut self, button: Gd<InputEventJoypadButton>) {
        let Some(index) = self.device_index(DeviceKind::Pad(button.get_device())) else {
            return;
        };
        let pressed = button.is_pressed();
        let joined = self.devices[index].joined;
        // Hold sources for pads are offset so A and Start count separately.
        let source = 100 + index as i64 * 2;
        match button.get_button_index() {
            JoyButton::A => {
                if pressed && !joined {
                    self.join(index);
                } else if !pressed || joined {
                    self.hold(source, pressed);
                }
            }
            JoyButton::START => self.hold(source + 1, pressed),
            JoyButton::DPAD_LEFT if pressed => self.set_split(index, false),
            JoyButton::DPAD_RIGHT if pressed => self.set_split(index, true),
            JoyButton::X if pressed => {
                self.add_bot();
            }
            JoyButton::Y if pressed => {
                self.remove_bot();
            }
            JoyButton::B if pressed => {
                if joined {
                    self.leave(index);
                } else {
                    self.back();
                }
            }
            _ => {}
        }
    }

    fn handle_pad_motion(&mut self, motion: Gd<InputEventJoypadMotion>) {
        if motion.get_axis() != JoyAxis::LEFT_X {
            return;
        }
        let Some(index) = self.device_index(DeviceKind::Pad(motion.get_device())) else {
            return;
        };
        let value = motion.get_axis_value();
        let dir = if value > STICK_THRESHOLD {
            1
        } else if value < -STICK_THRESHOLD {
            -1
        } else if value.abs() < STICK_RESET {
            0
        } else {
            return;
        };
        if dir != self.devices[index].stick_dir {
            self.devices[index].stick_dir = dir;
            if dir != 0 {
                self.set_split(index, dir > 0);
            }
        }
    }

    fn rebuild_device_rows(&mut self) {
        let Some(mut list) = self.device_list.clone() else {
            return;
        };
        for mut child in list.get_children().iter_shared() {
            child.queue_free();
        }
        for i in 0..self.devices.len() {
            let mut row = HBoxContainer::new_alloc();
            row.add_theme_constant_override("separation", 14);
            row.set_custom_minimum_size(Vector2::new(0.0, 46.0));
            row.set_mouse_filter(MouseFilter::IGNORE);
            let glyph = match self.devices[i].kind {
                DeviceKind::Keyboard => keyboard_glyph("arrows_all", 40.0),
                DeviceKind::Pad(_) => pad_glyph("dpad", 40.0),
            };
            row.add_child(&glyph);
            let mut name = label(&self.devices[i].name(), FontKind::Ui, 22, palette::TEXT);
            name.set_custom_minimum_size(Vector2::new(170.0, 0.0));
            name.set_v_size_flags(SizeFlags::SHRINK_CENTER);
            row.add_child(&name);
            let mut status = label("", FontKind::Narrow, 20, palette::MIST);
            status.set_custom_minimum_size(Vector2::new(230.0, 0.0));
            status.set_v_size_flags(SizeFlags::SHRINK_CENTER);
            row.add_child(&status);
            let mut swatches = HBoxContainer::new_alloc();
            swatches.add_theme_constant_override("separation", 6);
            swatches.set_v_size_flags(SizeFlags::SHRINK_CENTER);
            swatches.set_mouse_filter(MouseFilter::IGNORE);
            row.add_child(&swatches);
            list.add_child(&row);
            self.devices[i].status = Some(status);
            self.devices[i].swatches = Some(swatches);
        }
    }

    fn refresh(&mut self) {
        let types = self.seat_types();
        let names = seat_names(&types);
        let colors = Self::colors();
        // Seat index of each device's first player.
        let mut seat = 0;
        for device in &mut self.devices {
            let count = device.input_types().len();
            let status_text = match (device.joined, device.split, device.kind) {
                (false, _, DeviceKind::Keyboard) => "Press Enter to join".to_string(),
                (false, _, DeviceKind::Pad(_)) => "Press A to join".to_string(),
                (true, false, _) => "1 player, right to split".to_string(),
                (true, true, _) => "2 players, left to merge".to_string(),
            };
            if let Some(status) = device.status.as_mut() {
                status.set_text(&status_text);
                let color = if device.joined {
                    palette::TEXT
                } else {
                    palette::MIST
                };
                status.add_theme_color_override("font_color", color);
            }
            if let Some(swatches) = device.swatches.as_mut() {
                fill_swatches(swatches, &names[seat..seat + count], &colors, seat);
            }
            seat += count;
        }
        if let Some(status) = self.bot_status.as_mut() {
            status.set_text(&match self.bots {
                0 => "No bots".to_string(),
                1 => "1 bot".to_string(),
                n => format!("{n} bots"),
            });
        }
        if let Some(swatches) = self.bot_swatches.as_mut() {
            fill_swatches(swatches, &names[seat..], &colors, seat);
        }

        for (i, card) in self.seat_cards.iter_mut().enumerate() {
            let filled = types.get(i).copied();
            let was = card.filled;
            card.filled = filled;
            match filled {
                Some(input_type) => {
                    let color = colors[i % colors.len()];
                    card.icon.set_visible(true);
                    super::widgets::set_shape_color(&mut card.icon, color);
                    card.name.set_text(&names[i]);
                    card.name.add_theme_color_override("font_color", color);
                    card.detail.set_text(&input_detail(input_type));
                    card.root.set_modulate(Color::WHITE);
                    if was != filled {
                        pop(card.root.clone().upcast());
                    }
                }
                None => {
                    card.icon.set_visible(false);
                    card.name.set_text("Open");
                    card.name
                        .add_theme_color_override("font_color", palette::STEEL);
                    card.detail.set_text("");
                    card.root
                        .set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.55));
                }
            }
        }
        if let Some(label) = self.start_label.as_mut() {
            let text = if types.is_empty() {
                "Join or add a bot to start"
            } else {
                "Hold to start"
            };
            label.set_text(text);
        }
        let count = types.len() as i64;
        self.signals().seats_changed().emit(count);
    }

    fn build(&mut self) {
        let mut root = self.to_gd().upcast::<Control>();
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
        column.add_theme_constant_override("separation", 18);
        column.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&column);

        let level = self
            .base()
            .get_node_or_null("/root/GameConfig")
            .and_then(|n| n.try_cast::<GameConfig>().ok())
            .map(|c| {
                let c = c.bind();
                (c.selected_level_id.to_string(), c.difficulty_mode)
            });
        let (subtitle, accent) = match level
            .as_ref()
            .and_then(|(id, mode)| find_level(id).map(|spec| (spec, *mode)))
        {
            Some((spec, mode)) => (
                format!("{}, {} mode", spec.title, super::mode_name(mode)),
                rgb_to_color(spec.palette.accent),
            ),
            None => (
                "Up to 8 players, any mix of people and bots".into(),
                palette::ACCENT,
            ),
        };
        self.accent = accent;
        column.add_child(&super::widgets::screen_header("Lobby", &subtitle));

        // Seat strip.
        let mut seats = HBoxContainer::new_alloc();
        seats.add_theme_constant_override("separation", 10);
        seats.set_mouse_filter(MouseFilter::IGNORE);
        for i in 0..MAX_SEATS {
            let mut card = PanelContainer::new_alloc();
            card.set_custom_minimum_size(Vector2::new(124.0, 150.0));
            let mut card_style = godot::classes::StyleBoxFlat::new_gd();
            card_style.set_bg_color(palette::PANEL.with_alpha(0.85));
            card_style.set_border_width(godot::builtin::Side::TOP, 2);
            card_style.set_border_color(palette::STEEL);
            card_style.set_content_margin_all(6.0);
            card.add_theme_stylebox_override("panel", &card_style);
            card.set_mouse_filter(MouseFilter::IGNORE);
            let mut inner = VBoxContainer::new_alloc();
            inner.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);
            inner.add_theme_constant_override("separation", 4);
            inner.set_mouse_filter(MouseFilter::IGNORE);
            let mut icon = shape_icon(SHAPES[i % SHAPES.len()], palette::TEXT, 64.0);
            icon.set_h_size_flags(SizeFlags::SHRINK_CENTER);
            inner.add_child(&icon);
            let name = super::centered(label("Open", FontKind::Display, 22, palette::STEEL));
            inner.add_child(&name);
            let detail = super::centered(label("", FontKind::Narrow, 16, palette::MIST));
            inner.add_child(&detail);
            card.add_child(&inner);
            seats.add_child(&card);
            self.seat_cards.push(SeatCard {
                icon,
                name,
                detail,
                root: card,
                filled: None,
            });
        }
        column.add_child(&seats);

        // Devices and bots.
        let mut panel = PanelContainer::new_alloc();
        panel.set_mouse_filter(MouseFilter::IGNORE);
        let mut lists = VBoxContainer::new_alloc();
        lists.add_theme_constant_override("separation", 4);
        lists.set_mouse_filter(MouseFilter::IGNORE);
        let device_list = VBoxContainer::new_alloc();
        lists.add_child(&device_list);
        self.device_list = Some(device_list);

        let mut bot_row = HBoxContainer::new_alloc();
        bot_row.add_theme_constant_override("separation", 14);
        bot_row.set_custom_minimum_size(Vector2::new(0.0, 46.0));
        bot_row.set_mouse_filter(MouseFilter::IGNORE);
        let mut bot_tag = tag("AI", palette::MIST, palette::VOID, 18);
        bot_tag.set_custom_minimum_size(Vector2::new(40.0, 0.0));
        bot_tag.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        bot_row.add_child(&bot_tag);
        let mut bot_name = label("Bots", FontKind::Ui, 22, palette::TEXT);
        bot_name.set_custom_minimum_size(Vector2::new(170.0, 0.0));
        bot_name.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        bot_row.add_child(&bot_name);
        let mut bot_status = label("", FontKind::Narrow, 20, palette::MIST);
        bot_status.set_custom_minimum_size(Vector2::new(230.0, 0.0));
        bot_status.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        bot_row.add_child(&bot_status);
        let mut bot_swatches = HBoxContainer::new_alloc();
        bot_swatches.add_theme_constant_override("separation", 6);
        bot_swatches.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        bot_row.add_child(&bot_swatches);
        lists.add_child(&bot_row);
        self.bot_status = Some(bot_status);
        self.bot_swatches = Some(bot_swatches);
        panel.add_child(&lists);
        column.add_child(&panel);

        // Footer: controls and the start ring.
        let glyph = 28.0;
        let mut bar = hint_bar(vec![
            hint(
                &[
                    keyboard_glyph("enter", glyph),
                    pad_glyph("button_color_a", glyph),
                ],
                "Join",
            ),
            hint(
                &[keyboard_glyph("arrows", glyph), pad_glyph("dpad", glyph)],
                "Split",
            ),
            hint(
                &[
                    keyboard_glyph("b", glyph),
                    pad_glyph("button_color_x", glyph),
                ],
                "Add bot",
            ),
            hint(
                &[
                    keyboard_glyph("backspace", glyph),
                    pad_glyph("button_color_y", glyph),
                ],
                "Remove bot",
            ),
            hint(
                &[
                    keyboard_glyph("escape", glyph),
                    pad_glyph("button_color_b", glyph),
                ],
                "Leave",
            ),
        ]);
        bar.add_theme_constant_override("separation", 26);
        root.add_child(&bar);

        let mut start = HBoxContainer::new_alloc();
        start.add_theme_constant_override("separation", 12);
        start.set_alignment(godot::classes::box_container::AlignmentMode::END);
        start.set_mouse_filter(MouseFilter::IGNORE);
        let mut start_label = label("Hold to start", FontKind::Display, 30, palette::TEXT);
        start_label.set_v_size_flags(SizeFlags::SHRINK_CENTER);
        start.add_child(&start_label);
        start.add_child(&keyboard_glyph("space", 48.0));
        start.add_child(&pad_glyph("button_start", 40.0));
        let mut ring = HoldRing::new_alloc();
        ring.set_custom_minimum_size(Vector2::new(64.0, 64.0));
        ring.bind_mut().color = self.accent;
        start.add_child(&ring);
        column.add_child(&start);
        self.ring = Some(ring);
        self.start_label = Some(start_label);
    }
}

fn fill_swatches(
    box_: &mut Gd<HBoxContainer>,
    names: &[String],
    colors: &[Color],
    first_seat: usize,
) {
    for mut child in box_.get_children().iter_shared() {
        child.queue_free();
    }
    for (offset, name) in names.iter().enumerate() {
        let color = colors[(first_seat + offset) % colors.len()];
        let swatch = tag(name, color, palette::VOID, 16);
        box_.add_child(&swatch);
    }
}

fn pop(control: Gd<Control>) {
    let Some(mut tween) = super::make_tween(&control.clone().upcast()) else {
        return;
    };
    let mut control = control;
    let size = control.get_size();
    control.set_pivot_offset(size * 0.5);
    control.set_scale(Vector2::new(0.6, 0.6));
    tween
        .tween_property(&control, "scale", &Vector2::ONE.to_variant(), 0.25)
        .set_trans(godot::classes::tween::TransitionType::BACK)
        .set_ease(godot::classes::tween::EaseType::OUT);
}

#[godot_api]
impl IControl for Lobby {
    fn ready(&mut self) {
        let mut root = self.to_gd().upcast::<Control>();
        super::full_rect(&mut root);
        add_backdrop(&mut root);
        self.build();
        if let Some(mut ui) = super::services() {
            let accent = self.accent;
            ui.bind_mut().set_accent(accent);
        }
        self.sync_devices();
        self.restore_from_config();
        self.rebuild_device_rows();
        self.refresh();

        let this = self.to_gd();
        Input::singleton().connect(
            "joy_connection_changed",
            &this.callable("_on_joy_connection_changed"),
        );
    }

    fn exit_tree(&mut self) {
        let this = self.to_gd();
        let callable = this.callable("_on_joy_connection_changed");
        let mut input = Input::singleton();
        if input.is_connected("joy_connection_changed", &callable) {
            input.disconnect("joy_connection_changed", &callable);
        }
    }

    fn process(&mut self, delta: f64) {
        if self.started {
            return;
        }
        let seats = self.seat_types().len();
        if self.holding.is_empty() {
            self.hold_time = (self.hold_time - delta * 3.0).max(0.0);
        } else if seats == 0 {
            if self.hold_time == 0.0 {
                play_sfx("ui_error");
            }
            self.hold_time = 0.001;
        } else {
            self.hold_time += delta;
            if self.hold_time >= HOLD_SECONDS {
                self.start_game();
            }
        }
        let progress = self.get_hold_progress();
        if let Some(mut ring) = self.ring.clone() {
            ring.bind_mut().progress = progress;
            ring.queue_redraw();
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if self.started || super::transitioning() {
            return;
        }
        if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
            self.handle_key(key);
        } else if let Ok(button) = event.clone().try_cast::<InputEventJoypadButton>() {
            self.handle_pad_button(button);
        } else if let Ok(motion) = event.try_cast::<InputEventJoypadMotion>() {
            self.handle_pad_motion(motion);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_count_humans_and_bots_separately() {
        let types = [
            GameConfig::KEYBOARD1,
            GameConfig::BOT,
            GameConfig::GAMEPAD_LEFT_0,
            GameConfig::BOT,
        ];
        assert_eq!(seat_names(&types), ["P1", "BOT 1", "P2", "BOT 2"]);
    }

    #[test]
    fn input_details() {
        assert_eq!(input_detail(GameConfig::GAMEPAD_RIGHT_0 + 2), "Pad 3 right");
        assert_eq!(input_detail(GameConfig::GAMEPAD_LEFT_0), "Pad 1 left");
        assert_eq!(input_detail(GameConfig::BOT), "AI");
    }
}
