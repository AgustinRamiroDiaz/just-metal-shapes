//! JSB-style chart hazards, one file per hazard, plus the helpers they share.
//!
//! Adding a hazard (see `pulse.rs` for the reference implementation):
//! 1. Create `hazards/<name>.rs` with a `Node2D` class that:
//!    - stores a `HazardCore` (timing, clock, phase, color, beat grid, arena);
//!    - each frame calls `core.tick(delta)`; draws a harmless warning while
//!      `Telegraph`, calls `hit_players` / `hit_players_any` with its shapes while
//!      `Active`, and `queue_free()`s on `Done`;
//!    - exposes `#[func] fn danger_shapes(&self) -> PackedFloat32Array` (encode its
//!      `DangerShape`s with `activates_in = core.activates_in()`).
//!      Keep geometry closed-form in time (`geometry.rs`, unit tested) so seeks and frame
//!      drops never desync it. Many bullets belong to one node that draws them all.
//! 2. Add `pub fn spawn(director: &mut LevelDirector, event: &ChartEvent)` that builds the
//!    node from `event.params` using `HazardCore::from_event` and the director helpers
//!    (`arena_point`, `arena_length`, `hazard_color`) and calls
//!    `director.add_hazard(node)` (which joins the `hazards` and `danger` groups).
//! 3. Register it in `register_all` below and allow its `EventKind` in a level's
//!    `pattern_pool` (`level_catalog.rs`).
//!
//! Visual language (shared through `HazardCore` and `paint`): the warning is a
//! translucent fill plus outline in the level's danger color that grows toward the hit
//! and flickers in its last quarter; the hit flashes white, pops in scale, then
//! settles to solid danger color that pulses on every beat and fades out at the end.

pub mod barrage;
pub mod bomb;
pub mod bullet_ring;
pub mod geometry;
pub mod laser;
pub mod laser_sweep;
pub mod paint;
pub mod pulse;
pub mod spikes;
pub mod spiral;
pub mod wall;

use crate::conductor::Conductor;
use crate::core::chart::{ChartEvent, EventKind};
use crate::core::danger::{DangerShape, V2};
use crate::core::timing::Timing;
use crate::director::{LevelDirector, SpawnFn};
use crate::groups;
use geometry::Bounds;
use godot::classes::Node2D;
use godot::prelude::*;
use std::collections::HashMap;

/// Collision radius used when testing hazards against players (the player body is a
/// 22x22 square).
pub const PLAYER_HIT_RADIUS: f32 = 11.0;
/// A hazard whose song time jumps this far before its spawn time frees itself.
const REWIND_TOLERANCE: f64 = 0.25;
/// Seconds of white flash when a hazard becomes active.
pub const IMPACT_FLASH: f64 = 0.07;
/// Fraction of the active phase after which bullets fade out and stop hurting.
pub const FADE_START: f32 = 0.88;

/// Registers every implemented hazard's spawn function.
pub fn register_all(registry: &mut HashMap<EventKind, SpawnFn>) {
    registry.insert(EventKind::Laser, laser::spawn);
    registry.insert(EventKind::LaserSweep, laser_sweep::spawn);
    registry.insert(EventKind::BulletRing, bullet_ring::spawn);
    registry.insert(EventKind::Spiral, spiral::spawn);
    registry.insert(EventKind::Wall, wall::spawn);
    registry.insert(EventKind::Pulse, pulse::spawn);
    registry.insert(EventKind::Bomb, bomb::spawn);
    registry.insert(EventKind::Spikes, spikes::spawn);
    registry.insert(EventKind::Barrage, barrage::spawn);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HazardPhase {
    /// Harmless warning; progress `0..=1` toward the hit.
    Telegraph(f32),
    /// Damaging; progress `0..=1` toward the end.
    Active(f32),
    Done,
}

impl Default for HazardPhase {
    fn default() -> Self {
        HazardPhase::Telegraph(0.0)
    }
}

/// Absolute song times (seconds) of a hazard's lifecycle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HazardTiming {
    pub spawn_time: f64,
    pub hit_time: f64,
    pub end_time: f64,
}

impl HazardTiming {
    pub fn from_event(timing: &Timing, event: &ChartEvent) -> Self {
        Self {
            spawn_time: timing.beat_to_seconds(event.spawn_beat()),
            hit_time: timing.beat_to_seconds(event.beat),
            end_time: timing.beat_to_seconds(event.end_beat()),
        }
    }

    pub fn phase(&self, t: f64) -> HazardPhase {
        if t < self.spawn_time - REWIND_TOLERANCE || t >= self.end_time {
            HazardPhase::Done
        } else if t < self.hit_time {
            let span = (self.hit_time - self.spawn_time).max(1e-6);
            HazardPhase::Telegraph((((t - self.spawn_time) / span) as f32).clamp(0.0, 1.0))
        } else {
            let span = (self.end_time - self.hit_time).max(1e-6);
            HazardPhase::Active((((t - self.hit_time) / span) as f32).clamp(0.0, 1.0))
        }
    }

    /// Seconds until the hazard can hurt (0 once active).
    pub fn activates_in(&self, t: f64) -> f32 {
        (self.hit_time - t).max(0.0) as f32
    }

    pub fn active_seconds(&self) -> f64 {
        (self.end_time - self.hit_time).max(0.0)
    }
}

/// Song time source for a hazard: the `Conductor` when available, otherwise the
/// accumulated frame delta from the spawn time (standalone hazards in tests).
#[derive(Clone, Default)]
pub struct HazardClock {
    conductor: Option<Gd<Conductor>>,
    fallback: f64,
}

impl HazardClock {
    pub fn new(conductor: Option<Gd<Conductor>>, start_time: f64) -> Self {
        Self {
            conductor,
            fallback: start_time,
        }
    }

    pub fn now(&mut self, delta: f64) -> f64 {
        match &self.conductor {
            Some(conductor) if conductor.is_instance_valid() => conductor.bind().song_time(),
            _ => {
                self.fallback += delta;
                self.fallback
            }
        }
    }
}

/// State every hazard node shares: lifecycle timing, clock, current phase, color, the
/// song's beat grid (for beat-synced pulses) and the arena bounds.
#[derive(Clone)]
pub struct HazardCore {
    pub timing: HazardTiming,
    clock: HazardClock,
    pub phase: HazardPhase,
    /// Current song time (seconds).
    pub time: f64,
    pub color: Color,
    pub grid: Timing,
    pub bounds: Bounds,
}

impl Default for HazardCore {
    fn default() -> Self {
        Self {
            timing: HazardTiming::default(),
            clock: HazardClock::default(),
            phase: HazardPhase::default(),
            time: 0.0,
            color: Color::from_rgb(1.0, 0.15, 0.5),
            grid: Timing::new(120.0, 0.0),
            bounds: Bounds::default(),
        }
    }
}

impl HazardCore {
    pub fn from_event(director: &LevelDirector, event: &ChartEvent) -> Self {
        let grid = director.timing();
        let timing = HazardTiming::from_event(&grid, event);
        let arena = director.arena_rect();
        Self {
            timing,
            clock: HazardClock::new(director.conductor(), timing.spawn_time),
            phase: HazardPhase::Telegraph(0.0),
            time: timing.spawn_time,
            color: director.hazard_color(event.params.color_index),
            grid,
            bounds: Bounds::new(to_v2(arena.position), to_v2(arena.position + arena.size)),
        }
    }

    /// Standalone timing (tests, previews), driven by frame deltas.
    pub fn configure(&mut self, spawn_time: f64, hit_time: f64, end_time: f64) {
        self.timing = HazardTiming {
            spawn_time,
            hit_time,
            end_time,
        };
        self.clock = HazardClock::new(None, spawn_time);
        self.time = spawn_time;
    }

    /// Advances to the current song time and returns the phase.
    pub fn tick(&mut self, delta: f64) -> HazardPhase {
        self.time = self.clock.now(delta);
        self.phase = self.timing.phase(self.time);
        self.phase
    }

    pub fn activates_in(&self) -> f32 {
        self.timing.activates_in(self.time)
    }

    pub fn is_active(&self) -> bool {
        matches!(self.phase, HazardPhase::Active(_))
    }

    /// Seconds since the hit (negative while telegraphing).
    pub fn since_hit(&self) -> f32 {
        (self.time - self.timing.hit_time) as f32
    }

    /// Seconds of the active phase.
    pub fn active_seconds(&self) -> f32 {
        self.timing.active_seconds() as f32
    }

    /// `1` right on each beat, decaying to `0` before the next.
    pub fn beat_pulse(&self) -> f32 {
        let beat = self.grid.seconds_to_beat(self.time);
        (1.0 - beat.rem_euclid(1.0) as f32).powi(4)
    }

    /// White-flash amount (`1` at the hit, `0` after `IMPACT_FLASH`).
    pub fn flash(&self) -> f32 {
        let since = self.since_hit() as f64;
        if since < 0.0 {
            0.0
        } else {
            (1.0 - since / IMPACT_FLASH).clamp(0.0, 1.0) as f32
        }
    }

    /// Scale pop after the hit: overshoots then settles to 1.
    pub fn pop(&self) -> f32 {
        let since = self.since_hit().max(0.0);
        1.0 + 0.45 * (-since * 16.0).exp()
    }

    /// Alpha of the warning fill: rises toward the hit and flickers in the last
    /// quarter of the telegraph.
    pub fn warn_alpha(&self, progress: f32) -> f32 {
        let base = 0.12 + 0.22 * progress;
        if progress > 0.75 && (self.time * 18.0).fract() < 0.5 {
            base + 0.25
        } else {
            base
        }
    }

    /// Opacity while active: full, fading over the last stretch.
    pub fn fade(&self, progress: f32) -> f32 {
        if progress < FADE_START {
            1.0
        } else {
            1.0 - (progress - FADE_START) / (1.0 - FADE_START)
        }
    }

    /// Body color while active: white flash blending into the danger color, brightened
    /// slightly on each beat.
    pub fn body_color(&self, alpha: f32) -> Color {
        let mut color = self.color.lerp(
            Color::WHITE,
            self.flash().max(0.18 * self.beat_pulse()) as f64,
        );
        color.a = alpha;
        color
    }
}

pub fn to_v2(v: Vector2) -> V2 {
    V2::new(v.x, v.y)
}

pub fn to_vector2(v: V2) -> Vector2 {
    Vector2::new(v.x, v.y)
}

fn living_players(node: &Gd<Node2D>) -> Vec<Gd<Node2D>> {
    node.get_tree()
        .get_nodes_in_group(groups::PLAYERS)
        .iter_shared()
        .filter_map(|player| player.try_cast::<Node2D>().ok())
        .filter(|player| !player.get("is_dead").try_to::<bool>().unwrap_or(false))
        .collect()
}

/// Deals 1 damage to every living player touching `shape`. Returns how many were hit
/// (players with invincibility frames still count as touched but take no damage).
pub fn hit_players(node: &Gd<Node2D>, shape: &DangerShape) -> u32 {
    hit_players_any(node, std::slice::from_ref(shape))
}

/// Deals 1 damage to every living player touching any of `shapes` (once per player
/// per frame). Returns how many players were touched.
pub fn hit_players_any(node: &Gd<Node2D>, shapes: &[DangerShape]) -> u32 {
    if shapes.is_empty() {
        return 0;
    }
    let mut touched = 0;
    for mut player in living_players(node) {
        let position = to_v2(player.get_global_position());
        if shapes
            .iter()
            .any(|shape| shape.overlaps_circle(position, PLAYER_HIT_RADIUS))
        {
            touched += 1;
            player.call("take_damage", &[1.0f32.to_variant()]);
        }
    }
    touched
}

/// Encodes shapes for `danger_shapes()`.
pub fn encode_shapes(shapes: &[DangerShape]) -> PackedFloat32Array {
    PackedFloat32Array::from(crate::core::danger::encode(shapes).as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::chart::PatternParams;

    #[test]
    fn phases_follow_song_time() {
        let event = ChartEvent {
            beat: 8.0,
            telegraph_beats: 2.0,
            kind: EventKind::Pulse,
            params: PatternParams {
                duration_beats: 1.0,
                ..PatternParams::default()
            },
        };
        let timing = HazardTiming::from_event(&Timing::new(120.0, 1.0), &event);
        assert_eq!(timing.spawn_time, 4.0);
        assert_eq!(timing.hit_time, 5.0);
        assert_eq!(timing.end_time, 5.5);
        assert_eq!(timing.phase(4.5), HazardPhase::Telegraph(0.5));
        assert_eq!(timing.phase(5.25), HazardPhase::Active(0.5));
        assert_eq!(timing.phase(5.5), HazardPhase::Done);
        assert_eq!(timing.phase(3.0), HazardPhase::Done);
        assert_eq!(timing.activates_in(4.5), 0.5);
        assert_eq!(timing.activates_in(5.2), 0.0);
    }

    #[test]
    fn core_feel_curves() {
        let mut core = HazardCore::default();
        core.configure(0.0, 1.0, 2.0);
        core.time = 1.0;
        assert_eq!(core.flash(), 1.0);
        assert!(core.pop() > 1.4);
        core.time = 1.5;
        assert_eq!(core.flash(), 0.0);
        assert!(core.pop() < 1.01);
        // 120 BPM: beats every 0.5 s.
        core.time = 2.0;
        assert!((core.beat_pulse() - 1.0).abs() < 1e-5);
        core.time = 2.25;
        assert!(core.beat_pulse() < 0.1);
        assert_eq!(core.fade(0.5), 1.0);
        assert_eq!(core.fade(1.0), 0.0);
        assert!(core.warn_alpha(1.0) > core.warn_alpha(0.0));
    }
}
