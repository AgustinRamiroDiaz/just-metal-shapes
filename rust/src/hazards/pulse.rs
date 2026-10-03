//! Pulse: a circle that warns with a ring filling toward the hit beat, then bursts and
//! damages anyone inside for `duration_beats`.
//!
//! Params used: `x`, `y` (center), `size` (radius as a fraction of arena height),
//! `color_index`, `duration_beats`.

use super::{HazardClock, HazardPhase, HazardTiming, encode_shapes, hit_players, to_v2};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;
/// Seconds of white flash when the pulse becomes active.
const IMPACT_FLASH: f64 = 0.08;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct PulseHazard {
    #[var]
    #[init(val = 80.0)]
    pub radius: f32,
    #[var]
    #[init(val = Color::from_rgb(1.0, 0.2, 0.5))]
    pub color: Color,

    timing: HazardTiming,
    clock: HazardClock,
    #[init(val = HazardPhase::Telegraph(0.0))]
    phase: HazardPhase,
    time: f64,

    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Pulse`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let timing = HazardTiming::from_event(&director.timing(), event);
    let mut pulse = PulseHazard::new_alloc();
    {
        let mut hazard = pulse.bind_mut();
        hazard.timing = timing;
        hazard.clock = HazardClock::new(director.conductor(), timing.spawn_time);
        hazard.time = timing.spawn_time;
        hazard.radius = director.arena_length(event.params.size).max(8.0);
        hazard.color = director.hazard_color(event.params.color_index);
    }
    pulse.set_position(director.arena_point(event.params.x, event.params.y));
    director.add_hazard(pulse);
}

#[godot_api]
impl PulseHazard {
    /// Danger records (see `core::danger`) for bots and `DangerField`.
    #[func]
    pub fn danger_shapes(&self) -> PackedFloat32Array {
        encode_shapes(&[self.shape()])
    }

    /// Configures a standalone pulse (tests, previews): song times in seconds.
    #[func]
    pub fn configure(&mut self, spawn_time: f64, hit_time: f64, end_time: f64) {
        self.timing = HazardTiming {
            spawn_time,
            hit_time,
            end_time,
        };
        self.clock = HazardClock::new(None, spawn_time);
        self.time = spawn_time;
    }

    #[func]
    pub fn is_active(&self) -> bool {
        matches!(self.phase, HazardPhase::Active(_))
    }
}

impl PulseHazard {
    fn shape(&self) -> DangerShape {
        DangerShape::Circle {
            center: to_v2(self.base().get_global_position()),
            radius: self.radius,
            velocity: V2::ZERO,
            activates_in: self.timing.activates_in(self.time),
        }
    }
}

#[godot_api]
impl INode2D for PulseHazard {
    fn process(&mut self, delta: f64) {
        self.time = self.clock.now(delta);
        self.phase = self.timing.phase(self.time);
        match self.phase {
            HazardPhase::Done => {
                self.base_mut().queue_free();
                return;
            }
            HazardPhase::Active(_) => {
                let shape = self.shape();
                hit_players(&self.to_gd().upcast(), &shape);
            }
            HazardPhase::Telegraph(_) => {}
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let radius = self.radius;
        let color = self.color;
        match self.phase {
            HazardPhase::Telegraph(progress) => {
                let mut fill = color;
                fill.a = 0.08 + 0.17 * progress;
                self.base_mut()
                    .draw_circle(Vector2::ZERO, radius * progress, fill);
                let mut outline = color;
                outline.a = 0.35 + 0.5 * progress;
                self.base_mut()
                    .draw_arc_ex(Vector2::ZERO, radius, 0.0, TAU, 48, outline)
                    .width(2.0)
                    .done();
            }
            HazardPhase::Active(progress) => {
                let since_hit = self.time - self.timing.hit_time;
                let body = if since_hit < IMPACT_FLASH {
                    Color::WHITE
                } else {
                    let mut faded = color;
                    faded.a = 1.0 - 0.6 * progress;
                    faded
                };
                self.base_mut().draw_circle(Vector2::ZERO, radius, body);
            }
            HazardPhase::Done => {}
        }
    }
}
