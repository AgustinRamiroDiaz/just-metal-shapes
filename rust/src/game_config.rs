use crate::core::bot;
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
    /// `GameConfig.BOT_EASY` / `BOT_NORMAL` / `BOT_HARD`; only used by bots.
    #[var]
    #[init(val = GameConfig::BOT_NORMAL)]
    pub bot_skill: i32,
    /// Lobby/HUD label ("BOT 1", "P1"...). Empty for configs built without a name.
    #[var]
    pub display_name: GString,

    base: Base<RefCounted>,
}

#[godot_api]
impl PlayerConfig {
    #[func]
    pub fn new_config(input_type: i32, color: Color) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            input_type,
            color,
            bot_skill: GameConfig::BOT_NORMAL,
            display_name: GString::new(),
            base,
        })
    }

    #[func]
    pub fn new_bot(skill: i32, color: Color, display_name: GString) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            input_type: GameConfig::BOT,
            color,
            bot_skill: skill,
            display_name,
            base,
        })
    }

    #[func]
    pub fn is_bot(&self) -> bool {
        self.input_type == GameConfig::BOT
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct GameConfig {
    #[var]
    pub players: Array<Gd<PlayerConfig>>,
    /// `LevelCatalog` id the next level loads (empty: first level).
    #[var]
    pub selected_level_id: GString,
    /// `CASUAL`, `NORMAL` or `HARDCORE`.
    #[var]
    #[init(val = GameConfig::NORMAL)]
    pub difficulty_mode: i32,
    /// The next level runs as the enemy gym: endless, no chart, enemies placed by hand.
    #[var]
    pub gym: bool,

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

    #[constant]
    pub const CASUAL: i32 = 0;
    #[constant]
    pub const NORMAL: i32 = 1;
    #[constant]
    pub const HARDCORE: i32 = 2;
    /// Input type for AI-controlled players (`BotBrain`).
    #[constant]
    pub const BOT: i32 = 100;
    #[constant]
    pub const BOT_EASY: i32 = bot::SKILL_EASY;
    #[constant]
    pub const BOT_NORMAL: i32 = bot::SKILL_NORMAL;
    #[constant]
    pub const BOT_HARD: i32 = bot::SKILL_HARD;
    /// Seats in a level (humans + bots).
    #[constant]
    pub const MAX_PLAYERS: i32 = 8;

    /// Adds a bot with the first unused color, named "BOT n". False when full.
    #[func]
    pub fn add_bot(&mut self, skill: i32) -> bool {
        if self.players.len() >= Self::MAX_PLAYERS as usize {
            return false;
        }
        let skill = skill.clamp(Self::BOT_EASY, Self::BOT_HARD);
        let name = format!("BOT {}", self.bot_count() + 1);
        let config = PlayerConfig::new_bot(skill, self.next_free_color(), name.as_str().into());
        self.players.push(&config);
        true
    }

    /// Removes the most recently added bot. False when there is none.
    #[func]
    pub fn remove_last_bot(&mut self) -> bool {
        let last = (0..self.players.len())
            .rev()
            .find(|&i| self.players.at(i).bind().is_bot());
        match last {
            Some(index) => {
                self.players.remove(index);
                true
            }
            None => false,
        }
    }

    /// Adds a human seat for `input_type` with the first unused color, named "P n".
    /// False when full or that input already has a seat.
    #[func]
    pub fn add_human(&mut self, input_type: i32) -> bool {
        if input_type == Self::BOT
            || self.players.len() >= Self::MAX_PLAYERS as usize
            || self.find_human(input_type) >= 0
        {
            return false;
        }
        let config = PlayerConfig::new_config(input_type, self.next_free_color());
        let name = format!("P{}", self.human_count() + 1);
        config.clone().bind_mut().display_name = name.as_str().into();
        self.players.push(&config);
        true
    }

    /// Removes the human seat using `input_type`. False when there is none.
    #[func]
    pub fn remove_human(&mut self, input_type: i32) -> bool {
        let index = self.find_human(input_type);
        if index < 0 {
            return false;
        }
        self.players.remove(index as usize);
        true
    }

    /// Index in `players` of the human seat using `input_type`, or -1.
    #[func]
    pub fn find_human(&self, input_type: i32) -> i64 {
        if input_type == Self::BOT {
            return -1;
        }
        self.players
            .iter_shared()
            .position(|cfg| cfg.bind().input_type == input_type)
            .map_or(-1, |i| i as i64)
    }

    #[func]
    pub fn bot_count(&self) -> i64 {
        self.players
            .iter_shared()
            .filter(|cfg| cfg.bind().is_bot())
            .count() as i64
    }

    #[func]
    pub fn human_count(&self) -> i64 {
        self.players.len() as i64 - self.bot_count()
    }

    /// First color from `get_player_colors()` no seat uses yet (wraps when all are
    /// taken, which cannot happen within `MAX_PLAYERS`).
    #[func]
    pub fn next_free_color(&self) -> Color {
        let colors = Self::get_player_colors();
        let used: Vec<Color> = self.players.iter_shared().map(|c| c.bind().color).collect();
        colors
            .iter_shared()
            .find(|color| !used.contains(color))
            .unwrap_or_else(|| {
                colors
                    .get(self.players.len() % colors.len())
                    .unwrap_or(Color::WHITE)
            })
    }

    /// `easy`, `normal` or `hard`.
    #[func]
    pub fn bot_skill_name(skill: i32) -> GString {
        GString::from(bot::BotTuning::skill_name(skill))
    }

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
