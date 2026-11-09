//! Enemies: one `BaseEnemy` root per scene, behavior from attached components.
//!
//! Every timed component is beat-locked: it reads the song beat from the level's
//! `Conductor` each frame and acts on its `Cadence` (`every_beats`, `offset_beats`)
//! after a `windup_beats` warning, so enemies stay on the music through frame drops,
//! pauses, seeks and checkpoint rewinds. Components that act emit
//! `acted(action_beat, song_beat)` and expose `get_windup()` (0..1 toward the next
//! action) and `get_cadence()`; movers expose `get_pose()` (lift, squash) for
//! `EnemyVisual`. Anything that hurts reports `component_danger_shapes()`, which the
//! root's `danger_shapes()` gathers for `DangerField`.
//!
//! - `health`: `HealthComponent` (life, colored shield layers, wards).
//! - `movers`: chase, turn, hop, bounce, dash and orbit movement.
//! - `shooters`: projectile attacks, rings, mines and the lance beam.
//! - `special`: split on death, color cycling, the warden's wards and fuses.
//! - `areas`: self-drawn area attacks (shockwave rings, spinning blades).
//! - `mine`: `Mine` and the inside-spawn `SpawnEffect`.

pub mod areas;
pub mod health;
pub mod mine;
pub mod movers;
pub mod shooters;
pub mod special;

use crate::conductor::Conductor;
use crate::core::beat_motion::{Cadence, CadenceTracker};
use crate::core::danger::{DangerShape, V2};
use crate::core::timing::Timing;
use crate::fx::{BurstStyle, with_fx};
use crate::groups;
use crate::hazards::{encode_shapes, to_v2};
use crate::visuals::enemy_visual::ENEMY_METAL;
use godot::classes::{
    Area2D, CircleShape2D, CollisionShape2D, IArea2D, IStaticBody2D, Node, Node2D, PackedScene,
    ResourceLoader, StaticBody2D,
};
use godot::prelude::*;

pub use health::HealthComponent;

const TAU: f32 = std::f32::consts::TAU;
/// Beats after spawning before a component's first action (on top of its wind-up).
pub const SPAWN_GRACE_BEATS: f64 = 1.0;
/// Inset (px) from the arena edge that moving enemies keep to.
pub const ARENA_MARGIN: f32 = 36.0;
/// Beat grid used when no `Conductor` is in the tree (standalone enemies in tests).
const FALLBACK_TIMING: Timing = Timing::new(120.0, 0.0);

/// Beats before leaving in which the enemy blinks.
pub const LEAVE_WARNING_BEATS: f64 = 2.0;

#[derive(GodotClass)]
#[class(init, base = StaticBody2D)]
pub struct BaseEnemy {
    /// Beats after spawning before the enemy leaves on its own, without counting as a
    /// kill (0 = stays until killed). Set from the chart's `SpawnEnemy` duration.
    #[var]
    lifetime_beats: f64,
    clock: EnemyClock,
    spawn_beat: Option<f64>,
    /// Contact-damage radius reported to `DangerField` (0 = no contact damage).
    contact_radius: f32,
    last_position: Vector2,
    last_beat: f64,
    velocity: Vector2,
    /// Children that report `component_danger_shapes()`.
    danger_parts: Vec<Gd<Node>>,
    base: Base<StaticBody2D>,
}

#[godot_api]
impl BaseEnemy {
    #[signal]
    fn died();
    /// Left on its own (lifetime over, or a fuse went off); not a kill.
    #[signal]
    fn left();

    /// Beats until the enemy leaves; negative when it has no lifetime.
    #[func]
    pub fn get_beats_left(&self) -> f64 {
        match self.spawn_beat {
            Some(spawn) if self.lifetime_beats > 0.0 => {
                spawn + self.lifetime_beats - self.clock_beat()
            }
            _ => -1.0,
        }
    }

    /// Leaves the arena: a metal puff, no `died` signal, no kill credit.
    #[func]
    pub fn leave(&mut self) {
        if self.base().is_queued_for_deletion() {
            return;
        }
        let position = self.base().get_global_position();
        with_fx(|fx| {
            fx.burst_style(position, ENEMY_METAL, 10, BurstStyle::Dots as i32, 0.7);
            fx.ring(position, ENEMY_METAL, 30.0, 0.25);
        });
        self.signals().left().emit();
        self.base_mut().queue_free();
    }

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

    /// Contact-damage circle with the enemy's measured velocity, plus every
    /// component's attack shapes (telegraphed ones with `activates_in > 0`).
    #[func]
    fn danger_shapes(&self) -> PackedFloat32Array {
        let mut records = if self.contact_radius > 0.0 {
            encode_shapes(&[DangerShape::Circle {
                center: to_v2(self.base().get_global_position()),
                radius: self.contact_radius,
                velocity: to_v2(self.velocity),
                activates_in: 0.0,
            }])
        } else {
            PackedFloat32Array::new()
        };
        for part in &self.danger_parts {
            if !part.is_instance_valid() {
                continue;
            }
            let mut part = part.clone();
            if let Ok(more) = part
                .call("component_danger_shapes", &[])
                .try_to::<PackedFloat32Array>()
            {
                records.extend_array(&more);
            }
        }
        records
    }

    /// How close a player can stand without being hurt: the contact radius, or a
    /// component's larger reach (a Hopper's shockwave). Bots hold this distance.
    #[func]
    fn get_threat_radius(&self) -> f32 {
        let mut radius = self.contact_radius;
        for part in &self.danger_parts {
            if part.is_instance_valid() && part.has_method("get_threat_radius") {
                let mut part = part.clone();
                let reach = part
                    .call("get_threat_radius", &[])
                    .try_to::<f32>()
                    .unwrap_or(0.0);
                radius = radius.max(reach);
            }
        }
        radius
    }

    fn clock_beat(&self) -> f64 {
        self.last_beat
    }

    fn on_died(&mut self) {
        self.signals().died().emit();
        self.base_mut().queue_free();
    }
}

#[godot_api]
impl IStaticBody2D for BaseEnemy {
    fn ready(&mut self) {
        self.base_mut().add_to_group(groups::ENEMIES);
        if let Some(contact) = self
            .base()
            .try_get_node_as::<Area2D>("ContactDamageComponent")
        {
            self.contact_radius = circle_radius(&contact.upcast()).unwrap_or(16.0);
        }
        self.danger_parts = self
            .base()
            .get_children()
            .iter_shared()
            .filter(|child| child.has_method("component_danger_shapes"))
            .collect();
        if self.contact_radius > 0.0 || !self.danger_parts.is_empty() {
            self.base_mut().add_to_group(groups::DANGER);
        }
        self.last_position = self.base().get_global_position();
    }

    fn physics_process(&mut self, delta: f64) {
        if self.lifetime_beats > 0.0 {
            let node = self.to_gd().upcast::<Node>();
            self.last_beat = self.clock.beat(&node, delta);
            let spawn = *self.spawn_beat.get_or_insert(self.last_beat);
            if self.last_beat - spawn >= self.lifetime_beats {
                self.leave();
                return;
            }
        }
        let position = self.base().get_global_position();
        if delta > 0.0 {
            self.velocity = (position - self.last_position) / delta as f32;
        }
        self.last_position = position;
    }
}

/// Radius of the first `CollisionShape2D` child with a circle shape.
pub fn circle_radius(node: &Gd<Node>) -> Option<f32> {
    node.get_children().iter_shared().find_map(|child| {
        child
            .try_cast::<CollisionShape2D>()
            .ok()?
            .get_shape()?
            .try_cast::<CircleShape2D>()
            .ok()
            .map(|circle| circle.get_radius())
    })
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

/// Song time for an enemy component: the level's `Conductor` when there is one,
/// otherwise a 120 BPM clock from accumulated frame deltas.
#[derive(Default)]
pub struct EnemyClock {
    conductor: Option<Gd<Conductor>>,
    searched: bool,
    fallback_seconds: f64,
}

impl EnemyClock {
    pub fn beat(&mut self, node: &Gd<Node>, delta: f64) -> f64 {
        if !self.searched && node.is_inside_tree() {
            self.searched = true;
            self.conductor = node
                .get_tree()
                .get_first_node_in_group(groups::CONDUCTOR)
                .and_then(|n| n.try_cast::<Conductor>().ok());
        }
        match &self.conductor {
            Some(conductor) if conductor.is_instance_valid() => conductor.bind().song_beat(),
            _ => {
                self.fallback_seconds += delta;
                FALLBACK_TIMING.seconds_to_beat(self.fallback_seconds)
            }
        }
    }

    pub fn timing(&self) -> Timing {
        match &self.conductor {
            Some(conductor) if conductor.is_instance_valid() => conductor.bind().timing(),
            _ => FALLBACK_TIMING,
        }
    }

    pub fn seconds_per_beat(&self) -> f64 {
        self.timing().seconds_per_beat()
    }
}

/// Clock plus cadence tracker: what every beat-locked component steps each frame.
#[derive(Default)]
pub struct BeatDriver {
    pub clock: EnemyClock,
    pub tracker: CadenceTracker,
    /// Song beat at the last `tick`.
    pub beat: f64,
}

impl BeatDriver {
    pub fn configure(&mut self, every: f64, offset: f64, windup: f64) {
        self.tracker = CadenceTracker::new(Cadence::new(every, offset), windup);
    }

    /// Reads the song beat; returns the action index due now, if any.
    pub fn tick(&mut self, node: &Gd<Node>, delta: f64) -> Option<i64> {
        self.beat = self.clock.beat(node, delta);
        self.tracker.update(self.beat, SPAWN_GRACE_BEATS)
    }

    pub fn windup(&self) -> f32 {
        self.tracker.windup_progress(self.beat)
    }

    pub fn in_windup(&self) -> bool {
        self.tracker.in_windup(self.beat)
    }

    pub fn action_beat(&self, index: i64) -> f64 {
        self.tracker.cadence.beat_of(index)
    }

    /// Seconds until the next action.
    pub fn seconds_until_next(&self) -> f32 {
        (self.tracker.beats_until_next(self.beat) * self.clock.seconds_per_beat()) as f32
    }

    pub fn beats_to_seconds(&self, beats: f64) -> f64 {
        beats * self.clock.seconds_per_beat()
    }

    /// `(every, offset, windup)` for tests and tools.
    pub fn cadence_vector(&self) -> Vector3 {
        Vector3::new(
            self.tracker.cadence.every as f32,
            self.tracker.cadence.offset as f32,
            self.tracker.windup as f32,
        )
    }
}

pub fn parent_as_node2d(parent: Option<Gd<Node>>) -> Option<Gd<Node2D>> {
    parent?.try_cast::<Node2D>().ok()
}

fn is_dead(node: &Gd<Node2D>) -> bool {
    node.get("is_dead").try_to::<bool>().unwrap_or(false)
}

/// Living members of `group` (players by default) as `Node2D`s.
pub fn living(tree: &Gd<SceneTree>, group: &StringName) -> Vec<Gd<Node2D>> {
    tree.get_nodes_in_group(group)
        .iter_shared()
        .filter_map(|node| node.try_cast::<Node2D>().ok())
        .filter(|node| !is_dead(node))
        .collect()
}

pub fn nearest_alive(
    tree: &Gd<SceneTree>,
    origin: Vector2,
    group: &StringName,
) -> Option<Gd<Node2D>> {
    living(tree, group).into_iter().min_by(|a, b| {
        origin
            .distance_to(a.get_global_position())
            .total_cmp(&origin.distance_to(b.get_global_position()))
    })
}

pub fn nearest_mismatched_target(
    tree: &Gd<SceneTree>,
    origin: Vector2,
    group: &StringName,
    shield_color: Color,
) -> Option<Gd<Node2D>> {
    living(tree, group)
        .into_iter()
        .filter(|node| {
            let team_color = node
                .get("team_color")
                .try_to::<Color>()
                .unwrap_or(Color::WHITE);
            !colors_match(team_color, shield_color)
        })
        .min_by(|a, b| {
            origin
                .distance_to(a.get_global_position())
                .total_cmp(&origin.distance_to(b.get_global_position()))
        })
}

/// Distinct team colors of the players in the tree, in group order.
pub fn player_colors(tree: &Gd<SceneTree>) -> Vec<Color> {
    let mut colors: Vec<Color> = Vec::new();
    for node in tree.get_nodes_in_group(groups::PLAYERS).iter_shared() {
        if let Ok(color) = node.get("team_color").try_to::<Color>()
            && !colors.iter().any(|c| colors_match(*c, color))
        {
            colors.push(color);
        }
    }
    colors
}

/// Positions of the other living enemies (for separation).
pub fn other_enemy_positions(tree: &Gd<SceneTree>, me: &Gd<Node2D>) -> Vec<V2> {
    let id = me.instance_id();
    tree.get_nodes_in_group(groups::ENEMIES)
        .iter_shared()
        .filter(|node| node.instance_id() != id)
        .filter_map(|node| node.try_cast::<Node2D>().ok())
        .map(|node| to_v2(node.get_global_position()))
        .collect()
}

/// Arena box (global px) moving enemies stay inside: the visible rect inset by
/// `ARENA_MARGIN`.
pub fn arena_bounds(node: &Gd<Node>) -> (V2, V2) {
    let rect = node
        .get_viewport()
        .map(|v| v.get_visible_rect())
        .filter(|r| r.size.x > 0.0 && r.size.y > 0.0)
        .unwrap_or(Rect2::new(Vector2::ZERO, Vector2::new(1280.0, 720.0)));
    (
        to_v2(rect.position + Vector2::splat(ARENA_MARGIN)),
        to_v2(rect.position + rect.size - Vector2::splat(ARENA_MARGIN)),
    )
}

pub fn clamp_to(p: V2, (lo, hi): (V2, V2)) -> V2 {
    V2::new(p.x.clamp(lo.x, hi.x), p.y.clamp(lo.y, hi.y))
}

pub fn lerp_angle(from: f32, to: f32, weight: f32) -> f32 {
    let difference = (to - from + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI;
    from + difference * weight
}

pub fn colors_match(c1: Color, c2: Color) -> bool {
    c1.r == c2.r && c1.g == c2.g && c1.b == c2.b
}

pub fn load_packed_scene(path: &str) -> Option<Gd<PackedScene>> {
    ResourceLoader::singleton()
        .load(path)
        .and_then(|resource| resource.try_cast::<PackedScene>().ok())
}

/// Instantiates an enemy projectile next to `enemy` (in the enemy's parent). `speed`
/// <= 0 keeps the scene's default.
pub fn spawn_projectile(
    scene: &Gd<PackedScene>,
    enemy: &Gd<Node2D>,
    origin: Vector2,
    direction: Vector2,
    speed: f32,
) -> Option<Gd<Node>> {
    // Find the container first: an unparented projectile would leak.
    let mut container = enemy.get_parent()?;
    let mut projectile = scene.try_instantiate_as::<Area2D>()?;
    projectile.set_global_position(origin);
    projectile.set("direction", &direction.to_variant());
    if speed > 0.0 {
        projectile.set("speed", &speed.to_variant());
    }

    let skin = StringName::from("projectile_skin");
    if enemy.has_meta(&skin) {
        projectile.set("skin", &enemy.get_meta(&skin));
    }

    let node = projectile.upcast::<Node>();
    container.add_child(&node);
    Some(node)
}

/// A unit vector of a `V2` as a Godot `Vector2`.
pub fn to_vector(v: V2) -> Vector2 {
    Vector2::new(v.x, v.y)
}
