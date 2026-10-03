//! Bomb: drops from above onto its target while a blast circle fills in, the fuse
//! blinking faster toward the hit. On the hit it explodes (white flash, blast circle,
//! shockwave) and throws `count` shrapnel bullets outward for the rest of its duration.
//!
//! Params used: `x`, `y` (target), `size` (blast radius as a fraction of arena height),
//! `count` (shrapnel bullets), `angle` (first shrapnel direction), `speed` (shrapnel,
//! arena heights/s), `color_index`, `duration_beats`.

use super::geometry::{bomb_drop, bullet_position, polygon, ring_angles};
use super::{FADE_START, HazardCore, HazardPhase, encode_shapes, hit_players_any, paint, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Seconds the blast circle stays dangerous.
const BLAST_SECONDS: f32 = 0.3;
const BODY_RADIUS: f32 = 16.0;
const SHRAPNEL_RADIUS: f32 = 8.0;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct BombHazard {
    core: HazardCore,
    target: V2,
    #[init(val = 140.0)]
    blast: f32,
    shrapnel: Vec<f32>,
    #[init(val = 320.0)]
    speed: f32,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Bomb`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut node = BombHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.target = to_v2(director.arena_point(p.x, p.y));
        hazard.blast = director.arena_length(p.size).max(24.0);
        hazard.shrapnel = if p.count > 1 {
            ring_angles(p.count, p.angle, false)
        } else {
            Vec::new()
        };
        hazard.speed = director.arena_length(p.speed).max(80.0);
    }
    director.add_hazard(node);
}

#[godot_api]
impl BombHazard {
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

impl BombHazard {
    fn blast_seconds(&self) -> f32 {
        BLAST_SECONDS.min(self.core.active_seconds() * 0.5)
    }

    fn exploding(&self) -> bool {
        let since = self.core.since_hit();
        (0.0..self.blast_seconds()).contains(&since)
    }

    fn shards(&self) -> impl Iterator<Item = (V2, V2)> + '_ {
        let since = self.core.since_hit();
        let start = self.blast * 0.35;
        self.shrapnel.iter().map(move |angle| {
            (
                bullet_position(self.target, *angle, start, self.speed, since),
                V2::from_angle(*angle) * self.speed,
            )
        })
    }

    fn shapes(&self) -> Vec<DangerShape> {
        let core = &self.core;
        match core.phase {
            HazardPhase::Telegraph(_) => vec![DangerShape::Circle {
                center: self.target,
                radius: self.blast,
                velocity: V2::ZERO,
                activates_in: core.activates_in(),
            }],
            HazardPhase::Active(p) => {
                let mut shapes = Vec::new();
                if self.exploding() {
                    shapes.push(DangerShape::Circle {
                        center: self.target,
                        radius: self.blast,
                        velocity: V2::ZERO,
                        activates_in: 0.0,
                    });
                }
                if p < FADE_START {
                    shapes.extend(
                        self.shards()
                            .filter(|(c, _)| core.bounds.contains(*c, SHRAPNEL_RADIUS * 2.0))
                            .map(|(center, velocity)| DangerShape::Circle {
                                center,
                                radius: SHRAPNEL_RADIUS,
                                velocity,
                                activates_in: 0.0,
                            }),
                    );
                }
                shapes
            }
            HazardPhase::Done => Vec::new(),
        }
    }

    fn draw_warning(&self, canvas: &mut Gd<Node2D>, p: f32) {
        let core = &self.core;
        let color = core.color;
        paint::disc(
            canvas,
            self.target,
            self.blast * p,
            paint::alpha(color, core.warn_alpha(p)),
        );
        paint::ring(
            canvas,
            self.target,
            self.blast,
            2.5,
            paint::alpha(color, 0.35 + 0.5 * p),
        );
        let cross = 14.0;
        for dir in [V2::new(1.0, 0.0), V2::new(0.0, 1.0)] {
            paint::line(
                canvas,
                self.target - dir * cross,
                self.target + dir * cross,
                paint::alpha(color, 0.6),
                2.0,
            );
        }
        // The bomb itself, falling and spinning, fuse blinking faster near the hit.
        let body = bomb_drop(self.target, &core.bounds, p);
        let spin = (core.time as f32) * 3.0;
        let shell = polygon(body, BODY_RADIUS, 6, spin);
        paint::polygon(
            canvas,
            &shell,
            paint::alpha(Color::from_rgb(0.08, 0.06, 0.1), 0.95),
        );
        paint::outline(canvas, &shell, color, 3.0);
        let blink_rate = 3.0 + 14.0 * p;
        if ((core.time as f32) * blink_rate).fract() < 0.5 {
            paint::disc(
                canvas,
                body + V2::new(0.0, -BODY_RADIUS - 4.0),
                5.0,
                Color::WHITE,
            );
        }
    }
}

#[godot_api]
impl INode2D for BombHazard {
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
        match self.core.phase {
            HazardPhase::Telegraph(p) => self.draw_warning(&mut canvas, p),
            HazardPhase::Active(p) => {
                let core = &self.core;
                let since = core.since_hit();
                let blast_seconds = self.blast_seconds();
                if since < blast_seconds * 2.0 {
                    let t = (since / (blast_seconds * 2.0)).clamp(0.0, 1.0);
                    paint::ring(
                        &mut canvas,
                        self.target,
                        self.blast * (1.0 + 0.9 * t),
                        10.0 * (1.0 - t),
                        paint::alpha(paint::whiten(core.color, 0.5), 1.0 - t),
                    );
                }
                if self.exploding() {
                    let r = self.blast * core.pop();
                    paint::disc(&mut canvas, self.target, r, core.body_color(1.0));
                    paint::disc(
                        &mut canvas,
                        self.target,
                        r * 0.55,
                        paint::alpha(Color::WHITE, 0.8),
                    );
                } else {
                    let t = ((since - blast_seconds) / blast_seconds).clamp(0.0, 1.0);
                    paint::ring(
                        &mut canvas,
                        self.target,
                        self.blast * (1.0 - 0.3 * t),
                        4.0,
                        paint::alpha(core.color, 0.5 * (1.0 - t)),
                    );
                }
                let color = core.body_color(core.fade(p));
                for (center, _) in self.shards() {
                    if core.bounds.contains(center, SHRAPNEL_RADIUS * 2.0) {
                        paint::bullet(&mut canvas, center, SHRAPNEL_RADIUS, color);
                    }
                }
            }
            HazardPhase::Done => {}
        }
    }
}
