//! Spikes: a row of triangular spikes along one arena edge that thrusts inward on the
//! hit (overshooting), holds, then retracts. The warning shades the band they will
//! reach and outlines each spike at full length while their tips peek out of the edge.
//!
//! Params used: `variant` (side: `variant % 4`, 0 left, 1 top, 2 right, 3 bottom;
//! `variant & 4` keeps only every other spike), `count` (spikes along the edge),
//! `size` (thrust depth as a fraction of arena height), `color_index`,
//! `duration_beats`.

use super::geometry::{Spike, spike_extension, spike_layout};
use super::{HazardCore, HazardPhase, encode_shapes, hit_players_any, paint};
use crate::core::chart::ChartEvent;
use crate::core::danger::{DangerShape, V2};
use crate::director::LevelDirector;
use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Fraction of full length the tips peek out during the warning.
const PEEK: f32 = 0.12;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct SpikesHazard {
    core: HazardCore,
    side: u32,
    #[init(val = 8)]
    count: u32,
    #[init(val = 150.0)]
    depth: f32,
    comb: bool,
    base: Base<Node2D>,
}

/// `SpawnFn` for `EventKind::Spikes`.
pub fn spawn(director: &mut LevelDirector, event: &ChartEvent) {
    let p = &event.params;
    let mut node = SpikesHazard::new_alloc();
    {
        let mut hazard = node.bind_mut();
        hazard.core = HazardCore::from_event(director, event);
        hazard.side = p.variant % 4;
        hazard.comb = p.variant & 4 != 0;
        hazard.count = p.count.max(2);
        hazard.depth = director.arena_length(p.size).max(20.0);
    }
    director.add_hazard(node);
}

#[godot_api]
impl SpikesHazard {
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

impl SpikesHazard {
    fn spikes(&self, extension: f32) -> Vec<Spike> {
        spike_layout(
            &self.core.bounds,
            self.side,
            self.count,
            self.depth,
            extension,
            self.comb,
        )
    }

    fn extension(&self) -> f32 {
        match self.core.phase {
            HazardPhase::Active(p) => spike_extension(p),
            _ => 0.0,
        }
    }

    fn shapes(&self) -> Vec<DangerShape> {
        match self.core.phase {
            // Warn bots about the full reach.
            HazardPhase::Telegraph(_) => self
                .spikes(1.0)
                .iter()
                .map(|s| s.shape(self.core.activates_in()))
                .collect(),
            HazardPhase::Active(_) => {
                let extension = self.extension();
                if extension < 0.05 {
                    return Vec::new();
                }
                self.spikes(extension)
                    .iter()
                    .map(|s| s.shape(0.0))
                    .collect()
            }
            HazardPhase::Done => Vec::new(),
        }
    }
}

fn triangle(spike: &Spike, width_scale: f32) -> [V2; 3] {
    let across = V2::new(-spike.inward.y, spike.inward.x) * (spike.half_width * width_scale);
    [spike.base + across, spike.tip, spike.base - across]
}

#[godot_api]
impl INode2D for SpikesHazard {
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
                let color = core.color;
                let full = self.spikes(1.0);
                // Shaded band over the reach, spanning the whole edge.
                let edge =
                    spike_layout(&core.bounds, self.side, self.count, self.depth, 1.0, false);
                if let (Some(first), Some(last)) = (edge.first(), edge.last()) {
                    let across = V2::new(-first.inward.y, first.inward.x);
                    let a = first.base - across * first.half_width;
                    let b = last.base + across * last.half_width;
                    let reach = first.inward * (self.depth * p);
                    paint::polygon(
                        &mut canvas,
                        &[a, b, b + reach, a + reach],
                        paint::alpha(color, core.warn_alpha(p) * 0.6),
                    );
                }
                for spike in &full {
                    paint::outline(
                        &mut canvas,
                        &triangle(spike, 0.92),
                        paint::alpha(color, 0.3 + 0.5 * p),
                        2.0,
                    );
                }
                for spike in self.spikes(PEEK * p + 0.04 * core.beat_pulse()) {
                    paint::polygon(
                        &mut canvas,
                        &triangle(&spike, 0.92),
                        paint::alpha(color, 0.9),
                    );
                }
            }
            HazardPhase::Active(_) => {
                let width = 0.92 * (1.0 + 0.15 * (core.pop() - 1.0));
                let color = core.body_color(1.0);
                for spike in self.spikes(self.extension()) {
                    let points = triangle(&spike, width);
                    paint::polygon(&mut canvas, &points, color);
                    paint::outline(&mut canvas, &points, paint::alpha(Color::WHITE, 0.6), 2.0);
                }
            }
            HazardPhase::Done => {}
        }
    }
}
