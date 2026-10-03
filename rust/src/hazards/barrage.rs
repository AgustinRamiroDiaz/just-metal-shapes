//! Barrage: an aimed volley. When it spawns it locks onto every living player's
//! position (the arena center if there are none); the warning draws dashed aim lines
//! and crosshairs that tighten onto those points. On the hit the emitter fires `count`
//! bullets in quick succession (an eighth of a beat apart), cycling through the targets
//! with a slight fan.
//!
//! Params used: `x`, `y` (emitter, usually on an edge), `count` (bullets), `speed`
//! (arena heights/s), `size` (bullet diameter as a fraction of arena height),
//! `color_index`, `duration_beats`.

use super::geometry::{barrage_shots, bullet_position, line_through, polygon};
use super::{FADE_START, HazardCore, HazardPhase, encode_shapes, hit_players_any, paint, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Shots per beat.
const SHOTS_PER_BEAT: f32 = 8.0;
const START_RADIUS: f32 = 20.0;
const EMITTER_RADIUS: f32 = 18.0;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct BarrageHazard {
    core: HazardCore,
    emitter: V2,
    targets: Vec<V2>,
    /// `(launch offset after the hit, angle)`.
    shots: Vec<(f32, f32)>,
    #[init(val = 9.0)]
    radius: f32,
    #[init(val = 480.0)]
    speed: f32,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Barrage`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut targets: Vec<V2> = director
        .alive_player_positions()
        .into_iter()
        .map(to_v2)
        .collect();
    if targets.is_empty() {
        targets.push(to_v2(director.arena_point(0.5, 0.5)));
    }
    let mut node = BarrageHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.emitter = to_v2(director.arena_point(p.x, p.y));
        let interval = hazard.core.grid.seconds_per_beat() as f32 / SHOTS_PER_BEAT;
        hazard.shots = barrage_shots(hazard.emitter, &targets, p.count.max(1), interval);
        hazard.targets = targets;
        hazard.radius = (director.arena_length(p.size) * 0.5).max(4.0);
        hazard.speed = director.arena_length(p.speed).max(120.0);
    }
    director.add_hazard(node);
}

#[godot_api]
impl BarrageHazard {
    /// Danger records (see `core::danger`) for bots and `DangerField`.
    #[func]
    pub fn danger_shapes(&self) -> PackedFloat32Array {
        encode_shapes(&self.shapes())
    }

    #[func]
    pub fn is_active(&self) -> bool {
        self.core.is_active()
    }

    /// Positions the volley is aimed at (captured at spawn).
    #[func]
    pub fn get_targets(&self) -> PackedVector2Array {
        self.targets
            .iter()
            .map(|t| Vector2::new(t.x, t.y))
            .collect()
    }
}

impl BarrageHazard {
    fn aim(&self, target: V2) -> f32 {
        let d = target - self.emitter;
        d.y.atan2(d.x)
    }

    fn shapes(&self) -> Vec<DangerShape> {
        let core = &self.core;
        match core.phase {
            HazardPhase::Telegraph(_) => self
                .targets
                .iter()
                .map(|target| {
                    let (_, far) = line_through(self.emitter, self.aim(*target), &core.bounds);
                    DangerShape::Capsule {
                        a: self.emitter,
                        b: far,
                        radius: self.radius,
                        activates_in: core.activates_in(),
                    }
                })
                .collect(),
            HazardPhase::Active(p) if p < FADE_START => {
                let since = core.since_hit();
                self.shots
                    .iter()
                    .map(|(offset, angle)| {
                        let velocity = V2::from_angle(*angle) * self.speed;
                        let wait = (offset - since).max(0.0);
                        // Unfired shots sit back along their path so that advancing them
                        // by `wait` puts them at the muzzle.
                        let center = bullet_position(
                            self.emitter,
                            *angle,
                            START_RADIUS,
                            self.speed,
                            since - offset,
                        ) - velocity * wait;
                        DangerShape::Circle {
                            center,
                            radius: self.radius,
                            velocity,
                            activates_in: wait,
                        }
                    })
                    .filter(|shape| match shape {
                        DangerShape::Circle {
                            center,
                            activates_in,
                            ..
                        } => {
                            *activates_in > 0.0 || core.bounds.contains(*center, self.radius * 2.0)
                        }
                        _ => true,
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Launched bullets on screen.
    fn live_bullets(&self) -> Vec<V2> {
        let since = self.core.since_hit();
        self.shots
            .iter()
            .filter(|(offset, _)| *offset <= since)
            .map(|(offset, angle)| {
                bullet_position(
                    self.emitter,
                    *angle,
                    START_RADIUS,
                    self.speed,
                    since - offset,
                )
            })
            .filter(|p| self.core.bounds.contains(*p, self.radius * 2.0))
            .collect()
    }
}

#[godot_api]
impl INode2D for BarrageHazard {
    fn process(&mut self, delta: f64) {
        match self.core.tick(delta) {
            HazardPhase::Done => {
                self.base_mut().queue_free();
                return;
            }
            HazardPhase::Active(_) => {
                let shapes: Vec<DangerShape> = self
                    .shapes()
                    .into_iter()
                    .filter(|s| s.is_active())
                    .collect();
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
        let aim = self.targets.first().map_or(0.0, |target| self.aim(*target));
        let turret = polygon(self.emitter, EMITTER_RADIUS * core.pop(), 3, aim);
        match core.phase {
            HazardPhase::Telegraph(p) => {
                let color = core.color;
                for target in &self.targets {
                    paint::dashed(
                        &mut canvas,
                        self.emitter,
                        *target,
                        paint::alpha(color, 0.25 + 0.5 * p),
                        2.0,
                        10.0,
                    );
                    let lock = 14.0 + 50.0 * (1.0 - p);
                    paint::ring(
                        &mut canvas,
                        *target,
                        lock,
                        2.5,
                        paint::alpha(color, 0.4 + 0.5 * p),
                    );
                    for dir in [V2::new(1.0, 0.0), V2::new(0.0, 1.0)] {
                        for side in [-1.0, 1.0] {
                            let from = *target + dir * (side * (lock + 4.0));
                            paint::line(
                                &mut canvas,
                                from,
                                from + dir * (side * 10.0),
                                paint::alpha(color, 0.8),
                                2.0,
                            );
                        }
                    }
                }
                paint::disc(
                    &mut canvas,
                    self.emitter,
                    EMITTER_RADIUS + 8.0,
                    paint::alpha(color, core.warn_alpha(p)),
                );
                paint::polygon(&mut canvas, &turret, paint::alpha(color, 0.6 + 0.4 * p));
            }
            HazardPhase::Active(p) => {
                let fade = core.fade(p);
                let since = core.since_hit();
                let last_shot = self
                    .shots
                    .iter()
                    .map(|(offset, _)| *offset)
                    .filter(|offset| *offset <= since)
                    .fold(f32::NEG_INFINITY, f32::max);
                let firing = since - last_shot < 0.06;
                paint::polygon(&mut canvas, &turret, core.body_color(fade.max(0.3)));
                if firing {
                    paint::ring(
                        &mut canvas,
                        self.emitter,
                        EMITTER_RADIUS + 10.0,
                        4.0,
                        paint::alpha(Color::WHITE, 0.8),
                    );
                }
                let color = core.body_color(fade);
                for center in self.live_bullets() {
                    paint::bullet(&mut canvas, center, self.radius, color);
                }
            }
            HazardPhase::Done => {}
        }
    }
}
