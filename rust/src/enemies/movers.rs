//! Beat-driven movement components.
//!
//! - `ChaserComponent` / `ColorChaserComponent`: surge toward a player on every beat,
//!   steering with inertia along an arc, swaying, and keeping apart from other enemies.
//! - `TurnComponent`: turns the sprite pivot toward the nearest player with lag.
//! - `HopComponent`: crouches, hops toward the nearest player and lands on the beat,
//!   optionally with a shockwave.
//! - `BounceComponent`: steps one diagonal cell per beat, bouncing off the arena edges.
//! - `DashComponent`: telegraphs a line for its wind-up, then dashes along it.
//! - `OrbitComponent`: circles the arena center at a fixed radius, a fraction of a turn
//!   per beat.

use super::{
    BeatDriver, arena_bounds, clamp_to, lerp_angle, nearest_alive, nearest_mismatched_target,
    other_enemy_positions, parent_as_node2d, to_vector,
};
use crate::core::beat_motion::{
    HopShape, arc_heading, bounce_position, ease_out_cubic, separation, steer, steps_at, surge,
};
use crate::core::danger::{DangerShape, V2};
use crate::fx::{BurstStyle, with_fx};
use crate::hazards::{encode_shapes, hit_players, to_v2};
use crate::visuals::enemy_visual::ENEMY_GLOW;
use godot::classes::{INode, INode2D, Node, Node2D, Sprite2D};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;
/// Neighbors closer than this push a moving enemy away.
const SEPARATION_RADIUS: f32 = 90.0;
/// Distance at which chasers approach on their full arc.
const ARC_FULL_AT: f32 = 320.0;

/// Shared chase state: beat surges, inertia, arc and sway.
#[derive(Default)]
struct ChaseMotion {
    driver: BeatDriver,
    velocity: V2,
    /// +1 or -1: which side this chaser arcs around.
    arc_sign: f32,
    sway_seed: f32,
}

impl ChaseMotion {
    fn init(&mut self, node: &Gd<Node>) {
        self.driver.configure(1.0, 0.0, 0.0);
        let id = node.instance_id().to_i64();
        self.arc_sign = if id % 2 == 0 { 1.0 } else { -1.0 };
        self.sway_seed = (id % 97) as f32 * 0.37;
    }

    /// Moves `body` toward `target` for one frame; returns the surge index that
    /// started this frame.
    #[allow(clippy::too_many_arguments)]
    fn step(
        &mut self,
        node: &Gd<Node>,
        body: &mut Gd<Node2D>,
        target: Option<Vector2>,
        speed: f32,
        accel: f32,
        arc: f32,
        delta: f64,
    ) -> Option<i64> {
        let fired = self.driver.tick(node, delta);
        let dt = delta as f32;
        let origin = to_v2(body.get_global_position());
        let push = surge(self.driver.beat);
        let desired = match target {
            Some(target) => {
                let to_target = to_v2(target) - origin;
                let heading = arc_heading(to_target, arc * self.arc_sign, ARC_FULL_AT);
                let side = V2::new(-heading.y, heading.x);
                let sway = ((self.driver.beat as f32 * std::f32::consts::PI) + self.sway_seed)
                    .sin()
                    * 0.25;
                let apart = separation(
                    origin,
                    &other_enemy_positions(&body.get_tree(), body),
                    SEPARATION_RADIUS,
                );
                (heading + side * sway + apart * 1.5).normalized_or_zero() * (speed * push)
            }
            None => V2::ZERO,
        };
        self.velocity = steer(self.velocity, desired, accel * push.max(1.0), dt);
        body.set_global_position(to_vector(origin + self.velocity * dt));
        fired
    }

    /// Stretch along the travel direction while surging.
    fn pose(&self, speed: f32) -> Vector2 {
        let ratio = self.velocity.length() / (speed * 3.0).max(1.0);
        Vector2::new(0.0, 1.0 + 0.22 * ratio.clamp(0.0, 1.0))
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct ChaserComponent {
    /// Average speed (px/s); each beat surges above it, then glides below it.
    #[var]
    #[init(val = 30.0)]
    move_speed: f32,

    /// Steering acceleration (px/s^2): lower turns wider.
    #[var]
    #[init(val = 260.0)]
    accel: f32,

    /// Approach arc (radians) at range; straightens up close.
    #[var]
    #[init(val = 0.55)]
    arc: f32,

    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    motion: ChaseMotion,
    base: Base<Node>,
}

#[godot_api]
impl ChaserComponent {
    /// A beat surge started.
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.motion.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        0.0
    }

    #[func]
    fn get_pose(&self) -> Vector2 {
        self.motion.pose(self.move_speed)
    }
}

#[godot_api]
impl INode for ChaserComponent {
    fn ready(&mut self) {
        let node = self.to_gd().upcast::<Node>();
        self.motion.init(&node);
    }

    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let origin = parent.get_global_position();
        let target = nearest_alive(&self.base().get_tree(), origin, &self.target_group)
            .map(|t| t.get_global_position());
        let node = self.to_gd().upcast::<Node>();
        let (speed, accel, arc) = (self.move_speed, self.accel, self.arc);
        if let Some(index) = self
            .motion
            .step(&node, &mut parent, target, speed, accel, arc, delta)
        {
            let beat = self.motion.driver.beat;
            let action = self.motion.driver.action_beat(index);
            self.signals().acted().emit(action, beat);
        }
    }
}

/// Chases the nearest player whose color does not match its active shield, so the
/// "wrong" player must dodge while the matching one closes in.
#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct ColorChaserComponent {
    #[var]
    #[init(val = 50.0)]
    move_speed: f32,

    #[var]
    #[init(val = 320.0)]
    accel: f32,

    #[var]
    #[init(val = 0.45)]
    arc: f32,

    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    motion: ChaseMotion,
    base: Base<Node>,
}

#[godot_api]
impl ColorChaserComponent {
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.motion.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        0.0
    }

    #[func]
    fn get_pose(&self) -> Vector2 {
        self.motion.pose(self.move_speed)
    }
}

#[godot_api]
impl INode for ColorChaserComponent {
    fn ready(&mut self) {
        let node = self.to_gd().upcast::<Node>();
        self.motion.init(&node);
    }

    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let origin = parent.get_global_position();
        let active_color = parent
            .try_get_node_as::<super::HealthComponent>("HealthComponent")
            .map(|health| health.bind().get_active_color())
            .unwrap_or(Color::WHITE);
        let target = nearest_mismatched_target(
            &self.base().get_tree(),
            origin,
            &self.target_group,
            active_color,
        )
        .map(|t| t.get_global_position());
        let node = self.to_gd().upcast::<Node>();
        let (speed, accel, arc) = (self.move_speed, self.accel, self.arc);
        if let Some(index) = self
            .motion
            .step(&node, &mut parent, target, speed, accel, arc, delta)
        {
            let beat = self.motion.driver.beat;
            let action = self.motion.driver.action_beat(index);
            self.signals().acted().emit(action, beat);
        }
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct TurnComponent {
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

        let Some(mut sprite) = parent.try_get_node_as::<Sprite2D>(&self.sprite_path) else {
            return;
        };
        let target_angle = (nearest.get_global_position() - origin).angle();
        let rotation = lerp_angle(
            sprite.get_rotation(),
            target_angle,
            (self.turn_speed * delta as f32).min(1.0),
        );
        sprite.set_rotation(rotation);
    }
}

/// One planned hop: from, to, and the landing beat (action index).
#[derive(Clone, Copy)]
struct Hop {
    from: V2,
    to: V2,
    index: i64,
    landing: f64,
}

/// Hops toward the nearest player, landing on every `every_beats`-th beat. With a
/// `shockwave_radius`, each landing damages players inside that circle for
/// `SHOCK_BEATS`.
#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct HopComponent {
    #[var]
    #[init(val = 2.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    /// Longest hop (px).
    #[var]
    #[init(val = 120.0)]
    hop_distance: f32,
    /// Peak height of the hop drawn by `EnemyVisual` (px).
    #[var]
    #[init(val = 22.0)]
    hop_height: f32,
    /// Landing shockwave radius (px); 0 = none.
    #[var]
    #[init(val = 52.0)]
    shockwave_radius: f32,
    /// Hops stop this far short of the target (px), so the body lands beside a
    /// player who stands still and the shockwave does the hurting.
    #[var]
    #[init(val = 48.0)]
    standoff: f32,
    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    driver: BeatDriver,
    shape: HopShape,
    hop: Option<Hop>,
    arc_sign: f32,
    base: Base<Node>,
}

/// Beats the landing shockwave stays dangerous.
const SHOCK_BEATS: f64 = 0.3;

#[godot_api]
impl HopComponent {
    /// A hop landed.
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        self.driver.windup()
    }

    /// `(lift px, squash)`.
    #[func]
    fn get_pose(&self) -> Vector2 {
        match self.hop {
            Some(hop) => {
                let pose = self.shape.pose(self.driver.beat - hop.landing);
                Vector2::new(pose.lift * self.hop_height, pose.squash)
            }
            None => Vector2::new(0.0, 1.0),
        }
    }

    /// Direction of the planned hop (zero when idle).
    #[func]
    fn get_aim(&self) -> Vector2 {
        self.hop
            .filter(|hop| self.driver.beat < hop.landing)
            .map_or(Vector2::ZERO, |hop| {
                to_vector((hop.to - hop.from).normalized_or_zero())
            })
    }

    /// Landing spot `(x, y, radius)` in global px while a hop is coming (radius 0
    /// otherwise), drawn by `EnemyVisual`.
    #[func]
    fn get_marker(&self) -> Vector3 {
        match self.hop {
            Some(hop) if self.driver.beat < hop.landing => Vector3::new(
                hop.to.x,
                hop.to.y,
                if self.shockwave_radius > 0.0 {
                    self.shockwave_radius
                } else {
                    14.0
                },
            ),
            _ => Vector3::ZERO,
        }
    }

    /// How close a player can safely stand (bots keep outside it).
    #[func]
    fn get_threat_radius(&self) -> f32 {
        self.shockwave_radius
    }

    /// The landing circle: pending through the crouch and airtime, active briefly
    /// after touchdown.
    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        match self.shock_shape() {
            Some(shape) => encode_shapes(&[shape]),
            None => PackedFloat32Array::new(),
        }
    }
}

impl HopComponent {
    fn shock_shape(&self) -> Option<DangerShape> {
        let hop = self.hop?;
        if self.shockwave_radius <= 0.0 {
            return None;
        }
        let u = self.driver.beat - hop.landing;
        if u > SHOCK_BEATS {
            return None;
        }
        Some(DangerShape::Circle {
            center: hop.to,
            radius: self.shockwave_radius,
            velocity: V2::ZERO,
            activates_in: self.driver.beats_to_seconds((-u).max(0.0)) as f32,
        })
    }

    fn plan(&mut self, parent: &Gd<Node2D>) {
        let index = self.driver.tracker.next_index();
        let from = to_v2(parent.get_global_position());
        let tree = self.base().get_tree();
        let target = nearest_alive(&tree, parent.get_global_position(), &self.target_group)
            .map(|t| to_v2(t.get_global_position()));
        let to = match target {
            Some(target) => {
                let to_target = target - from;
                let heading = arc_heading(to_target, 0.35 * self.arc_sign, ARC_FULL_AT);
                let apart = separation(
                    from,
                    &other_enemy_positions(&tree, parent),
                    SEPARATION_RADIUS,
                );
                let direction = (heading + apart * 1.2).normalized_or_zero();
                let reach = (to_target.length() - self.standoff).max(0.0);
                from + direction * reach.min(self.hop_distance)
            }
            None => from,
        };
        let node = self.to_gd().upcast::<Node>();
        self.hop = Some(Hop {
            from,
            to: clamp_to(to, arena_bounds(&node)),
            index,
            landing: self.driver.tracker.next_beat(),
        });
        self.arc_sign = -self.arc_sign;
    }
}

#[godot_api]
impl INode for HopComponent {
    fn ready(&mut self) {
        self.driver
            .configure(self.every_beats, self.offset_beats, self.shape.lead());
        self.arc_sign = if self.to_gd().instance_id().to_i64() % 2 == 0 {
            1.0
        } else {
            -1.0
        };
    }

    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let node = self.to_gd().upcast::<Node>();
        let fired = self.driver.tick(&node, delta);
        let next = self.driver.tracker.next_index();
        if self.driver.in_windup() && self.hop.is_none_or(|hop| hop.index != next) {
            let landed = self
                .hop
                .is_none_or(|hop| self.driver.beat - hop.landing > self.shape.settle);
            if landed {
                self.plan(&parent);
            }
        }
        let Some(hop) = self.hop else {
            return;
        };
        let u = self.driver.beat - hop.landing;
        if u <= self.shape.settle + 0.5 {
            let pose = self.shape.pose(u);
            let position = hop.from + (hop.to - hop.from) * pose.travel;
            parent.set_global_position(to_vector(position));
        }
        if let Some(index) = fired
            && index == hop.index
        {
            let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
            self.signals().acted().emit(action, beat);
            if self.shockwave_radius > 0.0 {
                let pos = to_vector(hop.to);
                let radius = self.shockwave_radius;
                with_fx(|fx| {
                    fx.ring(pos, ENEMY_GLOW, radius, 0.3);
                    fx.burst_style(pos, ENEMY_GLOW, 8, BurstStyle::Dots as i32, 0.8);
                    fx.shake(0.12);
                });
            }
        }
        if let Some(shape) = self.shock_shape()
            && shape.is_active()
        {
            hit_players(&parent, &shape);
        }
    }
}

/// Steps one diagonal cell per beat (landing on the beat) and bounces off the arena
/// edges. Its path is a closed-form function of the beat since it started.
#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct BounceComponent {
    /// Cell size (px) per axis per step.
    #[var]
    #[init(val = 56.0)]
    cell: f32,
    /// Beats each step takes, ending on the beat.
    #[var]
    #[init(val = 0.3)]
    move_beats: f64,
    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    driver: BeatDriver,
    origin: V2,
    direction: V2,
    start_beat: Option<f64>,
    contact_radius: f32,
    base: Base<Node>,
}

#[godot_api]
impl BounceComponent {
    /// A step landed.
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        self.driver.windup()
    }

    #[func]
    fn get_pose(&self) -> Vector2 {
        let Some(start) = self.start_beat else {
            return Vector2::new(0.0, 1.0);
        };
        let since = (self.driver.beat - start).max(0.0);
        let frac = since.fract() as f32;
        let m = self.move_beats as f32;
        let moving = ((frac - (1.0 - m)) / m).clamp(0.0, 1.0);
        let lift = 4.0 * moving * (1.0 - moving) * 6.0;
        // Squash right after each landing, stretched mid-step.
        let land = (-frac * 9.0).exp();
        let squash = 1.0 - 0.3 * land + 0.18 * (4.0 * moving * (1.0 - moving));
        Vector2::new(lift, squash)
    }

    /// Next landing cell `(x, y, radius)` in global px, drawn by `EnemyVisual`.
    #[func]
    fn get_marker(&self) -> Vector3 {
        let Some(start) = self.start_beat else {
            return Vector3::ZERO;
        };
        let node = self.to_gd().upcast::<Node>();
        let next = self.driver.tracker.next_beat();
        let landing = self.position_at(next.max(start), start, &node);
        Vector3::new(landing.x, landing.y, self.contact_radius)
    }

    /// Where the next step lands, pending until the beat.
    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        let Some(start) = self.start_beat else {
            return PackedFloat32Array::new();
        };
        let node = self.to_gd().upcast::<Node>();
        let next = self.driver.tracker.next_beat();
        let landing = self.position_at(next.max(start), start, &node);
        encode_shapes(&[DangerShape::Circle {
            center: landing,
            radius: self.contact_radius,
            velocity: V2::ZERO,
            activates_in: self.driver.seconds_until_next(),
        }])
    }
}

impl BounceComponent {
    fn position_at(&self, beat: f64, start: f64, node: &Gd<Node>) -> V2 {
        let (lo, hi) = arena_bounds(node);
        let steps = steps_at(beat, start, self.move_beats) as f32;
        bounce_position(self.origin, self.direction, self.cell * steps, lo, hi)
    }
}

#[godot_api]
impl INode for BounceComponent {
    fn ready(&mut self) {
        self.driver.configure(1.0, 0.0, self.move_beats);
    }

    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let node = self.to_gd().upcast::<Node>();
        let fired = self.driver.tick(&node, delta);
        if self.start_beat.is_none() {
            // Start heading diagonally toward the nearest player's quadrant.
            let origin = parent.get_global_position();
            let toward = nearest_alive(&self.base().get_tree(), origin, &self.target_group)
                .map_or(Vector2::new(1.0, 1.0), |t| t.get_global_position() - origin);
            let sign = |v: f32| if v < 0.0 { -1.0 } else { 1.0 };
            self.direction = V2::new(sign(toward.x), sign(toward.y));
            self.origin = clamp_to(to_v2(origin), arena_bounds(&node));
            self.start_beat = Some(self.driver.tracker.next_beat() - 1.0);
            self.contact_radius = parent
                .try_get_node_as::<Node>("ContactDamageComponent")
                .and_then(|c| super::circle_radius(&c))
                .unwrap_or(22.0);
        }
        let Some(start) = self.start_beat else {
            return;
        };
        let position = self.position_at(self.driver.beat, start, &node);
        parent.set_global_position(to_vector(position));
        if let Some(index) = fired {
            let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
            self.signals().acted().emit(action, beat);
        }
    }
}

/// One planned dash.
#[derive(Clone, Copy)]
struct Dash {
    from: V2,
    to: V2,
    index: i64,
    beat: f64,
}

/// Every `every_beats` it locks a line toward the nearest player and shows it for
/// `windup_beats`, then dashes along it in `dash_beats`. Between dashes it drifts.
/// Draws its own telegraph in global coordinates (top-level node).
#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct DashComponent {
    #[var]
    #[init(val = 4.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    #[var]
    #[init(val = 1.0)]
    windup_beats: f64,
    #[var]
    #[init(val = 0.3)]
    dash_beats: f64,
    #[var]
    #[init(val = 380.0)]
    max_length: f32,
    /// Drift speed between dashes (px/s).
    #[var]
    #[init(val = 24.0)]
    drift_speed: f32,
    /// Half-width of the dash lane (px).
    #[var]
    #[init(val = 20.0)]
    lane_radius: f32,
    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    driver: BeatDriver,
    dash: Option<Dash>,
    velocity: V2,
    last_position: V2,
    color: Color,
    base: Base<Node2D>,
}

#[godot_api]
impl DashComponent {
    /// The dash started.
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        self.driver.windup()
    }

    #[func]
    fn get_pose(&self) -> Vector2 {
        let Some(dash) = self.dash else {
            return Vector2::new(0.0, 1.0);
        };
        let u = self.driver.beat - dash.beat;
        if u < 0.0 {
            // Coil up while aiming.
            Vector2::new(0.0, 1.0 - 0.25 * self.driver.windup())
        } else if u < self.dash_beats {
            Vector2::new(0.0, 1.45)
        } else {
            let t = ((u - self.dash_beats) / 0.4).clamp(0.0, 1.0) as f32;
            Vector2::new(0.0, 1.0 - 0.25 * (1.0 - t) * (t * 5.0).cos())
        }
    }

    #[func]
    fn get_aim(&self) -> Vector2 {
        self.dash
            .filter(|dash| self.driver.beat < dash.beat + self.dash_beats)
            .map_or(Vector2::ZERO, |dash| {
                to_vector((dash.to - dash.from).normalized_or_zero())
            })
    }

    /// The dash lane: pending through the wind-up, active while dashing.
    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        let Some(dash) = self.dash else {
            return PackedFloat32Array::new();
        };
        let u = self.driver.beat - dash.beat;
        if u >= self.dash_beats {
            return PackedFloat32Array::new();
        }
        let a = if u < 0.0 {
            dash.from
        } else {
            self.last_position
        };
        encode_shapes(&[DangerShape::Capsule {
            a,
            b: dash.to,
            radius: self.lane_radius,
            activates_in: self.driver.beats_to_seconds((-u).max(0.0)) as f32,
        }])
    }
}

impl DashComponent {
    fn plan(&mut self, parent: &Gd<Node2D>) {
        let from = to_v2(parent.get_global_position());
        let node = self.to_gd().upcast::<Node>();
        let target = nearest_alive(
            &self.base().get_tree(),
            parent.get_global_position(),
            &self.target_group,
        )
        .map(|t| to_v2(t.get_global_position()));
        let Some(target) = target else {
            return;
        };
        let to_target = target - from;
        let direction = to_target.normalized_or_zero();
        let length = (to_target.length() + 70.0).clamp(140.0, self.max_length);
        self.dash = Some(Dash {
            from,
            to: clamp_to(from + direction * length, arena_bounds(&node)),
            index: self.driver.tracker.next_index(),
            beat: self.driver.tracker.next_beat(),
        });
    }

    fn drift(&mut self, parent: &mut Gd<Node2D>, dt: f32) {
        let origin = to_v2(parent.get_global_position());
        let target = nearest_alive(
            &self.base().get_tree(),
            parent.get_global_position(),
            &self.target_group,
        )
        .map(|t| to_v2(t.get_global_position()));
        let desired = target.map_or(V2::ZERO, |t| {
            let heading = arc_heading(t - origin, 0.9, ARC_FULL_AT);
            let apart = separation(
                origin,
                &other_enemy_positions(&parent.get_tree(), parent),
                SEPARATION_RADIUS,
            );
            (heading + apart * 1.5).normalized_or_zero() * self.drift_speed
        });
        self.velocity = steer(self.velocity, desired, 120.0, dt);
        let node = self.to_gd().upcast::<Node>();
        let next = clamp_to(origin + self.velocity * dt, arena_bounds(&node));
        parent.set_global_position(to_vector(next));
    }
}

#[godot_api]
impl INode2D for DashComponent {
    fn ready(&mut self) {
        self.base_mut().set_as_top_level(true);
        self.base_mut().set_global_position(Vector2::ZERO);
        self.base_mut().set_z_index(-2);
        self.color = ENEMY_GLOW;
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
    }

    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let node = self.to_gd().upcast::<Node>();
        let fired = self.driver.tick(&node, delta);
        let next = self.driver.tracker.next_index();
        let idle = self
            .dash
            .is_none_or(|dash| self.driver.beat >= dash.beat + self.dash_beats);
        if self.driver.in_windup() && idle && self.dash.is_none_or(|dash| dash.index != next) {
            self.plan(&parent);
        }
        let before = to_v2(parent.get_global_position());
        match self.dash {
            Some(dash) if self.driver.beat < dash.beat => {}
            Some(dash) if self.driver.beat < dash.beat + self.dash_beats => {
                let t = ((self.driver.beat - dash.beat) / self.dash_beats) as f32;
                let position = dash.from + (dash.to - dash.from) * ease_out_cubic(t);
                parent.set_global_position(to_vector(position));
                let lane = DangerShape::Capsule {
                    a: before,
                    b: position,
                    radius: self.lane_radius,
                    activates_in: 0.0,
                };
                hit_players(&parent, &lane);
            }
            Some(dash) if self.driver.beat < dash.beat + self.dash_beats + 0.05 => {
                parent.set_global_position(to_vector(dash.to));
            }
            _ => self.drift(&mut parent, delta as f32),
        }
        self.last_position = to_v2(parent.get_global_position());
        if let Some(index) = fired
            && self.dash.is_some_and(|dash| dash.index == index)
        {
            let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
            self.signals().acted().emit(action, beat);
            let pos = parent.get_global_position();
            with_fx(|fx| {
                fx.burst_style(pos, ENEMY_GLOW, 10, BurstStyle::Sparks as i32, 1.2);
                fx.play_sfx("laser_fire".into(), pos, 0.1, -8.0);
            });
        }
        self.base_mut().queue_redraw();
    }

    /// Telegraph: a lane that fills toward the dash beat with chevrons pointing along
    /// it and a target ring; while dashing, a fading streak.
    fn draw(&mut self) {
        let Some(dash) = self.dash else {
            return;
        };
        let u = self.driver.beat - dash.beat;
        let glow = self.color;
        let from = to_vector(dash.from);
        let to = to_vector(dash.to);
        let direction = (to - from).normalized_or_zero();
        if u < 0.0 {
            let w = self.driver.windup();
            let flicker = if w > 0.75 && (self.driver.beat * 16.0).fract() < 0.5 {
                0.3
            } else {
                0.0
            };
            let alpha = 0.2 + 0.5 * w + flicker;
            let lane = self.lane_radius;
            self.base_mut()
                .draw_line_ex(
                    from,
                    to,
                    Color::from_rgba(glow.r, glow.g, glow.b, 0.12 + 0.18 * w),
                )
                .width(lane * 2.0 * w.max(0.15))
                .done();
            self.base_mut()
                .draw_line_ex(from, to, Color::from_rgba(glow.r, glow.g, glow.b, alpha))
                .width(2.0)
                .antialiased(true)
                .done();
            let length = from.distance_to(to);
            let side = Vector2::new(-direction.y, direction.x);
            let mut d = 30.0 + (self.driver.beat as f32 * 60.0) % 40.0;
            while d < length - 10.0 {
                let p = from + direction * d;
                let color = Color::from_rgba(glow.r, glow.g, glow.b, alpha * 0.8);
                self.base_mut()
                    .draw_line_ex(p - direction * 8.0 + side * 8.0, p, color)
                    .width(2.0)
                    .done();
                self.base_mut()
                    .draw_line_ex(p - direction * 8.0 - side * 8.0, p, color)
                    .width(2.0)
                    .done();
                d += 40.0;
            }
            self.base_mut()
                .draw_arc_ex(
                    to,
                    lane * (1.0 + 0.8 * (1.0 - w)),
                    0.0,
                    TAU,
                    24,
                    Color::from_rgba(glow.r, glow.g, glow.b, alpha),
                )
                .width(2.0)
                .antialiased(true)
                .done();
        } else if u < self.dash_beats + 0.4 {
            let lane = self.lane_radius;
            let t = (u / (self.dash_beats + 0.4)) as f32;
            let head = to_vector(self.last_position);
            self.base_mut()
                .draw_line_ex(
                    from,
                    head,
                    Color::from_rgba(1.0, 0.95, 0.85, 0.7 * (1.0 - t)),
                )
                .width(lane * 1.4 * (1.0 - t))
                .done();
        }
    }
}

/// Circles the arena center, `1 / beats_per_turn` of a turn per beat, at
/// `radius_fraction` of the arena's shorter side. Glides onto the circle over
/// `join_beats` from wherever it spawned. The direction comes from the instance.
#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct OrbitComponent {
    #[var]
    #[init(val = 0.38)]
    radius_fraction: f32,
    #[var]
    #[init(val = 32.0)]
    beats_per_turn: f64,
    #[var]
    #[init(val = 2.0)]
    join_beats: f64,

    driver: BeatDriver,
    /// `(spawn beat, start angle, spawn position)`, set on the first frame.
    start: Option<(f64, f32, V2)>,
    direction: f32,
    base: Base<Node>,
}

impl OrbitComponent {
    fn angle_at(&self, spawn: f64, start_angle: f32) -> f32 {
        let turns = (self.driver.beat - spawn) / self.beats_per_turn.max(1.0);
        start_angle + self.direction * TAU * turns as f32
    }
}

#[godot_api]
impl INode for OrbitComponent {
    fn ready(&mut self) {
        self.driver.configure(1.0, 0.0, 0.0);
        self.direction = if self.to_gd().instance_id().to_i64() % 2 == 0 {
            1.0
        } else {
            -1.0
        };
    }

    fn process(&mut self, delta: f64) {
        let Some(mut parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let node = self.to_gd().upcast::<Node>();
        self.driver.tick(&node, delta);
        let (lo, hi) = arena_bounds(&node);
        let center = (lo + hi) * 0.5;
        let radius = (hi.x - lo.x).min(hi.y - lo.y) * self.radius_fraction;
        let position = to_v2(parent.get_global_position());
        let beat = self.driver.beat;
        let (spawn, start_angle, from) = *self.start.get_or_insert_with(|| {
            let offset = position - center;
            (beat, offset.y.atan2(offset.x), position)
        });
        let a = self.angle_at(spawn, start_angle);
        let on_circle = center + V2::new(a.cos(), a.sin()) * radius;
        let join =
            ease_out_cubic(((beat - spawn) / self.join_beats.max(0.01)).clamp(0.0, 1.0) as f32);
        parent.set_global_position(to_vector(from + (on_circle - from) * join));
    }
}
