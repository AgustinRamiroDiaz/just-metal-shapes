//! Spiral: a rotating emitter firing `count` arms of bullets every quarter beat while
//! turning a quarter turn per beat, for the first `EMIT_FRACTION` of its active phase.
//! The warning shows the arms turning slowly in the spin direction.
//!
//! Params used: `x`, `y` (emitter), `count` (arms), `angle` (first arm), `variant` (0
//! clockwise, 1 counter-clockwise), `speed` (arena heights/s), `size` (bullet diameter
//! as a fraction of arena height), `color_index`, `duration_beats`.

use super::geometry::{bullet_position, polygon, spiral_shots};
use super::{FADE_START, HazardCore, HazardPhase, encode_shapes, hit_players_any, paint, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// Share of the active phase spent firing; the rest lets the last bullets fly.
const EMIT_FRACTION: f32 = 0.55;
/// Shots per beat on each arm.
const SHOTS_PER_BEAT: f32 = 4.0;
const START_RADIUS: f32 = 16.0;
const EMITTER_RADIUS: f32 = 16.0;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct SpiralHazard {
    core: HazardCore,
    origin: V2,
    arms: u32,
    start_angle: f32,
    /// Radians per second (negative: counter-clockwise).
    turn_rate: f32,
    /// `(launch offset after the hit, angle)` of every bullet.
    shots: Vec<(f32, f32)>,
    #[init(val = 9.0)]
    radius: f32,
    #[init(val = 250.0)]
    speed: f32,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Spiral`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut node = SpiralHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        let beat = hazard.core.grid.seconds_per_beat() as f32;
        let sign = if p.variant.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        hazard.origin = to_v2(director.arena_point(p.x, p.y));
        hazard.arms = p.count.max(1);
        hazard.start_angle = p.angle;
        hazard.turn_rate = sign * FRAC_PI_2 / beat;
        let emit = hazard.core.active_seconds() * EMIT_FRACTION;
        hazard.shots = spiral_shots(
            hazard.arms,
            p.angle,
            hazard.turn_rate,
            beat / SHOTS_PER_BEAT,
            emit,
        );
        hazard.radius = (director.arena_length(p.size) * 0.5).max(4.0);
        hazard.speed = director.arena_length(p.speed).max(60.0);
    }
    director.add_hazard(node);
}

#[godot_api]
impl SpiralHazard {
    /// Danger records (see `core::danger`) for bots and `DangerField`.
    #[func]
    pub fn danger_shapes(&self) -> PackedFloat32Array {
        encode_shapes(&self.shapes())
    }

    #[func]
    pub fn is_active(&self) -> bool {
        self.core.is_active()
    }
}

impl SpiralHazard {
    fn emitting(&self) -> bool {
        matches!(self.core.phase, HazardPhase::Active(p) if p < EMIT_FRACTION)
    }

    /// Launched bullets still on screen: `(position, velocity)`.
    fn bullets(&self) -> Vec<(V2, V2)> {
        let since = self.core.since_hit();
        self.shots
            .iter()
            .filter(|(offset, _)| *offset <= since)
            .map(|(offset, angle)| {
                (
                    bullet_position(
                        self.origin,
                        *angle,
                        START_RADIUS,
                        self.speed,
                        since - offset,
                    ),
                    V2::from_angle(*angle) * self.speed,
                )
            })
            .filter(|(p, _)| self.core.bounds.contains(*p, self.radius * 2.0))
            .collect()
    }

    fn shapes(&self) -> Vec<DangerShape> {
        let core = &self.core;
        let mut shapes = Vec::new();
        match core.phase {
            HazardPhase::Telegraph(_) => shapes.push(DangerShape::Circle {
                center: self.origin,
                radius: EMITTER_RADIUS + 8.0,
                velocity: V2::ZERO,
                activates_in: core.activates_in(),
            }),
            HazardPhase::Active(p) if p < FADE_START => {
                if self.emitting() {
                    shapes.push(DangerShape::Circle {
                        center: self.origin,
                        radius: EMITTER_RADIUS,
                        velocity: V2::ZERO,
                        activates_in: 0.0,
                    });
                }
                shapes.extend(self.bullets().into_iter().map(|(center, velocity)| {
                    DangerShape::Circle {
                        center,
                        radius: self.radius,
                        velocity,
                        activates_in: 0.0,
                    }
                }));
            }
            _ => {}
        }
        shapes
    }

    /// Current emitter rotation.
    fn spin(&self) -> f32 {
        let since = self.core.since_hit();
        let emit_end = self.core.active_seconds() * EMIT_FRACTION;
        if since < 0.0 {
            // Slow wind-up during the warning.
            self.start_angle + self.turn_rate * 0.25 * since
        } else {
            self.start_angle + self.turn_rate * since.min(emit_end)
        }
    }
}

#[godot_api]
impl INode2D for SpiralHazard {
    fn process(&mut self, delta: f64) {
        match self.core.tick(delta) {
            HazardPhase::Done => {
                self.base_mut().queue_free();
                return;
            }
            HazardPhase::Active(_) => {
                let shapes = self.shapes();
                hit_players_any(&self.to_gd().upcast(), &shapes);
            }
            HazardPhase::Telegraph(_) => {}
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let mut canvas: Gd<Node2D> = self.base().clone();
        paint::begin(&mut canvas);
        let core = &self.core;
        let spin = self.spin();
        let arm_step = std::f32::consts::TAU / self.arms as f32;
        match core.phase {
            HazardPhase::Telegraph(p) => {
                let color = core.color;
                let r = EMITTER_RADIUS * (0.6 + 0.6 * p) + 4.0 * core.beat_pulse();
                paint::disc(
                    &mut canvas,
                    self.origin,
                    r + 10.0,
                    paint::alpha(color, core.warn_alpha(p)),
                );
                paint::ring(
                    &mut canvas,
                    self.origin,
                    r + 10.0,
                    2.5,
                    paint::alpha(color, 0.5 + 0.4 * p),
                );
                for arm in 0..self.arms {
                    let dir = V2::from_angle(spin + arm as f32 * arm_step);
                    let from = self.origin + dir * (r + 14.0);
                    paint::line(
                        &mut canvas,
                        from,
                        from + dir * (20.0 + 60.0 * p),
                        paint::alpha(color, 0.3 + 0.5 * p),
                        3.0,
                    );
                }
            }
            HazardPhase::Active(p) => {
                let fade = core.fade(p);
                let color = core.body_color(fade);
                let radius = self.radius * (1.0 + 0.12 * core.beat_pulse());
                for (center, _) in self.bullets() {
                    paint::bullet(&mut canvas, center, radius, color);
                }
                let emitter_alpha = if self.emitting() { 1.0 } else { 0.4 * fade };
                let body = polygon(self.origin, EMITTER_RADIUS * core.pop(), 4, spin);
                paint::polygon(&mut canvas, &body, core.body_color(emitter_alpha));
                paint::outline(
                    &mut canvas,
                    &body,
                    paint::alpha(Color::WHITE, emitter_alpha),
                    2.0,
                );
            }
            HazardPhase::Done => {}
        }
    }
}
