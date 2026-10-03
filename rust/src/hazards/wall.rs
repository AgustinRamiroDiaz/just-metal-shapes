//! Wall: a bar spanning the arena that slides from one edge to the opposite one over
//! the active phase, with a gap the players must fit through. The warning lights the
//! entry edge (leaving the gap dark, framed by white brackets) with chevrons pointing
//! the way it will travel.
//!
//! Params used: `angle` (travel direction, snapped to right/down/left/up), `x` (gap
//! center for walls moving up/down) or `y` (gap center for walls moving left/right),
//! `size` (thickness as a fraction of arena height), `color_index`, `duration_beats`.

use super::geometry::{WallLayout, cardinal, cardinal_vector, wall_layout};
use super::{HazardCore, HazardPhase, encode_shapes, hit_players_any, paint};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Gap width as a fraction of arena height.
pub const GAP_FRACTION: f32 = 0.3;
/// Depth of the warning band along the entry edge.
const WARN_DEPTH: f32 = 70.0;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct WallHazard {
    core: HazardCore,
    dir: u32,
    #[init(val = 40.0)]
    thickness: f32,
    #[init(val = 0.5)]
    gap: f32,
    #[init(val = 216.0)]
    gap_width: f32,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Wall`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut node = WallHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.dir = cardinal(p.angle);
        hazard.thickness = director.arena_length(p.size).max(12.0);
        hazard.gap = if hazard.dir.is_multiple_of(2) {
            p.y
        } else {
            p.x
        };
        hazard.gap_width = director.arena_length(GAP_FRACTION);
    }
    director.add_hazard(node);
}

#[godot_api]
impl WallHazard {
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

impl WallHazard {
    fn layout(&self, progress: f32) -> WallLayout {
        wall_layout(
            &self.core.bounds,
            self.dir,
            self.thickness,
            self.gap,
            self.gap_width,
            progress,
            self.core.active_seconds(),
        )
    }

    fn progress(&self) -> f32 {
        match self.core.phase {
            HazardPhase::Active(p) => p,
            _ => 0.0,
        }
    }

    fn shapes(&self) -> Vec<DangerShape> {
        let layout = self.layout(self.progress());
        let wait = self.core.activates_in();
        layout
            .slabs
            .iter()
            .map(|(center, half)| DangerShape::Rect {
                // Shifted back so advancing by `wait` lands on the start position.
                center: *center - layout.velocity * wait,
                half: *half,
                angle: 0.0,
                velocity: layout.velocity,
                activates_in: wait,
            })
            .collect()
    }

    fn draw_slabs(&self, canvas: &mut Gd<Node2D>, layout: &WallLayout, color: Color, grow: f32) {
        for (center, half) in &layout.slabs {
            let mut half = *half;
            if self.dir.is_multiple_of(2) {
                half.x *= grow;
            } else {
                half.y *= grow;
            }
            let points = paint::rect_points(*center, half, 0.0);
            paint::polygon(canvas, &points, color);
        }
    }

    fn draw_warning(&self, canvas: &mut Gd<Node2D>, progress: f32) {
        let core = &self.core;
        let start = self.layout(0.0);
        let forward = cardinal_vector(self.dir);
        let color = core.color;
        // Band along the entry edge, gap left dark.
        let depth = WARN_DEPTH * (0.4 + 0.6 * progress);
        for (center, half) in &start.slabs {
            let band_center = *center + forward * (self.thickness * 0.5 + depth * 0.5);
            let mut band_half = *half;
            if self.dir.is_multiple_of(2) {
                band_half.x = depth * 0.5;
            } else {
                band_half.y = depth * 0.5;
            }
            let points = paint::rect_points(band_center, band_half, 0.0);
            paint::polygon(
                canvas,
                &points,
                paint::alpha(color, core.warn_alpha(progress)),
            );
            let edge = paint::rect_points(band_center, band_half, 0.0);
            paint::outline(
                canvas,
                &edge,
                paint::alpha(color, 0.4 + 0.4 * progress),
                2.0,
            );
        }
        // Brackets framing the gap.
        let across = V2::new(forward.y.abs(), forward.x.abs());
        let half_gap = self.gap_width * 0.5;
        let gap_line = start.gap_center + forward * (self.thickness * 0.5);
        let bracket = paint::alpha(Color::WHITE, 0.5 + 0.4 * progress);
        for side in [-1.0, 1.0] {
            let corner = gap_line + across * (side * half_gap);
            paint::line(canvas, corner, corner + forward * depth, bracket, 3.0);
        }
        // Chevrons showing the direction of travel, marching in on the beat.
        let march = 30.0 * core.beat_pulse();
        for lane in [-2.0, -1.0, 1.0, 2.0] {
            let base = gap_line + across * (lane * (half_gap + 90.0));
            let tip = base + forward * (depth + 30.0 + 40.0 * progress - march);
            paint::chevron(
                canvas,
                tip,
                forward,
                16.0,
                paint::alpha(color, 0.4 + 0.5 * progress),
                4.0,
            );
        }
    }
}

#[godot_api]
impl INode2D for WallHazard {
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
                for (lag, alpha) in [(0.05, 0.15), (0.025, 0.3)] {
                    let ghost = self.layout((p - lag).max(0.0));
                    self.draw_slabs(&mut canvas, &ghost, paint::alpha(core.color, alpha), 1.0);
                }
                let layout = self.layout(p);
                let grow = 1.0 + 0.6 * (core.pop() - 1.0) + 0.1 * core.beat_pulse();
                self.draw_slabs(&mut canvas, &layout, core.body_color(1.0), grow);
                // Bright leading edge.
                let forward = cardinal_vector(self.dir);
                for (center, half) in &layout.slabs {
                    let along = V2::new(forward.y.abs(), forward.x.abs());
                    let extent = half.x * along.x + half.y * along.y;
                    let front = *center + forward * (self.thickness * 0.5);
                    paint::line(
                        &mut canvas,
                        front - along * extent,
                        front + along * extent,
                        paint::alpha(Color::WHITE, 0.8),
                        3.0,
                    );
                }
            }
            HazardPhase::Done => {}
        }
    }
}
