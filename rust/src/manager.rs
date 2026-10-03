//! `GameManager`: runs one level = Conductor + LevelDirector + DangerField + players.
//!
//! Flow: spawn players -> load the selected level -> 3-2-1 countdown -> play the song.
//! The level is cleared when the Conductor emits `song_finished`. When every player is
//! down the song rewinds to the last checkpoint (players revived, arena cleared), or,
//! in hardcore mode, the run ends.

use crate::conductor::Conductor;
use crate::core::mode::DifficultyMode;
use crate::core::scoring::{self, RunStats};
use crate::director::{LevelDirector, current_mode};
use crate::game_config::{GameConfig, PlayerConfig};
use crate::groups;
use crate::player::Player;
use crate::util::dict_set;
use godot::classes::{
    Button, CanvasLayer, CenterContainer, CharacterBody2D, ColorRect, Control, INode2D, Label,
    Node2D, Os, PackedScene, ResourceLoader, SceneTree, VBoxContainer,
};
use godot::prelude::*;

const COUNTDOWN_SECONDS: f64 = 3.0;
/// How long "GO!" stays up after the countdown.
const GO_SECONDS: f64 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LevelState {
    Loading,
    Countdown,
    Playing,
    Cleared,
    GameOver,
}

impl LevelState {
    fn as_str(self) -> &'static str {
        match self {
            LevelState::Loading => "loading",
            LevelState::Countdown => "countdown",
            LevelState::Playing => "playing",
            LevelState::Cleared => "cleared",
            LevelState::GameOver => "game_over",
        }
    }
}

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct GameManager {
    #[export]
    pub player_scene: Option<Gd<PackedScene>>,
    /// Seconds of 3-2-1 before the song starts.
    #[export]
    #[init(val = COUNTDOWN_SECONDS)]
    pub countdown_seconds: f64,

    #[init(val = LevelState::Loading)]
    state: LevelState,
    countdown_left: f64,
    go_left: f64,
    stats: RunStats,
    mode: DifficultyMode,
    viewport_rect: Rect2,
    countdown_label: Option<Gd<Label>>,

    base: Base<Node2D>,
}

#[godot_api]
impl GameManager {
    /// Countdown number shown (3, 2, 1; 0 = "GO!").
    #[signal]
    fn countdown_tick(remaining: i64);
    #[signal]
    fn level_started();
    #[signal]
    fn level_cleared(score: i64);
    #[signal]
    fn game_over(score: i64);
    /// All players were down; the song rewound to `beat`. `count` is rewinds so far.
    #[signal]
    fn rewound(count: i64, beat: f64);

    /// `loading`, `countdown`, `playing`, `cleared` or `game_over`.
    #[func]
    pub fn get_state(&self) -> GString {
        GString::from(self.state.as_str())
    }

    #[func]
    pub fn is_game_over(&self) -> bool {
        self.state == LevelState::GameOver
    }

    #[func]
    pub fn is_level_clear(&self) -> bool {
        self.state == LevelState::Cleared
    }

    #[func]
    pub fn get_rewinds(&self) -> i64 {
        self.stats.rewinds as i64
    }

    #[func]
    pub fn get_score(&self) -> i64 {
        scoring::score(&self.current_stats())
    }

    /// Run stats for results screens. Keys: `score`, `rank`, `completed`,
    /// `seconds_survived`, `song_duration`, `players`, `hits_taken`, `downs`,
    /// `revives`, `rewinds`, `enemies_killed`, `difficulty`, `mode`.
    #[func]
    pub fn get_run_stats(&self) -> VarDictionary {
        let stats = self.current_stats();
        let mut dict = VarDictionary::new();
        dict_set(&mut dict, "score", scoring::score(&stats));
        dict_set(
            &mut dict,
            "rank",
            GString::from(scoring::rank(&stats).as_str()),
        );
        dict_set(&mut dict, "completed", stats.completed);
        dict_set(&mut dict, "seconds_survived", stats.seconds_survived);
        dict_set(&mut dict, "song_duration", stats.song_duration);
        dict_set(&mut dict, "players", stats.players as i64);
        dict_set(&mut dict, "hits_taken", stats.hits_taken as i64);
        dict_set(&mut dict, "downs", stats.downs as i64);
        dict_set(&mut dict, "revives", stats.revives as i64);
        dict_set(&mut dict, "rewinds", stats.rewinds as i64);
        dict_set(&mut dict, "enemies_killed", stats.enemies_killed as i64);
        dict_set(&mut dict, "difficulty", stats.difficulty as i64);
        dict_set(&mut dict, "mode", GString::from(stats.mode.as_str()));
        dict
    }

    /// Ends the countdown and starts the song now.
    #[func]
    pub fn skip_countdown(&mut self) {
        if self.state == LevelState::Countdown {
            self.countdown_left = 0.0;
            self.start_song();
        }
    }

    #[func]
    fn _on_enemy_died(&mut self) {
        self.stats.enemies_killed += 1;
    }

    #[func]
    fn _on_player_damaged(&mut self, lives_left: i32) {
        self.stats.hits_taken += 1;
        if lives_left <= 0 {
            self.stats.downs += 1;
        }
    }

    #[func]
    fn _on_player_revived(&mut self) {
        self.stats.revives += 1;
    }

    #[func]
    fn _on_player_died(&mut self) {
        // Deferred: the dying player is still mid-`take_damage` here.
        self.base_mut().call_deferred("_check_all_down", &[]);
    }

    #[func]
    fn _check_all_down(&mut self) {
        if self.state != LevelState::Playing || self.any_player_alive() {
            return;
        }
        if self.mode.allows_rewind() {
            self.rewind();
        } else {
            self.end_level(false);
        }
    }

    #[func]
    fn _on_song_finished(&mut self) {
        if self.state == LevelState::Playing {
            self.end_level(true);
        }
    }
}

impl GameManager {
    fn conductor(&self) -> Gd<Conductor> {
        self.base().get_node_as::<Conductor>("Conductor")
    }

    fn director(&self) -> Gd<LevelDirector> {
        self.base().get_node_as::<LevelDirector>("LevelDirector")
    }

    fn players(&self) -> Vec<Gd<Player>> {
        self.base()
            .get_tree()
            .get_nodes_in_group(groups::PLAYERS)
            .iter_shared()
            .filter_map(|node| node.try_cast::<Player>().ok())
            .collect()
    }

    fn any_player_alive(&self) -> bool {
        self.players().iter().any(|p| !p.bind().is_dead)
    }

    fn current_stats(&self) -> RunStats {
        let mut stats = self.stats.clone();
        let conductor = self.conductor();
        let conductor = conductor.bind();
        stats.seconds_survived = conductor.song_time();
        stats.song_duration = conductor.get_duration();
        stats
    }

    fn rewind(&mut self) {
        self.stats.rewinds += 1;
        let beat = self.director().bind_mut().rewind_to_checkpoint();
        for mut player in self.players() {
            player.bind_mut().respawn();
        }
        let count = self.stats.rewinds as i64;
        godot_print!("GameManager: all players down, rewind #{count} to beat {beat}");
        self.signals().rewound().emit(count, beat);
    }

    fn start_song(&mut self) {
        self.state = LevelState::Playing;
        self.go_left = GO_SECONDS;
        self.set_countdown_text("GO!");
        self.signals().countdown_tick().emit(0);
        self.conductor().bind_mut().play(0.0);
        self.signals().level_started().emit();
    }

    fn end_level(&mut self, cleared: bool) {
        self.state = if cleared {
            LevelState::Cleared
        } else {
            LevelState::GameOver
        };
        self.stats.completed = cleared;
        let mut director = self.director();
        director.bind_mut().active = false;
        director.bind_mut().clear_arena();
        if !cleared {
            self.conductor().bind_mut().stop();
        }
        let score = scoring::score(&self.current_stats());
        let rank = scoring::rank(&self.current_stats());
        if cleared {
            godot_print!(
                "GameManager: LEVEL CLEAR score={score} rank={}",
                rank.as_str()
            );
            self.signals().level_cleared().emit(score);
            self.show_end_screen("LEVEL CLEAR", Color::from_rgb(0.4, 1.0, 0.5), score);
        } else {
            godot_print!("GameManager: GAME OVER score={score}");
            self.signals().game_over().emit(score);
            self.show_end_screen("GAME OVER", Color::from_rgb(1.0, 0.2, 0.2), score);
        }
    }

    fn set_countdown_text(&mut self, text: &str) {
        if let Some(label) = self.countdown_label.as_mut() {
            label.set_text(text);
            label.set_visible(!text.is_empty());
        }
    }

    fn create_countdown_label(&mut self) {
        let mut canvas = CanvasLayer::new_alloc();
        canvas.set_name("CountdownLayer");
        self.base_mut().add_child(&canvas);
        let mut center = CenterContainer::new_alloc();
        center.set_anchors_and_offsets_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        center.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        canvas.add_child(&center);
        let label = Self::make_label("", 96, Some(Color::from_rgb(1.0, 0.95, 0.6)));
        center.add_child(&label);
        self.countdown_label = Some(label);
    }

    fn show_end_screen(&mut self, title: &str, color: Color, score: i64) {
        let mut canvas = CanvasLayer::new_alloc();
        canvas.set_name("EndScreen");
        self.base_mut().add_child(&canvas);

        let mut overlay = ColorRect::new_alloc();
        overlay.set_color(Color::from_rgba(0.0, 0.0, 0.0, 0.75));
        overlay.set_anchors_and_offsets_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        canvas.add_child(&overlay);

        let mut center = CenterContainer::new_alloc();
        center.set_anchors_and_offsets_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        canvas.add_child(&center);

        let mut vbox = VBoxContainer::new_alloc();
        vbox.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);
        center.add_child(&vbox);

        vbox.add_child(&Self::make_label(title, 64, Some(color)));
        vbox.add_child(&Self::make_label(&format!("Score: {score}"), 32, None));
        if self.stats.rewinds > 0 {
            let rewinds = format!("Rewinds: {}", self.stats.rewinds);
            vbox.add_child(&Self::make_label(&rewinds, 24, None));
        }

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 32.0));
        vbox.add_child(&spacer);

        let mut restart_btn = Self::scene_button("Restart", "on_restart", "res://main_level.tscn");
        vbox.add_child(&restart_btn);

        let menu_btn = Self::scene_button("Main Menu", "on_menu", "res://scenes/main_menu.tscn");
        vbox.add_child(&menu_btn);

        restart_btn.call_deferred("grab_focus", &[]);
    }

    fn make_label(text: &str, font_size: i32, font_color: Option<Color>) -> Gd<Label> {
        let mut label = Label::new_alloc();
        label.set_text(text);
        label.add_theme_font_size_override("font_size", font_size);
        label.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
        if let Some(color) = font_color {
            label.add_theme_color_override("font_color", color);
        }
        label
    }

    fn scene_button(
        text: &str,
        callback_name: &'static str,
        scene_path: &'static str,
    ) -> Gd<Button> {
        let mut button = Button::new_alloc();
        button.set_text(text);
        button.add_theme_font_size_override("font_size", 24);
        button.connect(
            "pressed",
            &Callable::from_fn(callback_name, move |_args| {
                change_scene_to_file(scene_path);
                Variant::nil()
            }),
        );
        button
    }

    fn spawn_players(&mut self) {
        let players_cfg = self.player_configs_or_default();
        let spawn_positions = self.spawn_positions();

        for (spawn_index, cfg) in players_cfg.iter_shared().enumerate() {
            let Some(scene) = &self.player_scene else {
                continue;
            };
            let mut p = scene.instantiate_as::<CharacterBody2D>();
            p.set_position(spawn_positions[spawn_index % spawn_positions.len()]);

            let cfg = cfg.bind();
            p.set("team_color", &cfg.color.to_variant());
            p.set("input_type", &cfg.input_type.to_variant());
            if let Some(actions) = Self::keyboard_actions(cfg.input_type) {
                Self::set_keyboard_actions(&mut p, actions);
            }

            self.base_mut().add_child(&p);

            let manager_gd = self.to_gd();
            p.connect("died", &manager_gd.callable("_on_player_died"));
            p.connect("damaged", &manager_gd.callable("_on_player_damaged"));
            p.connect("revived", &manager_gd.callable("_on_player_revived"));
        }
        self.stats.players = players_cfg.len() as u32;
    }

    fn player_configs_or_default(&self) -> Array<Gd<PlayerConfig>> {
        let game_config_node = self.base().get_node_or_null("/root/GameConfig");
        let mut players_cfg = Array::<Gd<PlayerConfig>>::new();

        if let Some(game_config) = game_config_node.clone()
            && let Ok(p) = game_config
                .get("players")
                .try_to::<Array<Gd<PlayerConfig>>>()
        {
            players_cfg = p;
        }

        if !players_cfg.is_empty() {
            return players_cfg;
        }

        let colors = GameConfig::get_player_colors();

        let p1 = PlayerConfig::new_config(GameConfig::KEYBOARD1, colors.get(0).unwrap());
        let p2 = PlayerConfig::new_config(GameConfig::KEYBOARD2, colors.get(1).unwrap());

        players_cfg.push(&p1);
        players_cfg.push(&p2);

        if let Some(mut game_config) = game_config_node {
            game_config.set("players", &players_cfg.to_variant());
        }

        players_cfg
    }

    fn spawn_positions(&self) -> [Vector2; 8] {
        let r = self.viewport_rect;
        [
            r.position + r.size * Vector2::new(0.40, 0.42),
            r.position + r.size * Vector2::new(0.40, 0.58),
            r.position + r.size * Vector2::new(0.60, 0.42),
            r.position + r.size * Vector2::new(0.60, 0.58),
            r.position + r.size * Vector2::new(0.50, 0.30),
            r.position + r.size * Vector2::new(0.50, 0.70),
            r.position + r.size * Vector2::new(0.30, 0.50),
            r.position + r.size * Vector2::new(0.70, 0.50),
        ]
    }

    fn keyboard_actions(
        input_type: i32,
    ) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
        match input_type {
            GameConfig::KEYBOARD1 => Some(("p1_left", "p1_right", "p1_up", "p1_down")),
            GameConfig::KEYBOARD2 => Some(("p2_left", "p2_right", "p2_up", "p2_down")),
            _ => None,
        }
    }

    fn set_keyboard_actions(player: &mut Gd<CharacterBody2D>, actions: (&str, &str, &str, &str)) {
        let (left, right, up, down) = actions;
        player.set("move_left_action", &StringName::from(left).to_variant());
        player.set("move_right_action", &StringName::from(right).to_variant());
        player.set("move_up_action", &StringName::from(up).to_variant());
        player.set("move_down_action", &StringName::from(down).to_variant());
    }

    fn update_ui(&mut self) {
        let score = self.get_score();
        let mut score_label = self.base().get_node_as::<Label>("ScoreLabel");
        score_label.set_text(&format!("Score: {score}"));
        self.update_debug_label();
    }

    fn update_debug_label(&mut self) {
        if !Os::singleton().is_debug_build() {
            return;
        }
        let conductor = self.conductor();
        let director = self.director();
        let (time, beat, section, audio) = {
            let c = conductor.bind();
            let beat = c.song_beat();
            (
                c.song_time(),
                beat,
                c.section_index_at(beat),
                c.is_using_audio_clock(),
            )
        };
        let (cursor, events) = {
            let d = director.bind();
            (d.get_cursor(), d.get_event_count())
        };
        let hazards = self
            .base()
            .get_tree()
            .get_nodes_in_group(groups::HAZARDS)
            .len();
        let lines = [
            "--- DEBUG ---".to_string(),
            format!("state:   {}", self.state.as_str()),
            format!(
                "time:    {time:>6.2}s ({})",
                if audio { "audio" } else { "clock" }
            ),
            format!("beat:    {beat:>6.2}"),
            format!("section: {section}"),
            format!("events:  {cursor}/{events}"),
            format!("hazards: {hazards}"),
            format!("rewinds: {}", self.stats.rewinds),
        ];
        let mut debug_label = self.base().get_node_as::<Label>("DebugLabel");
        debug_label.set_text(&lines.join("\n"));
    }
}

#[godot_api]
impl INode2D for GameManager {
    fn ready(&mut self) {
        if let Some(viewport) = self.base().get_viewport() {
            self.viewport_rect = viewport.get_visible_rect();
        }

        let mut loader = ResourceLoader::singleton();
        self.player_scene = loader
            .load("res://scenes/player.tscn")
            .and_then(|r| r.try_cast::<PackedScene>().ok());

        let manager_gd = self.to_gd();
        let mut director = self.director();
        director.connect("enemy_died", &manager_gd.callable("_on_enemy_died"));
        let mut conductor = self.conductor();
        conductor.connect("song_finished", &manager_gd.callable("_on_song_finished"));

        self.spawn_players();
        self.mode = current_mode(&self.to_gd().upcast());
        self.stats.mode = self.mode;

        let mut debug_label = self.base().get_node_as::<Label>("DebugLabel");
        debug_label.set_visible(Os::singleton().is_debug_build());
        self.create_countdown_label();

        if !director.bind_mut().load_level(GString::new()) {
            godot_error!("GameManager: level failed to load");
            self.state = LevelState::GameOver;
            self.show_end_screen("LEVEL FAILED TO LOAD", Color::from_rgb(1.0, 0.2, 0.2), 0);
            return;
        }
        self.stats.difficulty = director.bind().level().map_or(1, |level| level.difficulty);
        self.state = LevelState::Countdown;
        self.countdown_left = self.countdown_seconds;
        let shown = self.countdown_left.ceil() as i64;
        self.set_countdown_text(&shown.to_string());
        self.signals().countdown_tick().emit(shown);
    }

    fn process(&mut self, delta: f64) {
        match self.state {
            LevelState::Countdown => {
                let before = self.countdown_left.ceil() as i64;
                self.countdown_left -= delta;
                if self.countdown_left <= 0.0 {
                    self.start_song();
                } else {
                    let now = self.countdown_left.ceil() as i64;
                    if now != before {
                        self.set_countdown_text(&now.to_string());
                        self.signals().countdown_tick().emit(now);
                    }
                }
            }
            LevelState::Playing if self.go_left > 0.0 => {
                self.go_left -= delta;
                if self.go_left <= 0.0 {
                    self.set_countdown_text("");
                }
            }
            _ => {}
        }
        self.update_ui();
    }
}

fn change_scene_to_file(scene_path: &str) {
    let mut tree = godot::classes::Engine::singleton()
        .get_main_loop()
        .and_then(|l| l.try_cast::<SceneTree>().ok())
        .unwrap();
    tree.change_scene_to_file(scene_path);
}
