//! Laser: a full-length beam across the arena. A thin strip warns, widening toward the
//! hit; then the beam fires (white flash, width pop), holds, and narrows away.
//!
//! Params used: `x`, `y` (a point on the beam), `angle` (beam direction), `size` (beam
//! width as a fraction of arena height), `color_index`, `duration_beats`.

use super::geometry::{line_through, smoothstep};
use super::{HazardCore, HazardPhase, encode_shapes, hit_players_any, paint, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Active progress after which the beam narrows away.
const SHRINK_START: f32 = 0.7;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct LaserHazard {
    core: HazardCore,
    origin: V2,
    angle: f32,
    #[init(val = 40.0)]
    width: f32,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Laser`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut node = LaserHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.origin = to_v2(director.arena_point(p.x, p.y));
        hazard.angle = p.angle;
        hazard.width = director.arena_length(p.size).max(6.0);
    }
    director.add_hazard(node);
}

#[godot_api]
impl LaserHazard {
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

impl LaserHazard {
    /// Width multiplier: full until `SHRINK_START`, then narrowing to nothing.
    fn thickness(&self) -> f32 {
        match self.core.phase {
            HazardPhase::Active(p) if p > SHRINK_START => {
                1.0 - smoothstep((p - SHRINK_START) / (1.0 - SHRINK_START))
            }
            _ => 1.0,
        }
    }

    fn shapes(&self) -> Vec<DangerShape> {
        let (a, b) = line_through(self.origin, self.angle, &self.core.bounds);
        let radius = self.width * 0.5 * self.thickness();
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
}

#[godot_api]
impl INode2D for LaserHazard {
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
        let (a, b) = line_through(self.origin, self.angle, &core.bounds);
        match core.phase {
            HazardPhase::Telegraph(p) => {
                let color = core.color;
                paint::warn_beam(&mut canvas, a, b, self.width, color, core.warn_alpha(p), p);
                paint::line(&mut canvas, a, b, paint::alpha(color, 0.5 + 0.4 * p), 1.5);
            }
            HazardPhase::Active(_) => {
                let width = self.width * core.pop() * self.thickness();
                let flash = core.flash();
                if flash > 0.0 {
                    paint::line(
                        &mut canvas,
                        a,
                        b,
                        paint::alpha(Color::WHITE, 0.35 * flash),
                        width * 3.0,
                    );
                }
                paint::beam(&mut canvas, a, b, width, core.body_color(1.0));
            }
            HazardPhase::Done => {}
        }
    }
}
