use crate::player::Player;
use crate::spawner::EnemySpawner;
use godot::classes::{
    Button, CanvasLayer, CenterContainer, CharacterBody2D, ColorRect, Control, INode2D, Label,
    Node2D, Os, PackedScene, ResourceLoader, SceneTree, VBoxContainer,
};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct GameManager {
    #[export]
    pub player_scene: Option<Gd<PackedScene>>,

    score: i32,
    game_time: f32,
    is_game_over: bool,
    viewport_rect: Rect2,

    base: Base<Node2D>,
}

#[godot_api]
impl GameManager {
    #[func]
    fn _on_enemy_died(&mut self) {
        self.score += 10;
    }

    #[func]
    fn _on_player_died(&mut self) {
        let mut alive_count = 0;
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group("players").iter_shared() {
            if let Ok(p) = node.try_cast::<Player>()
                && !p.bind().is_dead
            {
                alive_count += 1;
            }
        }

        if alive_count == 0 {
            self.game_over();
        }
    }

    fn game_over(&mut self) {
        self.is_game_over = true;
        let mut spawner = self.base().get_node_as::<EnemySpawner>("EnemySpawner");
        spawner.bind_mut().stop();
        self.show_game_over_screen();
    }

    fn show_game_over_screen(&mut self) {
        let mut canvas = CanvasLayer::new_alloc();
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

        let title =
            Self::game_over_label("GAME OVER", 64, Some(Color::from_rgba(1.0, 0.2, 0.2, 1.0)));
        vbox.add_child(&title);

        let score_lbl = Self::game_over_label(&format!("Score: {}", self.score), 32, None);
        vbox.add_child(&score_lbl);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 32.0));
        vbox.add_child(&spacer);

        let mut restart_btn = Self::scene_button("Restart", "on_restart", "res://main_level.tscn");
        vbox.add_child(&restart_btn);

        let menu_btn = Self::scene_button("Main Menu", "on_menu", "res://scenes/main_menu.tscn");
        vbox.add_child(&menu_btn);

        restart_btn.call_deferred("grab_focus", &[]);
    }

    fn game_over_label(text: &str, font_size: i32, font_color: Option<Color>) -> Gd<Label> {
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

        let mut spawn_index = 0;
        for cfg in players_cfg.iter_shared() {
            if let Some(scene) = &self.player_scene {
                let mut p = scene.instantiate_as::<CharacterBody2D>();
                p.set_position(spawn_positions[spawn_index % spawn_positions.len()]);
                spawn_index += 1;

                let color = cfg.get("color").try_to::<Color>().unwrap_or(Color::WHITE);
                let input_type = cfg.get("input_type").try_to::<i32>().unwrap_or(0);

                p.set("team_color", &color.to_variant());
                p.set("input_type", &input_type.to_variant());

                if let Some(actions) = Self::keyboard_actions(input_type) {
                    Self::set_keyboard_actions(&mut p, actions);
                }

                self.base_mut().add_child(&p);

                let manager_gd = self.to_gd();
                p.connect("died", &manager_gd.callable("_on_player_died"));
            }
        }
    }

    fn player_configs_or_default(&self) -> Array<Gd<RefCounted>> {
        let game_config_node = self.base().get_node_or_null("/root/GameConfig");
        let mut players_cfg = Array::<Gd<RefCounted>>::new();

        if let Some(game_config) = game_config_node.clone()
            && let Ok(p) = game_config.get("players").try_to::<Array<Gd<RefCounted>>>()
        {
            players_cfg = p;
        }

        if !players_cfg.is_empty() {
            return players_cfg;
        }

        let colors = [
            Color::from_rgb(0.35, 0.75, 1.0),
            Color::from_rgb(1.0, 0.6, 0.2),
        ];

        let p1 = crate::game_config::PlayerConfig::new_config(0, colors[0]); // Keyboard1
        let p2 = crate::game_config::PlayerConfig::new_config(1, colors[1]); // Keyboard2

        players_cfg.push(&p1.upcast::<RefCounted>());
        players_cfg.push(&p2.upcast::<RefCounted>());

        if let Some(mut game_config) = game_config_node {
            game_config.set("players", &players_cfg.to_variant());
        }

        players_cfg
    }

    fn spawn_positions(&self) -> [Vector2; 8] {
        let r = self.viewport_rect;
        [
            r.position + r.size * Vector2::new(0.333, 0.390),
            r.position + r.size * Vector2::new(0.333, 0.612),
            r.position + r.size * Vector2::new(0.667, 0.390),
            r.position + r.size * Vector2::new(0.667, 0.612),
            r.position + r.size * Vector2::new(0.500, 0.260),
            r.position + r.size * Vector2::new(0.500, 0.703),
            r.position + r.size * Vector2::new(0.167, 0.502),
            r.position + r.size * Vector2::new(0.833, 0.502),
        ]
    }

    fn keyboard_actions(
        input_type: i32,
    ) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
        match input_type {
            0 => Some(("p1_left", "p1_right", "p1_up", "p1_down")),
            1 => Some(("p2_left", "p2_right", "p2_up", "p2_down")),
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
        let mut score_label = self.base().get_node_as::<Label>("ScoreLabel");
        score_label.set_text(&format!("Score: {}", self.score));
        self.update_debug_label();
    }

    fn update_debug_label(&mut self) {
        if !Os::singleton().is_debug_build() {
            return;
        }
        let mut debug_label = self.base().get_node_as::<Label>("DebugLabel");
        let spawner = self.base().get_node_as::<EnemySpawner>("EnemySpawner");

        let mut lines = Vec::new();
        lines.push("─── DEBUG ───".to_string());
        lines.push(format!("time:       {:>6.1}s", self.game_time));
        lines.push(format!(
            "difficulty: {:>6.2}",
            spawner.bind().difficulty_factor
        ));
        lines.push("".to_string());
        lines.push("spawn intervals:".to_string());

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

        let mut spawner = self.base().get_node_as::<EnemySpawner>("EnemySpawner");
        let manager_gd = self.to_gd();
        spawner.connect("enemy_died", &manager_gd.callable("_on_enemy_died"));

        self.spawn_players();

        let mut debug_label = self.base().get_node_as::<Label>("DebugLabel");
        debug_label.set_visible(Os::singleton().is_debug_build());
    }

    fn process(&mut self, delta: f64) {
        if self.is_game_over {
            return;
        }
        self.game_time += delta as f32;
        let mut spawner = self.base().get_node_as::<EnemySpawner>("EnemySpawner");
        spawner.bind_mut().update_difficulty(self.game_time);
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
