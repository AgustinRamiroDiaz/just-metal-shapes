//! Beat-locked attack components. Each fires on its cadence after a wind-up and
//! emits `fired(projectile)` per projectile (for `EnemyVisual`'s muzzle flash) and
//! `acted(action_beat, song_beat)` once per volley.
//!
//! - `ShooterComponent`: one shot at the nearest player.
//! - `ShotgunShooterComponent`: a fan along the sprite's facing.
//! - `TurretShooterComponent`: four shots, alternating cardinal and diagonal.
//! - `MineDropperComponent`: drops a mine where it stands.
//! - `RingEmitterComponent`: a ring of shots with a gap that turns every volley.
//! - `LanceComponent`: aims a thin beam at a player, then fires it.

use super::{
    BeatDriver, load_packed_scene, nearest_alive, parent_as_node2d, spawn_projectile, to_vector,
};
use crate::core::beat_motion::ring_angles;
use crate::core::danger::{DangerShape, V2};
use crate::fx::{BurstStyle, with_fx};
use crate::hazards::{encode_shapes, hit_players, to_v2};
use crate::visuals::enemy_visual::ENEMY_GLOW;
use godot::classes::{Area2D, INode, INode2D, Node, Node2D, PackedScene, Sprite2D};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;
const PROJECTILE_SCENE: &str = "res://scenes/projectile.tscn";
/// `projectile.tscn` speed and radius, for pending-shot danger shapes.
const DEFAULT_PROJECTILE_SPEED: f32 = 100.0;
const PROJECTILE_RADIUS: f32 = 12.0;

/// Shots that will leave `origin` along `directions` in `activates_in` seconds,
/// reported shifted back along their velocity (see `DangerShape::advanced`).
fn pending_shots(
    origin: V2,
    directions: &[V2],
    speed: f32,
    activates_in: f32,
) -> PackedFloat32Array {
    let shapes: Vec<DangerShape> = directions
        .iter()
        .map(|d| {
            let velocity = *d * speed;
            DangerShape::Circle {
                center: origin - velocity * activates_in,
                radius: PROJECTILE_RADIUS,
                velocity,
                activates_in,
            }
        })
        .collect();
    encode_shapes(&shapes)
}

fn speed_or_default(speed: f32) -> f32 {
    if speed > 0.0 {
        speed
    } else {
        DEFAULT_PROJECTILE_SPEED
    }
}

/// Direction from `origin` to the nearest living player, if any.
fn aim_at_nearest(node: &Gd<Node>, origin: Vector2, group: &StringName) -> Option<Vector2> {
    nearest_alive(&node.get_tree(), origin, group)
        .map(|target| (target.get_global_position() - origin).normalized_or_zero())
}

/// Fires `directions` from `parent` and emits `fired` per projectile via `emit`.
fn volley(
    scene: &Option<Gd<PackedScene>>,
    parent: &Gd<Node2D>,
    directions: &[Vector2],
    speed: f32,
    mut emit: impl FnMut(Gd<Node>),
) {
    let Some(scene) = scene else {
        return;
    };
    let origin = parent.get_global_position();
    for direction in directions {
        if let Some(projectile) = spawn_projectile(scene, parent, origin, *direction, speed) {
            emit(projectile);
        }
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct ShooterComponent {
    #[var]
    #[init(val = 4.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    #[var]
    #[init(val = 1.0)]
    windup_beats: f64,
    /// Projectile speed (px/s); 0 keeps the projectile's default.
    #[var]
    projectile_speed: f32,
    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    driver: BeatDriver,
    projectile_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

#[godot_api]
impl ShooterComponent {
    #[signal]
    fn fired(projectile: Gd<Node>);
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

    /// Toward the nearest player while winding up.
    #[func]
    fn get_aim(&self) -> Vector2 {
        if !self.driver.in_windup() {
            return Vector2::ZERO;
        }
        self.origin()
            .and_then(|o| aim_at_nearest(&self.to_gd().upcast(), o, &self.target_group))
            .unwrap_or(Vector2::ZERO)
    }

    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        let aim = self.get_aim();
        match self.origin() {
            Some(origin) if aim != Vector2::ZERO => pending_shots(
                to_v2(origin),
                &[to_v2(aim)],
                speed_or_default(self.projectile_speed),
                self.driver.seconds_until_next(),
            ),
            _ => PackedFloat32Array::new(),
        }
    }
}

impl ShooterComponent {
    fn origin(&self) -> Option<Vector2> {
        parent_as_node2d(self.base().get_parent()).map(|p| p.get_global_position())
    }
}

#[godot_api]
impl INode for ShooterComponent {
    fn ready(&mut self) {
        self.projectile_scene = load_packed_scene(PROJECTILE_SCENE);
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        let Some(index) = self.driver.tick(&node, delta) else {
            return;
        };
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let Some(direction) =
            aim_at_nearest(&node, parent.get_global_position(), &self.target_group)
        else {
            return;
        };
        let mut fired = Vec::new();
        volley(
            &self.projectile_scene,
            &parent,
            &[direction],
            self.projectile_speed,
            |p| fired.push(p),
        );
        for projectile in fired {
            self.signals().fired().emit(&projectile);
        }
        let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
        self.signals().acted().emit(action, beat);
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct ShotgunShooterComponent {
    #[var]
    #[init(val = 4.0)]
    every_beats: f64,
    #[var]
    #[init(val = 2.0)]
    offset_beats: f64,
    #[var]
    #[init(val = 1.0)]
    windup_beats: f64,
    #[var]
    #[init(val = 3)]
    shot_count: i32,
    #[var]
    #[init(val = 45.0)]
    spread_angle: f32,
    #[var]
    projectile_speed: f32,
    #[var]
    #[init(val = NodePath::from("Sprite2D"))]
    sprite_path: NodePath,

    driver: BeatDriver,
    projectile_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

#[godot_api]
impl ShotgunShooterComponent {
    #[signal]
    fn fired(projectile: Gd<Node>);
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
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        if !self.driver.in_windup() {
            return PackedFloat32Array::new();
        }
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return PackedFloat32Array::new();
        };
        let directions: Vec<V2> = self.directions(&parent).into_iter().map(to_v2).collect();
        pending_shots(
            to_v2(parent.get_global_position()),
            &directions,
            speed_or_default(self.projectile_speed),
            self.driver.seconds_until_next(),
        )
    }
}

impl ShotgunShooterComponent {
    fn directions(&self, parent: &Gd<Node2D>) -> Vec<Vector2> {
        let facing = parent
            .try_get_node_as::<Sprite2D>(&self.sprite_path)
            .map_or(0.0, |sprite| sprite.get_rotation());
        let base_direction = Vector2::RIGHT.rotated(facing);
        let start_angle = -self.spread_angle / 2.0;
        let angle_step = if self.shot_count > 1 {
            self.spread_angle / (self.shot_count - 1) as f32
        } else {
            0.0
        };
        (0..self.shot_count)
            .map(|i| base_direction.rotated((start_angle + angle_step * i as f32).to_radians()))
            .collect()
    }
}

#[godot_api]
impl INode for ShotgunShooterComponent {
    fn ready(&mut self) {
        self.projectile_scene = load_packed_scene(PROJECTILE_SCENE);
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        let Some(index) = self.driver.tick(&node, delta) else {
            return;
        };
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let directions = self.directions(&parent);
        let mut fired = Vec::new();
        volley(
            &self.projectile_scene,
            &parent,
            &directions,
            self.projectile_speed,
            |p| fired.push(p),
        );
        for projectile in fired {
            self.signals().fired().emit(&projectile);
        }
        let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
        self.signals().acted().emit(action, beat);
    }
}

/// Four shots per volley: cardinal on even volleys, diagonal on odd ones (the
/// volley index comes from the song beat, so the pattern survives rewinds).
#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct TurretShooterComponent {
    #[var]
    #[init(val = 2.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    #[var]
    #[init(val = 0.75)]
    windup_beats: f64,
    #[var]
    projectile_speed: f32,

    driver: BeatDriver,
    projectile_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

impl TurretShooterComponent {
    fn directions(index: i64) -> [Vector2; 4] {
        let turn = if index.rem_euclid(2) == 0 {
            0.0
        } else {
            TAU / 8.0
        };
        [0, 1, 2, 3].map(|k| Vector2::from_angle(turn + k as f32 * TAU / 4.0))
    }
}

#[godot_api]
impl TurretShooterComponent {
    #[signal]
    fn fired(projectile: Gd<Node>);
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

    /// The next volley's first direction while winding up (the body is four-way).
    #[func]
    fn get_aim(&self) -> Vector2 {
        if !self.driver.in_windup() {
            return Vector2::ZERO;
        }
        Self::directions(self.driver.tracker.next_index())[0]
    }

    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        if !self.driver.in_windup() {
            return PackedFloat32Array::new();
        }
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return PackedFloat32Array::new();
        };
        let directions: Vec<V2> = Self::directions(self.driver.tracker.next_index())
            .into_iter()
            .map(to_v2)
            .collect();
        pending_shots(
            to_v2(parent.get_global_position()),
            &directions,
            speed_or_default(self.projectile_speed),
            self.driver.seconds_until_next(),
        )
    }
}

#[godot_api]
impl INode for TurretShooterComponent {
    fn ready(&mut self) {
        self.projectile_scene = load_packed_scene(PROJECTILE_SCENE);
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        let Some(index) = self.driver.tick(&node, delta) else {
            return;
        };
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let mut fired = Vec::new();
        volley(
            &self.projectile_scene,
            &parent,
            &Self::directions(index),
            self.projectile_speed,
            |p| fired.push(p),
        );
        for projectile in fired {
            self.signals().fired().emit(&projectile);
        }
        let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
        self.signals().acted().emit(action, beat);
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct MineDropperComponent {
    #[var]
    #[init(val = 4.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    #[var]
    #[init(val = 0.5)]
    windup_beats: f64,

    driver: BeatDriver,
    mine_scene: Option<Gd<PackedScene>>,
    base: Base<Node>,
}

#[godot_api]
impl MineDropperComponent {
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
}

#[godot_api]
impl INode for MineDropperComponent {
    fn ready(&mut self) {
        self.mine_scene = load_packed_scene("res://scenes/mine.tscn");
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        let Some(index) = self.driver.tick(&node, delta) else {
            return;
        };
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        // Find the container before instantiating: an unparented mine would leak.
        let Some(mut container) = parent.get_parent() else {
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
        container.add_child(&mine);
        let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
        self.signals().acted().emit(action, beat);
    }
}

/// Every volley fires `count` shots in a ring with `gap` shots left out; the gap
/// turns by `gap_step` radians each volley (a quarter turn per bar by default).
/// Draws the next ring's spokes during the wind-up so the gap reads before it fires.
#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct RingEmitterComponent {
    #[var]
    #[init(val = 4.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    #[var]
    #[init(val = 1.0)]
    windup_beats: f64,
    #[var]
    #[init(val = 14)]
    count: i32,
    #[var]
    #[init(val = 3)]
    gap: i32,
    #[var]
    #[init(val = 1.5707964)]
    gap_step: f32,
    #[var]
    #[init(val = 130.0)]
    projectile_speed: f32,

    driver: BeatDriver,
    /// Gap angle of volley 0, fixed at spawn so each Pulser has its own start.
    gap_origin: f32,
    projectile_scene: Option<Gd<PackedScene>>,
    base: Base<Node2D>,
}

#[godot_api]
impl RingEmitterComponent {
    #[signal]
    fn fired(projectile: Gd<Node>);
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

    /// Gap direction (unit vector) of the next volley.
    #[func]
    fn get_gap_direction(&self) -> Vector2 {
        Vector2::from_angle(self.gap_angle(self.driver.tracker.next_index()))
    }

    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        if !self.driver.in_windup() {
            return PackedFloat32Array::new();
        }
        let directions: Vec<V2> = self
            .angles(self.driver.tracker.next_index())
            .into_iter()
            .map(V2::from_angle)
            .collect();
        pending_shots(
            to_v2(self.base().get_global_position()),
            &directions,
            speed_or_default(self.projectile_speed),
            self.driver.seconds_until_next(),
        )
    }
}

impl RingEmitterComponent {
    fn gap_angle(&self, index: i64) -> f32 {
        self.gap_origin + index as f32 * self.gap_step
    }

    fn angles(&self, index: i64) -> Vec<f32> {
        ring_angles(
            self.count.max(0) as u32,
            self.gap.max(0) as u32,
            self.gap_angle(index),
        )
    }
}

#[godot_api]
impl INode2D for RingEmitterComponent {
    fn ready(&mut self) {
        self.projectile_scene = load_packed_scene(PROJECTILE_SCENE);
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
        let id = self.to_gd().instance_id().to_i64();
        self.gap_origin = (id.rem_euclid(4)) as f32 * TAU / 4.0 + TAU / 8.0;
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        let fired_index = self.driver.tick(&node, delta);
        self.base_mut().queue_redraw();
        let Some(index) = fired_index else {
            return;
        };
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let directions: Vec<Vector2> = self
            .angles(index)
            .into_iter()
            .map(Vector2::from_angle)
            .collect();
        let mut fired = Vec::new();
        volley(
            &self.projectile_scene,
            &parent,
            &directions,
            self.projectile_speed,
            |p| fired.push(p),
        );
        for projectile in fired {
            self.signals().fired().emit(&projectile);
        }
        let pos = parent.get_global_position();
        with_fx(|fx| fx.ring(pos, ENEMY_GLOW, 60.0, 0.3));
        let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
        self.signals().acted().emit(action, beat);
    }

    /// Wind-up: a spoke per upcoming shot growing outward, the gap left dark with
    /// two brackets marking it.
    fn draw(&mut self) {
        let w = self.driver.windup();
        if w <= 0.0 {
            return;
        }
        let glow = ENEMY_GLOW;
        let next = self.driver.tracker.next_index();
        let inner = 30.0;
        let outer = inner + 8.0 + 22.0 * w;
        let alpha = 0.25 + 0.65 * w;
        for angle in self.angles(next) {
            let d = Vector2::from_angle(angle);
            self.base_mut()
                .draw_line_ex(
                    d * inner,
                    d * outer,
                    Color::from_rgba(glow.r, glow.g, glow.b, alpha),
                )
                .width(3.0)
                .antialiased(true)
                .done();
        }
        let gap = self.gap_angle(next);
        let half = (self.gap.max(1) as f32 / 2.0) * TAU / self.count.max(1) as f32;
        for side in [-1.0f32, 1.0] {
            let d = Vector2::from_angle(gap + side * half);
            self.base_mut()
                .draw_line_ex(
                    d * (outer + 2.0),
                    d * (outer + 14.0),
                    Color::from_rgba(1.0, 0.95, 0.85, alpha),
                )
                .width(2.0)
                .done();
        }
    }
}

/// A thin beam: aims at the nearest player when its wind-up starts, shows the line
/// for `windup_beats`, then fires for `active_beats`. The aim is locked for the whole
/// wind-up, so stepping off the line dodges it. Draws in global coordinates.
#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct LanceComponent {
    #[var]
    #[init(val = 8.0)]
    every_beats: f64,
    #[var]
    #[init(val = 4.0)]
    offset_beats: f64,
    #[var]
    #[init(val = 2.0)]
    windup_beats: f64,
    #[var]
    #[init(val = 0.5)]
    active_beats: f64,
    /// Beam half-width (px).
    #[var]
    #[init(val = 9.0)]
    beam_radius: f32,
    #[var]
    #[init(val = 1600.0)]
    beam_length: f32,
    #[var]
    #[init(val = StringName::from("players"))]
    target_group: StringName,

    driver: BeatDriver,
    /// (action index, origin, direction) of the current shot.
    shot: Option<(i64, V2, V2)>,
    charge_played: i64,
    base: Base<Node2D>,
}

#[godot_api]
impl LanceComponent {
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
    fn get_aim(&self) -> Vector2 {
        self.shot
            .filter(|_| self.beam_state().is_some())
            .map_or(Vector2::ZERO, |(_, _, d)| to_vector(d))
    }

    /// True while the beam is firing.
    #[func]
    fn is_firing(&self) -> bool {
        matches!(self.beam_state(), Some(u) if u >= 0.0)
    }

    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        match self.beam() {
            Some(shape) => encode_shapes(&[shape]),
            None => PackedFloat32Array::new(),
        }
    }
}

impl LanceComponent {
    /// Beats relative to the shot (negative while aiming), or None when idle.
    fn beam_state(&self) -> Option<f64> {
        let (index, _, _) = self.shot?;
        let u = self.driver.beat - self.driver.action_beat(index);
        (u >= -self.driver.tracker.windup - 0.01 && u < self.active_beats).then_some(u)
    }

    fn beam(&self) -> Option<DangerShape> {
        let u = self.beam_state()?;
        let (_, origin, direction) = self.shot?;
        Some(DangerShape::Capsule {
            a: origin,
            b: origin + direction * self.beam_length,
            radius: self.beam_radius,
            activates_in: self.driver.beats_to_seconds((-u).max(0.0)) as f32,
        })
    }
}

#[godot_api]
impl INode2D for LanceComponent {
    fn ready(&mut self) {
        self.base_mut().set_as_top_level(true);
        self.base_mut().set_global_position(Vector2::ZERO);
        self.base_mut().set_z_index(-2);
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
        self.charge_played = i64::MIN;
    }

    fn process(&mut self, delta: f64) {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let node = self.to_gd().upcast::<Node>();
        let fired = self.driver.tick(&node, delta);
        let next = self.driver.tracker.next_index();
        if self.driver.in_windup() && self.shot.is_none_or(|(index, _, _)| index != next) {
            let origin = parent.get_global_position();
            if let Some(direction) = aim_at_nearest(&node, origin, &self.target_group) {
                self.shot = Some((next, to_v2(origin), to_v2(direction)));
            }
        }
        if let Some((index, origin, _)) = self.shot
            && index == next
            && self.charge_played != index
        {
            self.charge_played = index;
            with_fx(|fx| {
                fx.play_sfx("laser_charge".into(), to_vector(origin), 0.1, -10.0);
            });
        }
        if let Some(index) = fired
            && self.shot.is_some_and(|(i, _, _)| i == index)
        {
            let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
            self.signals().acted().emit(action, beat);
            let pos = parent.get_global_position();
            with_fx(|fx| {
                fx.burst_style(pos, ENEMY_GLOW, 8, BurstStyle::Sparks as i32, 1.0);
                fx.shake(0.15);
                fx.play_sfx("laser_fire".into(), pos, 0.1, -6.0);
            });
        }
        if let Some(beam) = self.beam()
            && beam.is_active()
        {
            hit_players(&parent, &beam);
        }
        self.base_mut().queue_redraw();
    }

    /// Aiming: a thin line that brightens and flickers toward the shot. Firing: a
    /// white-hot core in an amber glow that narrows away.
    fn draw(&mut self) {
        let (Some(u), Some((_, origin, direction))) = (self.beam_state(), self.shot) else {
            return;
        };
        let glow = ENEMY_GLOW;
        let beam = self.beam_radius;
        let a = to_vector(origin);
        let b = to_vector(origin + direction * self.beam_length);
        if u < 0.0 {
            let w = self.driver.windup();
            let flicker = w > 0.75 && (self.driver.beat * 16.0).fract() < 0.5;
            let alpha = 0.25 + 0.5 * w + if flicker { 0.25 } else { 0.0 };
            self.base_mut()
                .draw_line_ex(
                    a,
                    b,
                    Color::from_rgba(glow.r, glow.g, glow.b, 0.10 + 0.15 * w),
                )
                .width(beam * 2.0 * w)
                .done();
            self.base_mut()
                .draw_line_ex(a, b, Color::from_rgba(glow.r, glow.g, glow.b, alpha))
                .width(1.5)
                .antialiased(true)
                .done();
        } else {
            let t = (u / self.active_beats.max(0.01)) as f32;
            let narrow = if t > 0.6 { 1.0 - (t - 0.6) / 0.4 } else { 1.0 };
            let flash = (1.0 - t * 4.0).max(0.0);
            self.base_mut()
                .draw_line_ex(
                    a,
                    b,
                    Color::from_rgba(glow.r, glow.g, glow.b, 0.5 * narrow + 0.3 * flash),
                )
                .width(beam * 3.2 * narrow)
                .done();
            self.base_mut()
                .draw_line_ex(a, b, Color::from_rgba(1.0, 0.97, 0.9, narrow))
                .width(beam * 1.1 * narrow)
                .done();
        }
    }
}
