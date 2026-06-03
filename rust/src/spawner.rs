use crate::game_config::PlayerConfig;
use godot::classes::{GpuParticles2D, INode, Node, Node2D, PackedScene, ResourceLoader};
use godot::global::randf;
use godot::prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq, GodotConvert, Var, Export, Default)]
#[godot(via = i32)]
pub enum SpawnType {
    #[default]
    Inside = 0,
    Outside = 1,
}

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct EnemyConfig {
    #[var]
    pub scene: Option<Gd<PackedScene>>,
    #[var]
    pub spawn_type: SpawnType,
    #[var]
    pub interval_multiplier: f32,
    #[var]
    pub timer: f32,
    #[var]
    pub spawn_interval: f32,

    base: Base<RefCounted>,
}

#[godot_api]
impl EnemyConfig {
    #[func]
    pub fn new_config(
        scene: Gd<PackedScene>,
        spawn_type: SpawnType,
        interval_multiplier: f32,
    ) -> Gd<Self> {
        Gd::from_init_fn(|base| Self {
            scene: Some(scene),
            spawn_type,
            interval_multiplier,
            timer: 0.0,
            spawn_interval: 0.0,
            base,
        })
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct EnemySpawner {
    #[export]
    pub base_spawn_interval: f32,

    enemy_configs: Vec<Gd<EnemyConfig>>,
    pub difficulty_factor: f32,
    spawn_effect_scene: Option<Gd<PackedScene>>,
    viewport_rect: Rect2,
    active: bool,

    base: Base<Node>,
}

#[godot_api]
impl EnemySpawner {
    #[signal]
    fn enemy_died();

    #[func]
    pub fn stop(&mut self) {
        self.active = false;
    }

    #[func]
    pub fn update_difficulty(&mut self, game_time: f32) {
        self.difficulty_factor = 1.0 - (game_time / 300.0).min(0.7);
        for cfg in &mut self.enemy_configs {
            let multiplier = cfg.bind().interval_multiplier;
            cfg.bind_mut().spawn_interval =
                self.base_spawn_interval * multiplier * self.difficulty_factor;
        }
    }

    #[func]
    fn _place_enemy(&mut self, cfg: Gd<EnemyConfig>, spawn_pos: Vector2) {
        if !self.active {
            return;
        }
        let scene = cfg.bind().scene.clone();
        if let Some(scene) = scene {
            let mut enemy = scene.instantiate_as::<Node2D>();
            enemy.set_global_position(spawn_pos);
            if let Some(mut parent) = self.base().get_parent() {
                parent.add_child(&enemy);
            }

            let player_count = self.get_player_count();
            if player_count > 1
                && let Some(mut health) = enemy.get_node_or_null("HealthComponent")
            {
                let mut max_life = health.get("max_life").try_to::<f32>().unwrap_or(1.0);
                max_life *= player_count as f32;
                health.set("max_life", &max_life.to_variant());
                health.set("life", &max_life.to_variant());
            }

            let spawner_gd = self.to_gd();
            enemy.connect(
                "died",
                &Callable::from_fn("on_enemy_died", move |_args| {
                    let mut s = spawner_gd.clone();
                    s.emit_signal("enemy_died", &[]);
                    Variant::nil()
                }),
            );
        }
    }

    fn get_player_count(&self) -> i32 {
        if let Some(game_config) = self.base().get_node_or_null("/root/GameConfig")
            && let Ok(players) = game_config
                .get("players")
                .try_to::<Array<Gd<PlayerConfig>>>()
        {
            return players.len() as i32;
        }
        1
    }

    fn _spawn_enemy(&mut self, cfg: Gd<EnemyConfig>) {
        let spawn_type = cfg.bind().spawn_type;
        let spawn_pos = if spawn_type == SpawnType::Inside {
            self.get_spawn_inside_viewport()
        } else {
            self.get_spawn_on_circle()
        };

        if spawn_type == SpawnType::Inside {
            if let Some(scene) = &self.spawn_effect_scene {
                let mut effect = scene.instantiate_as::<GpuParticles2D>();
                effect.set_global_position(spawn_pos);
                if let Some(mut parent) = self.base().get_parent() {
                    parent.add_child(&effect);
                }

                let spawner_gd = self.to_gd();
                effect.connect(
                    "spawn_ready",
                    &Callable::from_fn("on_spawn_ready", move |_args| {
                        let mut s = spawner_gd.clone();
                        s.call("_place_enemy", &[cfg.to_variant(), spawn_pos.to_variant()]);
                        Variant::nil()
                    }),
                );
            }
        } else {
            self._place_enemy(cfg, spawn_pos);
        }
    }

    fn get_spawn_on_circle(&self) -> Vector2 {
        let center = self.viewport_rect.center();
        let angle = randf() as f32 * std::f32::consts::TAU;
        let radius = self.viewport_rect.size.length() / 2.0 + 50.0;
        center + Vector2::new(angle.cos(), angle.sin()) * radius
    }

    fn get_spawn_inside_viewport(&self) -> Vector2 {
        let margin = 50.0;
        Vector2::new(
            godot::global::randf_range(
                (self.viewport_rect.position.x + margin) as f64,
                (self.viewport_rect.end().x - margin) as f64,
            ) as f32,
            godot::global::randf_range(
                (self.viewport_rect.position.y + margin) as f64,
                (self.viewport_rect.end().y - margin) as f64,
            ) as f32,
        )
    }
}

#[godot_api]
impl INode for EnemySpawner {
    fn ready(&mut self) {
        if let Some(viewport) = self.base().get_viewport() {
            self.viewport_rect = viewport.get_visible_rect();
        }
        self.base_spawn_interval = 20.0;
        self.active = true;
        self.difficulty_factor = 1.0;

        let mut loader = ResourceLoader::singleton();
        self.spawn_effect_scene = loader
            .load("res://scenes/spawn_effect.tscn")
            .and_then(|r| r.try_cast::<PackedScene>().ok());

        let scenes = [
            (
                "res://scenes/static_shooter_enemy.tscn",
                SpawnType::Inside,
                1.0,
            ),
            ("res://scenes/shotgun_enemy.tscn", SpawnType::Outside, 1.5),
            ("res://scenes/turret_enemy.tscn", SpawnType::Inside, 2.0),
            ("res://scenes/runner_enemy.tscn", SpawnType::Outside, 1.2),
            (
                "res://scenes/mine_layer_enemy.tscn",
                SpawnType::Outside,
                2.5,
            ),
        ];

        for (path, spawn_type, multiplier) in scenes {
            if let Some(scene) = loader
                .load(path)
                .and_then(|r| r.try_cast::<PackedScene>().ok())
            {
                let mut cfg = EnemyConfig::new_config(scene, spawn_type, multiplier);
                cfg.bind_mut().timer =
                    godot::global::randf_range(0.0, self.base_spawn_interval as f64) as f32;
                cfg.bind_mut().spawn_interval = self.base_spawn_interval * multiplier;
                self.enemy_configs.push(cfg);
            }
        }
    }

    fn process(&mut self, delta: f64) {
        if !self.active {
            return;
        }

        let configs = self.enemy_configs.clone();
        for mut cfg in configs {
            let mut cfg_bind = cfg.bind_mut();
            cfg_bind.timer += delta as f32;
            if cfg_bind.timer >= cfg_bind.spawn_interval {
                cfg_bind.timer = 0.0;
                drop(cfg_bind);
                self._spawn_enemy(cfg);
            }
        }
    }
}
