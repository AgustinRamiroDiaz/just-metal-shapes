use crate::game_config::{GameConfig, PlayerConfig};
use godot::classes::{
    ColorRect, Control, HBoxContainer, IControl, Input, InputEvent, InputEventJoypadButton,
    InputEventJoypadMotion, InputEventKey, Label, VBoxContainer,
};
use godot::prelude::*;
use std::collections::HashMap;

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct DeviceSlot {
    #[var]
    pub display_name: GString,
    #[var]
    pub is_keyboard: bool,
    #[var]
    pub device_index: i32,
    #[var]
    pub joined: bool,
    #[var]
    pub split: bool,

    base: Base<RefCounted>,
}

#[godot_api]
impl DeviceSlot {
    pub fn input_types(&self) -> Vec<i32> {
        let mut result = Vec::new();
        if self.is_keyboard {
            if self.split {
                result.push(GameConfig::KEYBOARD1);
                result.push(GameConfig::KEYBOARD2);
            } else {
                result.push(GameConfig::KEYBOARD1);
            }
            return result;
        }

        let left = GameConfig::GAMEPAD_LEFT_0 + self.device_index;
        let right = GameConfig::GAMEPAD_RIGHT_0 + self.device_index;
        if self.split {
            result.push(left);
            result.push(right);
        } else {
            result.push(left);
        }
        result
    }
}

struct DeviceRow {
    dots: Gd<HBoxContainer>,
    status: Gd<Label>,
}

#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct MainMenu {
    device_slots: Vec<Gd<DeviceSlot>>,
    device_rows: Vec<DeviceRow>,
    hold_timers: HashMap<i32, f32>,
    last_stick_dir: HashMap<i32, i32>,

    base: Base<Control>,
}

const HOLD_TIME: f32 = 1.0;
const STICK_THRESHOLD: f32 = 0.5;
const STICK_RESET: f32 = 0.3;

#[godot_api]
impl MainMenu {
    #[func]
    fn _on_joy_connection_changed(&mut self, _device: i32, _connected: bool) {
        self.rebuild_devices();
    }

    fn rebuild_devices(&mut self) {
        let mut device_list = self
            .base()
            .get_node_as::<VBoxContainer>("CenterContainer/VBox/DeviceList");
        for mut child in device_list.get_children().iter_shared() {
            child.queue_free();
        }
        self.device_slots.clear();
        self.device_rows.clear();

        let kb = Gd::<DeviceSlot>::from_init_fn(|base| DeviceSlot {
            display_name: "Keyboard".into(),
            is_keyboard: true,
            device_index: 0,
            joined: false,
            split: false,
            base,
        });
        self.device_slots.push(kb);

        let input = Input::singleton();
        for idx in input.get_connected_joypads().iter_shared() {
            let gp = Gd::<DeviceSlot>::from_init_fn(|base| DeviceSlot {
                display_name: GString::from(&format!("Gamepad {}", idx + 1)),
                is_keyboard: false,
                device_index: idx as i32,
                joined: false,
                split: false,
                base,
            });
            self.device_slots.push(gp);
        }

        for slot in self.device_slots.iter() {
            let mut row = HBoxContainer::new_alloc();
            row.set_custom_minimum_size(Vector2::new(480.0, 44.0));

            let mut name_lbl = Label::new_alloc();
            name_lbl.set_custom_minimum_size(Vector2::new(140.0, 0.0));
            name_lbl.set_text(&slot.bind().display_name);

            let mut hint_lbl = Label::new_alloc();
            hint_lbl.set_custom_minimum_size(Vector2::new(200.0, 0.0));
            hint_lbl.set_text(&self.hint_for(slot.clone()));
            hint_lbl.set_modulate(Color::from_rgb(0.55, 0.55, 0.55));

            let mut dots_box = HBoxContainer::new_alloc();
            dots_box.set_custom_minimum_size(Vector2::new(52.0, 0.0));
            dots_box.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);
            dots_box.add_theme_constant_override("separation", 4);

            let mut status_lbl = Label::new_alloc();
            status_lbl.set_custom_minimum_size(Vector2::new(140.0, 0.0));
            status_lbl.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);

            row.add_child(&name_lbl);
            row.add_child(&hint_lbl);
            row.add_child(&dots_box);
            row.add_child(&status_lbl);
            device_list.add_child(&row);

            self.device_rows.push(DeviceRow {
                dots: dots_box,
                status: status_lbl,
            });
        }

        self.refresh();
    }

    fn hint_for(&self, slot: Gd<DeviceSlot>) -> String {
        if slot.bind().is_keyboard {
            "Enter / ← →  /  Hold Enter".to_string()
        } else {
            "A / D-Pad or L-Stick  /  Hold A".to_string()
        }
    }

    fn refresh(&mut self) {
        let mut any_joined = false;

        for i in 0..self.device_slots.len() {
            let slot = self.device_slots[i].clone();
            let row = &self.device_rows[i];
            let mut dots_box = row.dots.clone();
            let mut status_lbl = row.status.clone();

            for mut child in dots_box.get_children().iter_shared() {
                child.queue_free();
            }

            if slot.bind().joined {
                any_joined = true;
                for color in self.get_slot_colors(i as i32) {
                    let mut dot = ColorRect::new_alloc();
                    dot.set_custom_minimum_size(Vector2::new(20.0, 20.0));
                    dot.set_color(color);
                    dots_box.add_child(&dot);
                }
            }

            if let Some(hold_time) = self.hold_timers.get(&(i as i32)) {
                let progress = (hold_time / HOLD_TIME).min(1.0);
                let filled = (progress * 8.0).round() as usize;
                let text = format!("{}{}", "█".repeat(filled), "░".repeat(8 - filled));
                status_lbl.set_text(&text);
                status_lbl.set_modulate(Color::from_rgb(1.0, 0.88, 0.35));
            } else if !slot.bind().joined {
                status_lbl.set_text("— not joined —");
                status_lbl.set_modulate(Color::from_rgb(0.45, 0.45, 0.45));
            } else if slot.bind().split {
                status_lbl.set_text("◀ SPLIT (2P) ▶");
                status_lbl.set_modulate(Color::from_rgb(1.0, 0.88, 0.35));
            } else {
                status_lbl.set_text("◀ SINGLE ▶");
                status_lbl.set_modulate(Color::from_rgb(0.4, 0.95, 0.5));
            }
        }

        let mut hold_hint_label = self
            .base()
            .get_node_as::<Label>("CenterContainer/VBox/HoldHintLabel");
        if any_joined {
            hold_hint_label.set_modulate(Color::from_rgb(0.9, 0.9, 0.9));
        } else {
            hold_hint_label.set_modulate(Color::from_rgb(0.35, 0.35, 0.35));
        }
    }

    fn get_slot_colors(&self, slot_idx: i32) -> Vec<Color> {
        let mut color_index = 0;
        let player_colors = GameConfig::get_player_colors();
        for i in 0..self.device_slots.len() {
            let slot = self.device_slots[i].clone();
            if !slot.bind().joined {
                continue;
            }
            let types = slot.bind().input_types();
            if i as i32 == slot_idx {
                let mut result = Vec::new();
                for _ in 0..types.len() {
                    let color = player_colors
                        .get(color_index % player_colors.len())
                        .unwrap();
                    result.push(color);
                    color_index += 1;
                }
                return result;
            }
            color_index += types.len();
        }
        Vec::new()
    }

    fn start_game(&mut self) {
        let mut players_cfg = Array::<Gd<PlayerConfig>>::new();
        let mut color_index = 0;
        let player_colors = GameConfig::get_player_colors();

        for slot in &self.device_slots {
            if !slot.bind().joined {
                continue;
            }
            for input_type in slot.bind().input_types() {
                let color = player_colors
                    .get(color_index % player_colors.len())
                    .unwrap();
                let cfg = PlayerConfig::new_config(input_type, color);
                players_cfg.push(&cfg);
                color_index += 1;
            }
        }

        if let Some(mut game_config) = self.base().get_node_or_null("/root/GameConfig") {
            game_config.set("players", &players_cfg.to_variant());
        }

        let mut tree = self.base().get_tree();
        tree.change_scene_to_file("res://main_level.tscn");
    }

    fn handle_join_down(&mut self, slot_idx: i32) {
        if slot_idx == -1 {
            return;
        }
        let mut slot = self.device_slots[slot_idx as usize].clone();
        if !slot.bind().joined {
            slot.bind_mut().joined = true;
            self.refresh();
        } else {
            self.hold_timers.insert(slot_idx, 0.0);
        }
    }

    fn handle_join_up(&mut self, slot_idx: i32) {
        if slot_idx == -1 || !self.hold_timers.contains_key(&slot_idx) {
            return;
        }
        let held = self.hold_timers.get(&slot_idx).copied().unwrap();
        self.hold_timers.remove(&slot_idx);
        if held < HOLD_TIME {
            let mut slot = self.device_slots[slot_idx as usize].clone();
            slot.bind_mut().joined = false;
            slot.bind_mut().split = false;
        }
        self.refresh();
    }

    fn set_split(&mut self, slot_idx: i32, enable_split: bool) {
        if slot_idx == -1 {
            return;
        }
        let mut slot = self.device_slots[slot_idx as usize].clone();
        if !slot.bind().joined {
            return;
        }
        slot.bind_mut().split = enable_split;
        self.refresh();
    }

    fn keyboard_slot_index(&self) -> i32 {
        for i in 0..self.device_slots.len() {
            if self.device_slots[i].bind().is_keyboard {
                return i as i32;
            }
        }
        -1
    }

    fn gamepad_slot_index(&self, device_index: i32) -> i32 {
        for i in 0..self.device_slots.len() {
            let slot = self.device_slots[i].bind();
            if !slot.is_keyboard && slot.device_index == device_index {
                return i as i32;
            }
        }
        -1
    }
}

#[godot_api]
impl IControl for MainMenu {
    fn ready(&mut self) {
        if let Some(mut game_config) = self.base().get_node_or_null("/root/GameConfig") {
            game_config.set("players", &Array::<Gd<PlayerConfig>>::new().to_variant());
        }

        let mut input = Input::singleton();
        let menu_gd = self.to_gd();
        input.connect(
            "joy_connection_changed",
            &menu_gd.callable("_on_joy_connection_changed"),
        );

        self.rebuild_devices();
    }

    fn process(&mut self, delta: f64) {
        if self.hold_timers.is_empty() {
            return;
        }
        let mut should_start = false;
        for timer in self.hold_timers.values_mut() {
            *timer += delta as f32;
            if *timer >= HOLD_TIME {
                should_start = true;
            }
        }

        self.refresh();
        if should_start {
            self.hold_timers.clear();
            self.start_game();
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(key_event) = event.clone().try_cast::<InputEventKey>() {
            if !key_event.is_echo() {
                let idx = self.keyboard_slot_index();
                if key_event.is_pressed() {
                    match key_event.get_keycode() {
                        godot::global::Key::ENTER | godot::global::Key::KP_ENTER => {
                            self.handle_join_down(idx);
                        }
                        godot::global::Key::LEFT => {
                            self.set_split(idx, false);
                        }
                        godot::global::Key::RIGHT => {
                            self.set_split(idx, true);
                        }
                        _ => {}
                    }
                } else {
                    match key_event.get_keycode() {
                        godot::global::Key::ENTER | godot::global::Key::KP_ENTER => {
                            self.handle_join_up(idx);
                        }
                        _ => {}
                    }
                }
            }
        } else if let Ok(joy_button) = event.clone().try_cast::<InputEventJoypadButton>() {
            let idx = self.gamepad_slot_index(joy_button.get_device());
            if idx == -1 {
                return;
            }
            if joy_button.is_pressed() {
                match joy_button.get_button_index() {
                    godot::global::JoyButton::A | godot::global::JoyButton::START => {
                        self.handle_join_down(idx);
                    }
                    godot::global::JoyButton::DPAD_LEFT => {
                        self.set_split(idx, false);
                    }
                    godot::global::JoyButton::DPAD_RIGHT => {
                        self.set_split(idx, true);
                    }
                    _ => {}
                }
            } else {
                match joy_button.get_button_index() {
                    godot::global::JoyButton::A | godot::global::JoyButton::START => {
                        self.handle_join_up(idx);
                    }
                    _ => {}
                }
            }
        } else if let Ok(joy_motion) = event.clone().try_cast::<InputEventJoypadMotion>()
            && joy_motion.get_axis() == godot::global::JoyAxis::LEFT_X
        {
            let idx = self.gamepad_slot_index(joy_motion.get_device());
            if idx == -1 {
                return;
            }
            let slot_joined = self.device_slots[idx as usize].bind().joined;
            if !slot_joined {
                return;
            }

            let prev_dir = self
                .last_stick_dir
                .get(&joy_motion.get_device())
                .copied()
                .unwrap_or(0);
            let axis_value = joy_motion.get_axis_value();
            let new_dir = if axis_value > STICK_THRESHOLD {
                1
            } else if axis_value < -STICK_THRESHOLD {
                -1
            } else if axis_value.abs() < STICK_RESET {
                0
            } else {
                return;
            };

            if new_dir != prev_dir {
                self.last_stick_dir.insert(joy_motion.get_device(), new_dir);
                if new_dir == 1 {
                    self.set_split(idx, true);
                } else if new_dir == -1 {
                    self.set_split(idx, false);
                }
            }
        }
    }
}
