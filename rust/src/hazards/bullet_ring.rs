//! BulletRing: a radial burst of bullets from a point. The emitter swells with spokes
//! pointing where each bullet will fly; on the hit a shockwave ring fires the bullets
//! outward. One node draws and tests every bullet.
//!
//! Params used: `x`, `y` (origin), `count` (bullets), `angle` (first bullet direction),
//! `variant` (1 = leave a gap around `angle`), `speed` (arena heights/s), `size`
//! (bullet diameter as a fraction of arena height), `color_index`, `duration_beats`.

use super::geometry::{bullet_position, ring_angles};
use super::{FADE_START, HazardCore, HazardPhase, encode_shapes, hit_players_any, paint, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Distance from the origin where bullets appear.
const START_RADIUS: f32 = 18.0;
const EMITTER_RADIUS: f32 = 22.0;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct BulletRingHazard {
    core: HazardCore,
    origin: V2,
    angles: Vec<f32>,
    #[init(val = 9.0)]
    radius: f32,
    #[init(val = 250.0)]
    speed: f32,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::BulletRing`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut node = BulletRingHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.origin = to_v2(director.arena_point(p.x, p.y));
        hazard.angles = ring_angles(p.count.max(3), p.angle, p.variant % 2 == 1);
        hazard.radius = (director.arena_length(p.size) * 0.5).max(4.0);
        hazard.speed = director.arena_length(p.speed).max(60.0);
    }
    director.add_hazard(node);
}

#[godot_api]
impl BulletRingHazard {
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

impl BulletRingHazard {
    fn bullets(&self) -> impl Iterator<Item = (V2, V2)> + '_ {
        let since = self.core.since_hit();
        self.angles.iter().map(move |angle| {
            let velocity = V2::from_angle(*angle) * self.speed;
            (
                bullet_position(self.origin, *angle, START_RADIUS, self.speed, since),
                velocity,
            )
        })
    }

    fn harmful(&self) -> bool {
        matches!(self.core.phase, HazardPhase::Active(p) if p < FADE_START)
    }

    fn shapes(&self) -> Vec<DangerShape> {
        let core = &self.core;
        match core.phase {
            HazardPhase::Telegraph(_) => {
                // Bullets placed so that advancing them by `activates_in` lands them on
                // their launch points.
                let wait = core.activates_in();
                let mut shapes = vec![DangerShape::Circle {
                    center: self.origin,
                    radius: EMITTER_RADIUS,
                    velocity: V2::ZERO,
                    activates_in: wait,
                }];
                shapes.extend(self.angles.iter().map(|angle| {
                    let velocity = V2::from_angle(*angle) * self.speed;
                    DangerShape::Circle {
                        center: bullet_position(self.origin, *angle, START_RADIUS, 0.0, 0.0)
                            - velocity * wait,
                        radius: self.radius,
                        velocity,
                        activates_in: wait,
                    }
                }));
                shapes
            }
            HazardPhase::Active(_) if self.harmful() => self
                .bullets()
                .filter(|(p, _)| core.bounds.contains(*p, self.radius * 2.0))
                .map(|(center, velocity)| DangerShape::Circle {
                    center,
                    radius: self.radius,
                    velocity,
                    activates_in: 0.0,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn all_gone(&self) -> bool {
        self.bullets()
            .all(|(p, _)| !self.core.bounds.contains(p, self.radius * 3.0))
    }
}

#[godot_api]
impl INode2D for BulletRingHazard {
    fn process(&mut self, delta: f64) {
        match self.core.tick(delta) {
            HazardPhase::Done => {
                self.base_mut().queue_free();
                return;
            }
            HazardPhase::Active(_) => {
                if self.all_gone() {
                    self.base_mut().queue_free();
                    return;
                }
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
                let color = core.color;
                let swell = EMITTER_RADIUS * (0.5 + 0.5 * p) + 5.0 * core.beat_pulse();
                paint::disc(
                    &mut canvas,
                    self.origin,
                    swell,
                    paint::alpha(color, core.warn_alpha(p)),
                );
                paint::ring(
                    &mut canvas,
                    self.origin,
                    swell,
                    2.5,
                    paint::alpha(color, 0.5 + 0.4 * p),
                );
                let spoke = 10.0 + 34.0 * p;
                for angle in &self.angles {
                    let dir = V2::from_angle(*angle);
                    let from = self.origin + dir * (swell + 6.0);
                    paint::line(
                        &mut canvas,
                        from,
                        from + dir * spoke,
                        paint::alpha(color, 0.3 + 0.5 * p),
                        3.0,
                    );
                }
            }
            HazardPhase::Active(p) => {
                let fade = core.fade(p);
                let since = core.since_hit();
                if since < 0.3 {
                    let t = since / 0.3;
                    paint::ring(
                        &mut canvas,
                        self.origin,
                        EMITTER_RADIUS + 220.0 * t,
                        6.0 * (1.0 - t),
                        paint::alpha(paint::whiten(core.color, 0.5), 1.0 - t),
                    );
                }
                let radius =
                    self.radius * (1.0 + 0.3 * (core.pop() - 1.0) + 0.12 * core.beat_pulse());
                let color = core.body_color(fade);
                for (center, _) in self.bullets() {
                    if core.bounds.contains(center, radius * 2.0) {
                        paint::bullet(&mut canvas, center, radius, color);
                    }
                }
            }
            HazardPhase::Done => {}
        }
    }
}
