use godot::prelude::*;

#[derive(GodotConvert, Var, Export, Clone, Copy, PartialEq, Eq)]
#[godot(via = i32)]
#[allow(dead_code)]
pub enum InputType {
    Keyboard1 = 0,
    Keyboard2 = 1,
    GamepadLeft0 = 2,
    GamepadLeft1 = 3,
    GamepadLeft2 = 4,
    GamepadLeft3 = 5,
    GamepadLeft4 = 6,
    GamepadLeft5 = 7,
    GamepadLeft6 = 8,
    GamepadLeft7 = 9,
    GamepadRight0 = 10,
    GamepadRight1 = 11,
    GamepadRight2 = 12,
    GamepadRight3 = 13,
    GamepadRight4 = 14,
    GamepadRight5 = 15,
    GamepadRight6 = 16,
    GamepadRight7 = 17,
}

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct PlayerConfig {
    #[var]
    pub input_type: i32,
    #[var]
    pub color: Color,

    base: Base<RefCounted>,
}

#[godot_api]
impl PlayerConfig {
    #[func]
    pub fn new_config(input_type: i32, color: Color) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            input_type,
            color,
            base,
        })
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct GameConfig {
    #[var]
    pub players: Array<Gd<PlayerConfig>>,

    base: Base<Node>,
}

#[godot_api]
impl INode for GameConfig {
    fn ready(&mut self) {
        // Optional initialization
    }
}

#[godot_api]
impl GameConfig {
    #[constant]
    pub const KEYBOARD1: i32 = 0;
    #[constant]
    pub const KEYBOARD2: i32 = 1;
    #[constant]
    pub const GAMEPAD_LEFT_0: i32 = 2;
    #[constant]
    pub const GAMEPAD_LEFT_1: i32 = 3;
    #[constant]
    pub const GAMEPAD_LEFT_2: i32 = 4;
    #[constant]
    pub const GAMEPAD_LEFT_3: i32 = 5;
    #[constant]
    pub const GAMEPAD_LEFT_4: i32 = 6;
    #[constant]
    pub const GAMEPAD_LEFT_5: i32 = 7;
    #[constant]
    pub const GAMEPAD_LEFT_6: i32 = 8;
    #[constant]
    pub const GAMEPAD_LEFT_7: i32 = 9;
    #[constant]
    pub const GAMEPAD_RIGHT_0: i32 = 10;
    #[constant]
    pub const GAMEPAD_RIGHT_1: i32 = 11;
    #[constant]
    pub const GAMEPAD_RIGHT_2: i32 = 12;
    #[constant]
    pub const GAMEPAD_RIGHT_3: i32 = 13;
    #[constant]
    pub const GAMEPAD_RIGHT_4: i32 = 14;
    #[constant]
    pub const GAMEPAD_RIGHT_5: i32 = 15;
    #[constant]
    pub const GAMEPAD_RIGHT_6: i32 = 16;
    #[constant]
    pub const GAMEPAD_RIGHT_7: i32 = 17;

    #[func]
    pub fn get_player_colors() -> Array<Color> {
        let mut arr = Array::new();
        arr.push(Color::from_rgb(0.35, 0.75, 1.0));
        arr.push(Color::from_rgb(1.0, 0.6, 0.2));
        arr.push(Color::from_rgb(0.4, 0.9, 0.3));
        arr.push(Color::from_rgb(0.9, 0.3, 0.9));
        arr.push(Color::from_rgb(1.0, 0.9, 0.15));
        arr.push(Color::from_rgb(0.9, 0.3, 0.3));
        arr.push(Color::from_rgb(0.4, 0.9, 0.85));
        arr.push(Color::from_rgb(1.0, 0.6, 0.75));
        arr
    }

    #[func]
    pub fn get_input_labels() -> Dictionary<i32, GString> {
        let mut dict = Dictionary::new();
        let _ = dict.insert(Self::KEYBOARD1, "KB: WASD/Arrows");
        let _ = dict.insert(Self::KEYBOARD2, "KB: IJKL");
        let _ = dict.insert(Self::GAMEPAD_LEFT_0, "Pad 1 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_LEFT_1, "Pad 2 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_LEFT_2, "Pad 3 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_LEFT_3, "Pad 4 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_LEFT_4, "Pad 5 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_LEFT_5, "Pad 6 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_LEFT_6, "Pad 7 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_LEFT_7, "Pad 8 Left Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_0, "Pad 1 Right Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_1, "Pad 2 Right Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_2, "Pad 3 Right Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_3, "Pad 4 Right Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_4, "Pad 5 Right Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_5, "Pad 6 Right Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_6, "Pad 7 Right Stick");
        let _ = dict.insert(Self::GAMEPAD_RIGHT_7, "Pad 8 Right Stick");
        dict
    }
}
