//! Pulse: a circle that warns with a ring and a fill growing toward the hit beat, then
//! bursts (white flash, scale pop, shockwave) and damages anyone inside for
//! `duration_beats`, shrinking away at the end.
//!
//! Params used: `x`, `y` (center), `size` (radius as a fraction of arena height),
//! `color_index`, `duration_beats`.

use super::geometry::smoothstep;
use super::{HazardCore, HazardPhase, encode_shapes, hit_players, paint, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Active progress after which the pulse shrinks away.
const SHRINK_START: f32 = 0.7;
/// Seconds the shockwave ring takes to expand and fade.
const SHOCKWAVE: f32 = 0.35;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct PulseHazard {
    #[var]
    #[init(val = 80.0)]
    pub radius: f32,
    core: HazardCore,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Pulse`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let mut pulse = PulseHazard::new_alloc();
    {
        let mut hazard = pulse.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.radius = director.arena_length(event.params.size).max(8.0);
    }
    pulse.set_position(director.arena_point(event.params.x, event.params.y));
    director.add_hazard(pulse);
}

#[godot_api]
impl PulseHazard {
    /// Danger records (see `core::danger`) for bots and `DangerField`.
    #[func]
    pub fn danger_shapes(&self) -> PackedFloat32Array {
        encode_shapes(&self.shape().into_iter().collect::<Vec<_>>())
    }

    /// Configures a standalone pulse (tests, previews): song times in seconds.
    #[func]
    pub fn configure(&mut self, spawn_time: f64, hit_time: f64, end_time: f64) {
        self.core.configure(spawn_time, hit_time, end_time);
    }

    #[func]
    pub fn is_active(&self) -> bool {
        self.core.is_active()
    }
}

impl PulseHazard {
    fn scale(&self) -> f32 {
        match self.core.phase {
            HazardPhase::Active(p) if p > SHRINK_START => {
                1.0 - smoothstep((p - SHRINK_START) / (1.0 - SHRINK_START))
            }
            _ => 1.0,
        }
    }

    fn shape(&self) -> Option<DangerShape> {
        let radius = self.radius * self.scale();
        (radius >= 1.0).then(|| DangerShape::Circle {
            center: to_v2(self.base().get_global_position()),
            radius,
            velocity: V2::ZERO,
            activates_in: self.core.activates_in(),
        })
    }
}

#[godot_api]
impl INode2D for PulseHazard {
    fn process(&mut self, delta: f64) {
        match self.core.tick(delta) {
            HazardPhase::Done => {
                self.base_mut().queue_free();
                return;
            }
            HazardPhase::Active(_) => {
                if let Some(shape) = self.shape() {
                    hit_players(&self.to_gd().upcast(), &shape);
                }
            }
            HazardPhase::Telegraph(_) => {}
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let mut canvas: Gd<Node2D> = self.base().clone();
        paint::begin(&mut canvas);
        let center = to_v2(canvas.get_global_position());
        let core = &self.core;
        let radius = self.radius;
        match core.phase {
            HazardPhase::Telegraph(p) => {
                let color = core.color;
                paint::disc(
                    &mut canvas,
                    center,
                    radius * p,
                    paint::alpha(color, core.warn_alpha(p)),
                );
                paint::ring(
                    &mut canvas,
                    center,
                    radius,
                    2.5,
                    paint::alpha(color, 0.35 + 0.55 * p),
                );
                // A second ring closing in on the edge, landing on the hit.
                let inner = radius * (1.0 + 0.6 * (1.0 - p));
                paint::ring(
                    &mut canvas,
                    center,
                    inner,
                    1.5,
                    paint::alpha(color, 0.25 * p),
                );
                paint::disc(
                    &mut canvas,
                    center,
                    3.0 + 3.0 * core.beat_pulse(),
                    paint::alpha(color, 0.8),
                );
            }
            HazardPhase::Active(_) => {
                let since = core.since_hit();
                if since < SHOCKWAVE {
                    let t = since / SHOCKWAVE;
                    paint::ring(
                        &mut canvas,
                        center,
                        radius * (1.0 + 0.8 * t),
                        8.0 * (1.0 - t),
                        paint::alpha(paint::whiten(core.color, 0.5), 1.0 - t),
                    );
                }
                let r = radius * self.scale() * (core.pop() + 0.05 * core.beat_pulse());
                paint::disc(&mut canvas, center, r, core.body_color(1.0));
                paint::ring(
                    &mut canvas,
                    center,
                    r,
                    3.0,
                    paint::alpha(Color::WHITE, 0.55),
                );
            }
            HazardPhase::Done => {}
        }
    }
}
