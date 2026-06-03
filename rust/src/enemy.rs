use godot::classes::{
    Area2D, GpuParticles2D, IArea2D, IGpuParticles2D, INode, INode2D, IStaticBody2D, Node, Node2D,
    PackedScene, ResourceLoader, Sprite2D, StaticBody2D, Time, Timer,
};
use godot::global::randi_range;
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;

#[derive(GodotClass)]
#[class(init, base = StaticBody2D)]
struct BaseEnemy {
    base: Base<StaticBody2D>,
}

#[godot_api]
impl BaseEnemy {
    #[signal]
    fn died();

    #[func]
    fn take_damage(
        &mut self,
        amount: f32,
        #[opt(default = Color::WHITE)] damage_color: Color,
    ) -> bool {
        let mut health = self
            .base()
            .get_node_as::<HealthComponent>("HealthComponent");
        let did_damage = health.bind_mut().take_damage(amount, damage_color);
        let is_dead = health.bind().life <= 0.0;

        if did_damage && is_dead {
            self.on_died();
        }

        did_damage
    }

    fn on_died(&mut self) {
        self.signals().died().emit();
        self.base_mut().queue_free();
    }
}

#[godot_api]
impl IStaticBody2D for BaseEnemy {
    fn ready(&mut self) {}
}

#[derive(GodotClass)]
#[class(init, base = Node2D)]
struct HealthComponent {
    #[var]
    #[init(val = 3.0)]
    max_life: f32,

    #[var]
    shield_colors: PackedColorArray,

    #[var]
    auto_shield_layers: i32,

    #[var]
    life: f32,

    shield_fills: Vec<f32>,
    damage_flash_timer: f32,
    base: Base<Node2D>,
}

#[godot_api]
impl HealthComponent {
    #[signal]
    fn damaged(amount: f32);

    #[signal]
    fn died();

    #[func]
    fn get_active_layer(&self) -> i32 {
        self.shield_fills
            .iter()
            .position(|fill| *fill > 0.0)
            .map(|idx| idx as i32)
            .unwrap_or(-1)
    }

    #[func]
    fn get_active_color(&self) -> Color {
        let idx = self.get_active_layer();
        if idx >= 0 {
            self.shield_colors.get(idx as usize).unwrap_or(Color::WHITE)
        } else {
            Color::WHITE
        }
    }

    #[func]
    fn take_damage(
        &mut self,
        amount: f32,
        #[opt(default = Color::WHITE)] damage_color: Color,
    ) -> bool {
        if self.life <= 0.0 {
            return false;
        }

        let active = self.get_active_layer();
        if active >= 0 {
            let active_idx = active as usize;
            let active_color = self.shield_colors.get(active_idx).unwrap_or(Color::WHITE);
            if colors_match(active_color, damage_color) {
                self.shield_fills[active_idx] = (self.shield_fills[active_idx] - amount).max(0.0);
                self.damage_flash_timer = DAMAGE_FLASH_DURATION;
                self.signals().damaged().emit(amount);
                self.base_mut().queue_redraw();
                return true;
            }

            return false;
        }

        self.life = (self.life - amount).max(0.0);
        self.damage_flash_timer = DAMAGE_FLASH_DURATION;
        self.signals().damaged().emit(amount);
        self.base_mut().queue_redraw();

        if self.life == 0.0 {
            self.signals().died().emit();
        }

        true
    }
}

#[godot_api]
impl INode2D for HealthComponent {
    fn ready(&mut self) {
        self.life = self.max_life;
        self.init_shields();
        self.base_mut().queue_redraw();
    }

    fn process(&mut self, delta: f64) {
        if self.damage_flash_timer > 0.0 {
            self.damage_flash_timer -= delta as f32;
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let pulse =
            (Time::singleton().get_ticks_msec() as f32 / 1000.0 * PULSE_SPEED).sin() * 0.5 + 0.5;
        let base_alpha = 0.5 + pulse * GLOW_INTENSITY;

        self.base_mut()
            .draw_arc_ex(
                Vector2::ZERO,
                SHIELD_BASE_RADIUS,
                0.0,
                TAU,
                64,
                Color::from_rgba(0.2, 0.2, 0.2, base_alpha * 0.6),
            )
            .width(6.0)
            .done();

        let active = self.get_active_layer();

        for i in (0..self.shield_fills.len()).rev() {
            if self.shield_fills[i] <= 0.0 {
                continue;
            }

            let radius = self.shield_radius(i);
            let color = self.shield_colors.get(i).unwrap_or(Color::WHITE);

            let mut shield_alpha = 1.0;
            let mut shield_width = 6.0;
            if i as i32 == active && self.damage_flash_timer > 0.0 {
                let t = self.damage_flash_timer / DAMAGE_FLASH_DURATION;
                shield_alpha = 1.0 + t * 0.8;
                shield_width = 6.0 + t * 4.0;
            }

            let start = -std::f32::consts::FRAC_PI_2;
            let end = start + TAU * self.shield_fills[i];
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    radius,
                    start,
                    end,
                    64,
                    Color::from_rgba(color.r, color.g, color.b, base_alpha),
                )
                .width(6.0)
                .done();
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    radius,
                    start,
                    end,
                    64,
                    Color::from_rgba(color.r, color.g, color.b, shield_alpha),
                )
                .width(shield_width)
                .done();

            if i as i32 == active {
                self.base_mut()
                    .draw_arc_ex(
                        Vector2::ZERO,
                        radius + 4.0,
                        start,
                        end,
                        64,
                        Color::from_rgba(color.r, color.g, color.b, base_alpha * 0.3),
                    )
                    .width(2.0)
                    .done();
            }
        }

        let health_ratio = self.life / self.max_life.max(1.0);
        if health_ratio > 0.0 {
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    28.0,
                    -std::f32::consts::FRAC_PI_2,
                    -std::f32::consts::FRAC_PI_2 + TAU * health_ratio,
                    32,
                    Color::from_rgba(0.3, 0.8, 0.3, base_alpha),
                )
                .width(4.0)
                .done();
        }
    }
}

impl HealthComponent {
    fn init_shields(&mut self) {
        self.shield_fills.clear();

        if self.auto_shield_layers > 0 && self.shield_colors.is_empty() {
            let players = self.base().get_tree().get_nodes_in_group("players");
            if !players.is_empty() {
                let mut colors = PackedColorArray::new();
                for _ in 0..self.auto_shield_layers {
                    let idx = randi_range(0, players.len() as i64 - 1) as usize;
                    let color = players
                        .get(idx)
                        .and_then(|player| player.get("team_color").try_to::<Color>().ok())
                        .unwrap_or(Color::WHITE);
                    colors.push(color);
                }
                self.shield_colors = colors;
            }
        }

        self.shield_fills.resize(self.shield_colors.len(), 1.0);
    }

    fn shield_radius(&self, layer_idx: usize) -> f32 {
        let layer_count = self.shield_fills.len();
        SHIELD_BASE_RADIUS + (layer_count - 1 - layer_idx) as f32 * SHIELD_LAYER_SPACING
    }
}

const DAMAGE_FLASH_DURATION: f32 = 0.4;
const PULSE_SPEED: f32 = 4.0;
const GLOW_INTENSITY: f32 = 0.4;
const SHIELD_BASE_RADIUS: f32 = 34.0;
const SHIELD_LAYER_SPACING: f32 = 8.0;

#[derive(GodotClass)]
#[class(init, base = Area2D)]
struct Mine {
    armed: bool,
    base: Base<Area2D>,
}

#[godot_api]
impl IArea2D for Mine {
    fn ready(&mut self) {
        self.base_mut().set_collision_layer(0);
        self.base_mut().set_collision_mask(1);

        let mine = self.to_gd();
        self.base_mut()
            .signals()
            .body_entered()
            .connect_other(&mine, Self::on_body_entered);

        let mut arm_timer = Timer::new_alloc();
        arm_timer.set_wait_time(ARM_TIME as f64);
        arm_timer.set_one_shot(true);
        arm_timer
            .signals()
            .timeout()
            .connect_other(&mine, Self::arm);
        self.base_mut().add_child(&arm_timer);
        arm_timer.start();

        let mut lifetime_timer = Timer::new_alloc();
        lifetime_timer.set_wait_time(MINE_LIFETIME as f64);
        lifetime_timer.set_one_shot(true);
        lifetime_timer
            .signals()
            .timeout()
            .connect_other(&mine, |mine: &mut Mine| {
                mine.base_mut().queue_free();
            });
        self.base_mut().add_child(&lifetime_timer);
        lifetime_timer.start();

        self.base_mut().queue_redraw();
    }

    fn process(&mut self, _delta: f64) {
        if self.armed {
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let color = if self.armed {
            Color::from_rgba(1.0, 0.3, 0.1, 1.0)
        } else {
            Color::from_rgba(0.5, 0.5, 0.5, 0.6)
        };
        self.base_mut()
            .draw_circle(Vector2::ZERO, MINE_RADIUS, color);

        if self.armed {
            let pulse =
                (Time::singleton().get_ticks_msec() as f32 / 1000.0 * 6.0).sin() * 0.3 + 0.7;
            self.base_mut().draw_circle(
                Vector2::ZERO,
                MINE_RADIUS + 4.0,
                Color::from_rgba(1.0, 0.5, 0.1, pulse * 0.4),
            );
        }
    }
}

impl Mine {
    fn arm(&mut self) {
        self.armed = true;
        self.base_mut().queue_redraw();
    }

    fn on_body_entered(&mut self, mut body: Gd<Node2D>) {
        if !self.armed {
            return;
        }

        if body.has_method("take_damage") {
            body.call("take_damage", &[1.0f32.to_variant()]);
        }

        self.base_mut().queue_free();
    }
}

const ARM_TIME: f32 = 0.5;
const MINE_LIFETIME: f32 = 15.0;
const MINE_RADIUS: f32 = 10.0;

#[derive(GodotClass)]
#[class(init, base = GpuParticles2D)]
struct SpawnEffect {
    #[var]
    #[init(val = 0.8)]
    duration: f64,

    base: Base<GpuParticles2D>,
}

#[godot_api]
impl SpawnEffect {
    #[signal]
    fn spawn_ready();
}

#[godot_api]
impl IGpuParticles2D for SpawnEffect {
    fn ready(&mut self) {
        self.base_mut().set_emitting(true);

        let effect = self.to_gd();
        let mut ready_timer = Timer::new_alloc();
        ready_timer.set_wait_time(self.duration);
        ready_timer.set_one_shot(true);
        ready_timer
            .signals()
            .timeout()
            .connect_other(&effect, |effect: &mut SpawnEffect| {
                effect.signals().spawn_ready().emit();
                effect.base_mut().set_emitting(false);

                let mut free_timer = Timer::new_alloc();
                free_timer.set_wait_time(effect.base().get_lifetime());
                free_timer.set_one_shot(true);
                let effect_gd = effect.to_gd();
                free_timer.signals().timeout().connect_other(
                    &effect_gd,
                    |effect: &mut SpawnEffect| {
                        effect.base_mut().queue_free();
                    },
                );
                effect.base_mut().add_child(&free_timer);
                free_timer.start();
            });
        self.base_mut().add_child(&ready_timer);
        ready_timer.start();
    }
}

fn colors_match(c1: Color, c2: Color) -> bool {
    c1.r == c2.r && c1.g == c2.g && c1.b == c2.b
}

#[derive(GodotClass)]
#[class(init, base = Area2D)]
struct ContactDamageComponent {
    #[var]
    #[init(val = 1)]
    damage: i64,

    base: Base<Area2D>,
}

#[godot_api]
impl IArea2D for ContactDamageComponent {
    fn ready(&mut self) {
        self.base_mut().set_collision_layer(0);
        self.base_mut().set_collision_mask(1);

        let component = self.to_gd();
        self.base_mut()
            .signals()
            .body_entered()
            .connect_other(&component, Self::on_body_entered);
    }
}

impl ContactDamageComponent {
    fn on_body_entered(&mut self, mut body: Gd<Node2D>) {
        if body.has_method("take_damage") {
            body.call("take_damage", &[(self.damage as f32).to_variant()]);
        }
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct ChaserComponent {
    #[var]
    #[init(val = 30.0)]
    move_speed: f32,

    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    base: Base<Node>,
}

#[godot_api]
impl ChaserComponent {
    #[signal]
    fn moved(amount: f32);
}

#[godot_api]
impl INode for ChaserComponent {
    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };

        let origin = parent.get_global_position();
        let Some(nearest) = nearest_alive(&self.base().get_tree(), origin, &self.target_group)
        else {
            return;
        };

        let direction = (nearest.get_global_position() - origin).normalized_or_zero();
        let amount = self.move_speed * delta as f32;
        parent.set_global_position(origin + direction * amount);
        self.signals().moved().emit(amount);
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct ColorChaserComponent {
    #[var]
    #[init(val = 50.0)]
    move_speed: f32,

    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    base: Base<Node>,
}

#[godot_api]
impl INode for ColorChaserComponent {
    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };

        let origin = parent.get_global_position();
        let active_color = parent
            .try_get_node_as::<HealthComponent>("HealthComponent")
            .map(|health| health.bind().get_active_color())
            .unwrap_or(Color::WHITE);

        let Some(nearest) = nearest_mismatched_target(
            &self.base().get_tree(),
            origin,
            &self.target_group,
            active_color,
        ) else {
            return;
        };

        let direction = (nearest.get_global_position() - origin).normalized_or_zero();
        parent.set_global_position(origin + direction * self.move_speed * delta as f32);
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct TurnComponent {
    #[var]
    #[init(val = 0.5)]
    turn_speed: f32,

    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    #[var]
    #[init(val = NodePath::from("Sprite2D"))]
    sprite_path: NodePath,

    base: Base<Node>,
}

#[godot_api]
impl INode for TurnComponent {
    fn process(&mut self, delta: f64) {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };

        let origin = parent.get_global_position();
        let Some(nearest) = nearest_alive(&self.base().get_tree(), origin, &self.target_group)
        else {
            return;
        };

        let mut sprite = parent.get_node_as::<Sprite2D>(&self.sprite_path);
        let target_angle = (nearest.get_global_position() - origin).angle();
        let rotation = lerp_angle(
            sprite.get_rotation(),
            target_angle,
            self.turn_speed * delta as f32,
        );
        sprite.set_rotation(rotation);
    }
}

fn parent_as_node2d(parent: Option<Gd<Node>>) -> Option<Gd<Node2D>> {
    parent?.try_cast::<Node2D>().ok()
}

fn nearest_alive(tree: &Gd<SceneTree>, origin: Vector2, group: &StringName) -> Option<Gd<Node2D>> {
    let mut nearest = None;
    let mut min_dist = f32::INFINITY;

    let nodes = tree.get_nodes_in_group(group);
    for node in nodes.iter_shared() {
        let Ok(node2d) = node.try_cast::<Node2D>() else {
            continue;
        };
        if node2d.get("is_dead").try_to::<bool>().unwrap_or(false) {
            continue;
        }

        let distance = origin.distance_to(node2d.get_global_position());
        if distance < min_dist {
            min_dist = distance;
            nearest = Some(node2d);
        }
    }

    nearest
}

fn nearest_mismatched_target(
    tree: &Gd<SceneTree>,
    origin: Vector2,
    group: &StringName,
    shield_color: Color,
) -> Option<Gd<Node2D>> {
    let mut nearest = None;
    let mut min_dist = f32::INFINITY;

    let nodes = tree.get_nodes_in_group(group);
    for node in nodes.iter_shared() {
        let Ok(node2d) = node.try_cast::<Node2D>() else {
            continue;
        };
        if node2d.get("is_dead").try_to::<bool>().unwrap_or(false) {
            continue;
        }

        let team_color = node2d
            .get("team_color")
            .try_to::<Color>()
            .unwrap_or(Color::WHITE);
        if colors_match(team_color, shield_color) {
            continue;
        }

        let distance = origin.distance_to(node2d.get_global_position());
        if distance < min_dist {
            min_dist = distance;
            nearest = Some(node2d);
        }
    }

    nearest
}

fn lerp_angle(from: f32, to: f32, weight: f32) -> f32 {
    let difference = (to - from + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI;
    from + difference * weight
}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct ShooterComponent {
    #[var]
    #[init(val = 2.0)]
    shoot_interval: f64,

    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    projectile_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

#[godot_api]
impl ShooterComponent {
    #[signal]
    fn fired(projectile: Gd<Node>);
}

#[godot_api]
impl INode for ShooterComponent {
    fn ready(&mut self) {
        self.projectile_scene = load_packed_scene("res://scenes/projectile.tscn");
        let shooter = self.to_gd();
        let mut timer = Timer::new_alloc();
        timer.set_wait_time(self.shoot_interval);
        timer
            .signals()
            .timeout()
            .connect_other(&shooter, Self::shoot);
        self.base_mut().add_child(&timer);
        timer.start();
    }
}

impl ShooterComponent {
    fn shoot(&mut self) {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };

        let origin = parent.get_global_position();
        let Some(nearest) = nearest_alive(&self.base().get_tree(), origin, &self.target_group)
        else {
            return;
        };

        let direction = (nearest.get_global_position() - origin).normalized_or_zero();
        if let Some(projectile) = self.spawn_projectile(origin, direction) {
            self.signals().fired().emit(&projectile);
        }
    }

    fn spawn_projectile(&mut self, origin: Vector2, direction: Vector2) -> Option<Gd<Node>> {
        spawn_projectile(
            self.projectile_scene.as_ref()?,
            &self.base().get_tree(),
            origin,
            direction,
        )
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct ShotgunShooterComponent {
    #[var]
    #[init(val = 3.0)]
    shoot_interval: f64,

    #[var]
    #[init(val = 3)]
    shot_count: i32,

    #[var]
    #[init(val = 45.0)]
    spread_angle: f32,

    #[var]
    #[init(val = NodePath::from("Sprite2D"))]
    sprite_path: NodePath,

    projectile_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

#[godot_api]
impl ShotgunShooterComponent {
    #[signal]
    fn fired(projectile: Gd<Node>);
}

#[godot_api]
impl INode for ShotgunShooterComponent {
    fn ready(&mut self) {
        self.projectile_scene = load_packed_scene("res://scenes/projectile.tscn");
        let shooter = self.to_gd();
        let mut timer = Timer::new_alloc();
        timer.set_wait_time(self.shoot_interval);
        timer
            .signals()
            .timeout()
            .connect_other(&shooter, Self::shoot);
        self.base_mut().add_child(&timer);
        timer.start();
    }
}

impl ShotgunShooterComponent {
    fn shoot(&mut self) {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };

        let origin = parent.get_global_position();
        let sprite = parent.get_node_as::<Sprite2D>(&self.sprite_path);
        let base_direction = Vector2::RIGHT.rotated(sprite.get_rotation());
        let start_angle = -self.spread_angle / 2.0;
        let angle_step = if self.shot_count > 1 {
            self.spread_angle / (self.shot_count - 1) as f32
        } else {
            0.0
        };

        let Some(scene) = self.projectile_scene.clone() else {
            return;
        };
        for i in 0..self.shot_count {
            let angle = (start_angle + angle_step * i as f32).to_radians();
            if let Some(projectile) = spawn_projectile(
                &scene,
                &self.base().get_tree(),
                origin,
                base_direction.rotated(angle),
            ) {
                self.signals().fired().emit(&projectile);
            }
        }
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct TurretShooterComponent {
    #[var]
    #[init(val = 2.0)]
    shoot_interval: f64,

    phase: i32,
    projectile_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

#[godot_api]
impl TurretShooterComponent {
    #[signal]
    fn fired(projectile: Gd<Node>);
}

#[godot_api]
impl INode for TurretShooterComponent {
    fn ready(&mut self) {
        self.projectile_scene = load_packed_scene("res://scenes/projectile.tscn");
        let shooter = self.to_gd();
        let mut timer = Timer::new_alloc();
        timer.set_wait_time(self.shoot_interval);
        timer
            .signals()
            .timeout()
            .connect_other(&shooter, Self::shoot);
        self.base_mut().add_child(&timer);
        timer.start();
    }
}

impl TurretShooterComponent {
    fn shoot(&mut self) {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };

        let origin = parent.get_global_position();
        let directions = if self.phase == 0 {
            [Vector2::RIGHT, Vector2::LEFT, Vector2::UP, Vector2::DOWN]
        } else {
            [
                Vector2::new(1.0, 1.0).normalized(),
                Vector2::new(1.0, -1.0).normalized(),
                Vector2::new(-1.0, 1.0).normalized(),
                Vector2::new(-1.0, -1.0).normalized(),
            ]
        };

        let Some(scene) = self.projectile_scene.clone() else {
            return;
        };
        for direction in directions {
            if let Some(projectile) =
                spawn_projectile(&scene, &self.base().get_tree(), origin, direction)
            {
                self.signals().fired().emit(&projectile);
            }
        }

        self.phase = 1 - self.phase;
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
struct MineDropperComponent {
    #[var]
    #[init(val = 2.5)]
    drop_interval: f64,

    mine_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

#[godot_api]
impl INode for MineDropperComponent {
    fn ready(&mut self) {
        self.mine_scene = load_packed_scene("res://scenes/mine.tscn");
        let dropper = self.to_gd();
        let mut timer = Timer::new_alloc();
        timer.set_wait_time(self.drop_interval);
        timer
            .signals()
            .timeout()
            .connect_other(&dropper, Self::drop_mine);
        self.base_mut().add_child(&timer);
        timer.start();
    }
}

impl MineDropperComponent {
    fn drop_mine(&mut self) {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let Some(mut mine) = self
            .mine_scene
            .as_ref()
            .and_then(|scene| scene.try_instantiate_as::<Area2D>())
        else {
            return;
        };

        mine.set_global_position(parent.get_global_position());
        if let Some(mut container) = parent.get_parent() {
            container.add_child(&mine);
        }
    }
}

fn load_packed_scene(path: &str) -> Option<Gd<PackedScene>> {
    ResourceLoader::singleton()
        .load(path)
        .and_then(|resource| resource.try_cast::<PackedScene>().ok())
}

fn spawn_projectile(
    scene: &Gd<PackedScene>,
    tree: &Gd<SceneTree>,
    origin: Vector2,
    direction: Vector2,
) -> Option<Gd<Node>> {
    let mut projectile = scene.try_instantiate_as::<Area2D>()?;
    projectile.set_global_position(origin);
    projectile.set("direction", &direction.to_variant());

    let node = projectile.upcast::<Node>();
    tree.get_current_scene()?.add_child(&node);
    Some(node)
}
