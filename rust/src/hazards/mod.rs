//! JSB-style chart hazards, one file per hazard, plus the helpers they share.
//!
//! Adding a hazard (see `pulse.rs` for the reference implementation):
//! 1. Create `hazards/<name>.rs` with a `Node2D` class that:
//!    - stores a `HazardTiming` (spawn / hit / end song seconds) and a `HazardClock`;
//!    - each frame reads `clock.now(delta)` and `timing.phase(t)`; draws a harmless
//!      warning while `Telegraph`, calls `hit_players` with its shape while `Active`,
//!      and `queue_free()`s on `Done`;
//!    - exposes `#[func] fn danger_shapes(&self) -> PackedFloat32Array` (encode its
//!      `DangerShape`s with `activates_in = timing.activates_in(t)`).
//! 2. Add `pub fn spawn(director: &mut LevelDirector, event: &ChartEvent)` that builds the
//!    node from `event.params` using the director helpers (`arena_point`,
//!    `arena_length`, `hazard_color`, `conductor`, `timing`) and calls
//!    `director.add_hazard(node)` (which joins the `hazards` and `danger` groups).
//! 3. Register it in `register_all` below and allow its `EventKind` in a level's
//!    `pattern_pool` (`level_catalog.rs`).

pub mod pulse;

use crate::conductor::Conductor;
use crate::core::chart::{ChartEvent, EventKind};
use crate::core::danger::{DangerShape, V2};
use crate::core::timing::Timing;
use crate::director::SpawnFn;
use crate::groups;
use godot::classes::Node2D;
use godot::prelude::*;
use std::collections::HashMap;

/// Collision radius used when testing hazards against players (the player body is a
/// 22x22 square).
pub const PLAYER_HIT_RADIUS: f32 = 11.0;
/// A hazard whose song time jumps this far before its spawn time frees itself.
const REWIND_TOLERANCE: f64 = 0.25;

/// Registers every implemented hazard's spawn function.
pub fn register_all(registry: &mut HashMap<EventKind, SpawnFn>) {
    registry.insert(EventKind::Pulse, pulse::spawn);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HazardPhase {
    /// Harmless warning; progress `0..=1` toward the hit.
    Telegraph(f32),
    /// Damaging; progress `0..=1` toward the end.
    Active(f32),
    Done,
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

pub fn to_v2(v: Vector2) -> V2 {
    V2::new(v.x, v.y)
}

pub fn to_vector2(v: V2) -> Vector2 {
    Vector2::new(v.x, v.y)
}

/// Deals 1 damage to every living player touching `shape`. Returns how many were hit
/// (players with invincibility frames still count as touched but take no damage).
pub fn hit_players(node: &Gd<Node2D>, shape: &DangerShape) -> u32 {
    let mut touched = 0;
    for player in node
        .get_tree()
        .get_nodes_in_group(groups::PLAYERS)
        .iter_shared()
    {
        let Ok(mut player) = player.try_cast::<Node2D>() else {
            continue;
        };
        if player.get("is_dead").try_to::<bool>().unwrap_or(false) {
            continue;
        }
        if shape.overlaps_circle(to_v2(player.get_global_position()), PLAYER_HIT_RADIUS) {
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
}
