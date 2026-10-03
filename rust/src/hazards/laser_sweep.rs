//! LaserSweep: a full-length beam that moves while firing.
//!
//! - Rotation (`variant` bit 0 clear): the beam pivots on (`x`, `y`) a quarter turn over
//!   the active phase; `variant` bit 1 set turns counter-clockwise. The opposite
//!   quadrants stay safe.
//! - Translation (`variant` bit 0 set): the beam at (`x`, `y`, `angle`) pushes toward
//!   the arena center, at most `MAX_PUSH` of the arena, `speed` arena heights/s.
//!
//! The warning shows the start position plus where it will travel (an arc with an
//! arrowhead, or chevrons along the push). Params also used: `size` (beam width),
//! `color_index`, `duration_beats`.

use super::geometry::{line_through, smoothstep, sweep_angle, sweep_push};
use super::{HazardCore, HazardPhase, encode_shapes, hit_players_any, paint, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;
use std::f32::consts::FRAC_PI_2;

/// Largest fraction of the arena a translating beam crosses.
const MAX_PUSH: f32 = 0.45;
/// Radius of the rotation guide arc.
const GUIDE_RADIUS: f32 = 90.0;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct LaserSweepHazard {
    core: HazardCore,
    origin: V2,
    angle: f32,
    #[init(val = 36.0)]
    width: f32,
    translate: bool,
    clockwise: bool,
    push_dir: V2,
    travel: f32,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::LaserSweep`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut node = LaserSweepHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.origin = to_v2(director.arena_point(p.x, p.y));
        hazard.angle = p.angle;
        hazard.width = director.arena_length(p.size).max(6.0);
        hazard.translate = p.variant & 1 == 1;
        hazard.clockwise = p.variant & 2 == 0;
        let seconds = hazard.core.active_seconds();
        let travel = director.arena_length(p.speed.max(0.05)) * seconds;
        let (dir, travel) = sweep_push(
            hazard.origin,
            hazard.angle,
            &hazard.core.bounds,
            travel,
            MAX_PUSH,
        );
        hazard.push_dir = dir;
        hazard.travel = travel;
    }
    director.add_hazard(node);
}

#[godot_api]
impl LaserSweepHazard {
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

impl LaserSweepHazard {
    /// Beam point and angle at active progress `p`.
    fn pose(&self, p: f32) -> (V2, f32) {
        if self.translate {
            (
                self.origin + self.push_dir * (self.travel * smoothstep(p)),
                self.angle,
            )
        } else {
            (self.origin, sweep_angle(self.angle, self.clockwise, p))
        }
    }

    fn progress(&self) -> f32 {
        match self.core.phase {
            HazardPhase::Active(p) => p,
            _ => 0.0,
        }
    }

    fn fade(&self) -> f32 {
        self.core.fade(self.progress())
    }

    fn shapes(&self) -> Vec<DangerShape> {
        let (point, angle) = self.pose(self.progress());
        let (a, b) = line_through(point, angle, &self.core.bounds);
        let radius = self.width * 0.5 * self.fade();
        if radius < 1.0 {
            return Vec::new();
        }
        vec![DangerShape::Capsule {
            a,
            b,
            radius,
            activates_in: self.core.activates_in(),
        }]
    }

    fn draw_guide(&self, canvas: &mut Gd<Node2D>, progress: f32) {
        let color = paint::alpha(self.core.color, 0.35 + 0.45 * progress);
        if self.translate {
            let (end, angle) = self.pose(1.0);
            let (ea, eb) = line_through(end, angle, &self.core.bounds);
            paint::line(canvas, ea, eb, paint::alpha(self.core.color, 0.12), 2.0);
            let along = V2::from_angle(angle);
            let size = 14.0 + 4.0 * self.core.beat_pulse();
            for offset in [-260.0, 0.0, 260.0] {
                let base = self.origin + along * offset;
                for step in 1..=3 {
                    let tip = base + self.push_dir * (step as f32 * 26.0 * (0.4 + 0.6 * progress));
                    paint::chevron(canvas, tip, self.push_dir, size, color, 3.0);
                }
            }
        } else {
            let sign = if self.clockwise { 1.0 } else { -1.0 };
            for half in [0.0, std::f32::consts::PI] {
                let start = self.angle + half;
                paint::arc(
                    canvas,
                    self.origin,
                    GUIDE_RADIUS,
                    start,
                    sign * FRAC_PI_2 * progress.max(0.15),
                    4.0,
                    color,
                );
                let end = start + sign * FRAC_PI_2 * progress.max(0.15);
                let tip = self.origin + V2::from_angle(end) * GUIDE_RADIUS;
                let tangent = V2::from_angle(end + sign * FRAC_PI_2);
                paint::chevron(canvas, tip, tangent, 12.0, color, 4.0);
            }
            paint::disc(
                canvas,
                self.origin,
                10.0 + 4.0 * self.core.beat_pulse(),
                color,
            );
        }
    }
}

#[godot_api]
impl INode2D for LaserSweepHazard {
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
        match core.phase {
            HazardPhase::Telegraph(p) => {
                let (a, b) = line_through(self.origin, self.angle, &core.bounds);
                paint::warn_beam(
                    &mut canvas,
                    a,
                    b,
                    self.width,
                    core.color,
                    core.warn_alpha(p),
                    p,
                );
                self.draw_guide(&mut canvas, p);
            }
            HazardPhase::Active(p) => {
                let (point, angle) = self.pose(p);
                let (a, b) = line_through(point, angle, &core.bounds);
                let fade = self.fade();
                // Afterimage trailing the motion.
                let (ghost_point, ghost_angle) = self.pose((p - 0.06).max(0.0));
                let (ga, gb) = line_through(ghost_point, ghost_angle, &core.bounds);
                paint::line(
                    &mut canvas,
                    ga,
                    gb,
                    paint::alpha(core.color, 0.25 * fade),
                    self.width * 0.8,
                );
                let width = self.width * core.pop() * fade;
                paint::beam(&mut canvas, a, b, width, core.body_color(1.0));
                if !self.translate {
                    paint::disc(
                        &mut canvas,
                        point,
                        self.width * 0.8 * core.pop(),
                        core.body_color(fade),
                    );
                    paint::disc(
                        &mut canvas,
                        point,
                        self.width * 0.35,
                        paint::alpha(Color::WHITE, fade),
                    );
                }
            }
            HazardPhase::Done => {}
        }
    }
}
