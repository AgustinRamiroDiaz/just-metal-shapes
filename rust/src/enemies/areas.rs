//! Area attacks the component draws itself.
//!
//! - `ShockwaveComponent`: on its cadence, a ring expands from the body to
//!   `max_radius` with `gaps` openings; the gaps turn by `gap_step` every wave, so
//!   players slip through a gap or stay out of reach.
//! - `SpinBladeComponent`: blades that turn a step on every beat (snapping round on
//!   the beat, then holding), always dangerous.

use super::{BeatDriver, nearest_alive, parent_as_node2d};
use crate::core::beat_motion::{ease_out_cubic, smoothstep};
use crate::core::danger::{DangerShape, V2};
use crate::fx::with_fx;
use crate::hazards::{encode_shapes, hit_players_any, to_v2};
use crate::visuals::enemy_visual::ENEMY_GLOW;
use godot::classes::{INode2D, Node, Node2D};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;
/// Segments per full ring, for drawing and danger capsules.
const RING_SEGMENTS: usize = 40;
const WHITE_HOT: Color = Color::from_rgba(1.0, 0.95, 0.85, 1.0);

/// A wave in flight: its center (global px), start beat and gap rotation.
#[derive(Clone, Copy)]
struct Wave {
    center: V2,
    start: f64,
    gap_angle: f32,
}

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct ShockwaveComponent {
    #[var]
    #[init(val = 4.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    #[var]
    #[init(val = 1.0)]
    windup_beats: f64,
    /// Radius (px) the ring stops at.
    #[var]
    #[init(val = 200.0)]
    max_radius: f32,
    /// Radius (px) the ring starts at (about the body).
    #[var]
    #[init(val = 26.0)]
    start_radius: f32,
    /// Beats to travel from `start_radius` to `max_radius`.
    #[var]
    #[init(val = 1.0)]
    travel_beats: f64,
    /// Ring thickness (px).
    #[var]
    #[init(val = 12.0)]
    width: f32,
    /// Openings in the ring, evenly spaced.
    #[var]
    #[init(val = 1)]
    gaps: i32,
    /// Angular width of each opening (radians).
    #[var]
    #[init(val = 1.0)]
    gap_width: f32,
    /// Turn of the openings from one wave to the next (radians).
    #[var]
    #[init(val = std::f32::consts::FRAC_PI_2)]
    gap_step: f32,
    /// Aim the first opening at the nearest player instead of a fixed angle.
    #[var]
    aim_gap: bool,

    driver: BeatDriver,
    waves: Vec<Wave>,
    gap_origin: f32,
    base: Base<Node2D>,
}

#[godot_api]
impl ShockwaveComponent {
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
    fn get_threat_radius(&self) -> f32 {
        self.max_radius
    }

    /// Rings in flight (active) and the coming one at its start radius (pending).
    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        let mut shapes = self.active_shapes();
        if self.driver.in_windup()
            && let Some(origin) = self.origin()
        {
            let activates_in = self.driver.seconds_until_next();
            let wave = Wave {
                center: origin,
                start: self.driver.tracker.next_beat(),
                gap_angle: self.next_gap_angle(),
            };
            shapes.extend(self.ring_shapes(&wave, self.start_radius, activates_in));
        }
        encode_shapes(&shapes)
    }

    /// `(radius, gap angle)` of the newest wave in flight; radius 0 when none.
    #[func]
    fn get_wave(&self) -> Vector2 {
        self.waves.last().map_or(Vector2::ZERO, |wave| {
            Vector2::new(self.radius_at(wave), wave.gap_angle)
        })
    }
}

impl ShockwaveComponent {
    fn origin(&self) -> Option<V2> {
        parent_as_node2d(self.base().get_parent()).map(|p| to_v2(p.get_global_position()))
    }

    fn radius_at(&self, wave: &Wave) -> f32 {
        let u = ((self.driver.beat - wave.start) / self.travel_beats.max(0.05)) as f32;
        self.start_radius
            + (self.max_radius - self.start_radius) * ease_out_cubic(u.clamp(0.0, 1.0))
    }

    fn next_gap_angle(&self) -> f32 {
        let index = self.driver.tracker.next_index();
        self.gap_origin + index as f32 * self.gap_step
    }

    /// Whether `angle` falls inside one of the wave's openings.
    fn in_gap(&self, wave: &Wave, angle: f32) -> bool {
        let gaps = self.gaps.max(0);
        (0..gaps).any(|k| {
            let center = wave.gap_angle + k as f32 * TAU / gaps as f32;
            let d = (angle - center + TAU / 2.0).rem_euclid(TAU) - TAU / 2.0;
            d.abs() < self.gap_width / 2.0
        })
    }

    /// The ring as capsules between consecutive points, skipping the openings.
    fn ring_shapes(&self, wave: &Wave, radius: f32, activates_in: f32) -> Vec<DangerShape> {
        (0..RING_SEGMENTS)
            .filter_map(|i| {
                let a0 = TAU * i as f32 / RING_SEGMENTS as f32;
                let a1 = TAU * (i + 1) as f32 / RING_SEGMENTS as f32;
                if self.in_gap(wave, (a0 + a1) / 2.0) {
                    return None;
                }
                Some(DangerShape::Capsule {
                    a: wave.center + V2::new(a0.cos(), a0.sin()) * radius,
                    b: wave.center + V2::new(a1.cos(), a1.sin()) * radius,
                    radius: self.width / 2.0,
                    activates_in,
                })
            })
            .collect()
    }

    fn active_shapes(&self) -> Vec<DangerShape> {
        self.waves
            .iter()
            .flat_map(|wave| self.ring_shapes(wave, self.radius_at(wave), 0.0))
            .collect()
    }

    fn draw_ring(&mut self, center: Vector2, radius: f32, wave: &Wave, width: f32, color: Color) {
        let step = TAU / RING_SEGMENTS as f32;
        let mut run: Option<f32> = None;
        for i in 0..=RING_SEGMENTS {
            let a = step * i as f32;
            let open = i == RING_SEGMENTS || self.in_gap(wave, a + step / 2.0);
            match (run, open) {
                (None, false) => run = Some(a),
                (Some(start), true) => {
                    self.base_mut()
                        .draw_arc_ex(center, radius, start, a, 6, color)
                        .width(width)
                        .antialiased(true)
                        .done();
                    run = None;
                }
                _ => {}
            }
        }
    }
}

#[godot_api]
impl INode2D for ShockwaveComponent {
    fn ready(&mut self) {
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
        self.gap_origin = (self.to_gd().instance_id().to_i64() % 8) as f32 * TAU / 8.0;
        // Rings are drawn in global space under the body.
        self.base_mut().set_as_top_level(true);
        self.base_mut().set_z_index(-1);
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        if self.driver.in_windup()
            && self.aim_gap
            && let Some(parent) = parent_as_node2d(self.base().get_parent())
            && let Some(target) = nearest_alive(
                &self.base().get_tree(),
                parent.get_global_position(),
                &"players".into(),
            )
        {
            // Point the coming wave's first opening at the nearest player.
            let to = target.get_global_position() - parent.get_global_position();
            let index = self.driver.tracker.next_index();
            self.gap_origin = to.angle() - index as f32 * self.gap_step;
        }
        if let Some(index) = self.driver.tick(&node, delta)
            && let Some(origin) = self.origin()
        {
            let start = self.driver.action_beat(index);
            let gap_angle = self.gap_origin + index as f32 * self.gap_step;
            self.waves.push(Wave {
                center: origin,
                start,
                gap_angle,
            });
            let beat = self.driver.beat;
            self.signals().acted().emit(start, beat);
            let position = Vector2::new(origin.x, origin.y);
            with_fx(|fx| fx.ring(position, WHITE_HOT, self.start_radius * 1.6, 0.18));
        }
        let beat = self.driver.beat;
        let travel = self.travel_beats;
        self.waves
            .retain(|wave| beat - wave.start <= travel && beat >= wave.start - 0.5);
        if let Some(parent) = parent_as_node2d(self.base().get_parent()) {
            let shapes = self.active_shapes();
            hit_players_any(&parent, &shapes);
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let glow = ENEMY_GLOW;
        let waves = self.waves.clone();
        for wave in &waves {
            let center = Vector2::new(wave.center.x, wave.center.y);
            let radius = self.radius_at(wave);
            let u = ((self.driver.beat - wave.start) / self.travel_beats.max(0.05)) as f32;
            let fade = 1.0 - 0.5 * u.clamp(0.0, 1.0);
            let halo = Color::from_rgba(glow.r, glow.g, glow.b, 0.35 * fade);
            self.draw_ring(center, radius, wave, self.width * 1.8, halo);
            let core = Color::from_rgba(glow.r, glow.g, glow.b, 0.95 * fade);
            self.draw_ring(center, radius, wave, self.width * 0.75, core);
            self.draw_ring(center, radius, wave, 2.0, WHITE_HOT);
        }
        // Wind-up: the full reach as a faint ring, openings and all.
        let w = self.driver.windup();
        if w > 0.0
            && let Some(origin) = self.origin()
        {
            let wave = Wave {
                center: origin,
                start: 0.0,
                gap_angle: self.next_gap_angle(),
            };
            let center = Vector2::new(origin.x, origin.y);
            let color = Color::from_rgba(glow.r, glow.g, glow.b, 0.12 + 0.35 * w);
            let radius = self.max_radius;
            self.draw_ring(center, radius, &wave, 2.0 + 2.0 * w, color);
        }
    }
}

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct SpinBladeComponent {
    /// Blades, evenly spaced.
    #[var]
    #[init(val = 2)]
    blades: i32,
    /// Blade reach from the body center (px).
    #[var]
    #[init(val = 96.0)]
    length: f32,
    /// Where the blade starts (px from the center).
    #[var]
    #[init(val = 20.0)]
    inner: f32,
    /// Blade thickness (px).
    #[var]
    #[init(val = 8.0)]
    width: f32,
    /// Turn per beat (radians); negative turns counterclockwise.
    #[var]
    #[init(val = std::f32::consts::FRAC_PI_4)]
    step: f32,
    /// Beats each turn takes, starting on the beat.
    #[var]
    #[init(val = 0.35)]
    move_beats: f64,

    driver: BeatDriver,
    origin_angle: f32,
    /// Song beat the blades lit at (they extend over the first beat).
    lit_at: Option<f64>,
    base: Base<Node2D>,
}

#[godot_api]
impl SpinBladeComponent {
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        0.0
    }

    #[func]
    fn get_threat_radius(&self) -> f32 {
        self.length
    }

    #[func]
    fn component_danger_shapes(&self) -> PackedFloat32Array {
        encode_shapes(&self.blade_shapes())
    }

    /// Current blade angle (radians) of blade 0.
    #[func]
    fn get_angle(&self) -> f32 {
        self.angle()
    }
}

impl SpinBladeComponent {
    fn angle(&self) -> f32 {
        let beat = self.driver.beat;
        let whole = beat.floor();
        let turn = smoothstep(((beat - whole) / self.move_beats.max(0.01)).min(1.0) as f32);
        self.origin_angle + self.step * (whole as f32 + turn)
    }

    /// 0..1 as the blades ignite over the first beat after spawning.
    fn reach(&self) -> f32 {
        self.lit_at
            .map_or(0.0, |lit| ((self.driver.beat - lit) as f32).clamp(0.0, 1.0))
    }

    fn blade_shapes(&self) -> Vec<DangerShape> {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return Vec::new();
        };
        let reach = self.reach();
        if reach <= 0.0 {
            return Vec::new();
        }
        let center = to_v2(parent.get_global_position());
        let tip = self.inner + (self.length - self.inner) * reach;
        let count = self.blades.max(1);
        (0..count)
            .map(|k| {
                let a = self.angle() + k as f32 * TAU / count as f32;
                let dir = V2::new(a.cos(), a.sin());
                DangerShape::Capsule {
                    a: center + dir * self.inner,
                    b: center + dir * tip,
                    radius: self.width / 2.0,
                    activates_in: 0.0,
                }
            })
            .collect()
    }
}

#[godot_api]
impl INode2D for SpinBladeComponent {
    fn ready(&mut self) {
        self.driver.configure(1.0, 0.0, 0.0);
        self.origin_angle = (self.to_gd().instance_id().to_i64() % 4) as f32 * TAU / 8.0;
        self.base_mut().set_z_index(-1);
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        if let Some(index) = self.driver.tick(&node, delta) {
            let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
            self.lit_at.get_or_insert(action);
            self.signals().acted().emit(action, beat);
        }
        if let Some(parent) = parent_as_node2d(self.base().get_parent()) {
            let shapes = self.blade_shapes();
            hit_players_any(&parent, &shapes);
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let reach = self.reach();
        if reach <= 0.0 {
            return;
        }
        let glow = ENEMY_GLOW;
        let tip = self.inner + (self.length - self.inner) * reach;
        let count = self.blades.max(1);
        let angle = self.angle();
        for k in 0..count {
            let a = angle + k as f32 * TAU / count as f32;
            let dir = Vector2::new(a.cos(), a.sin());
            let (from, to) = (dir * self.inner, dir * tip);
            let width = self.width;
            let halo = Color::from_rgba(glow.r, glow.g, glow.b, 0.3);
            let mut base = self.base_mut();
            base.draw_line_ex(from, to, halo)
                .width(width * 2.4)
                .antialiased(true)
                .done();
            base.draw_line_ex(from, to, glow)
                .width(width)
                .antialiased(true)
                .done();
            base.draw_line_ex(from, to, WHITE_HOT)
                .width(width * 0.4)
                .antialiased(true)
                .done();
            base.draw_circle(to, width * 0.5, WHITE_HOT);
        }
    }
}
