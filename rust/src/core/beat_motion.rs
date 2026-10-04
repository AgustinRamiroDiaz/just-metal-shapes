//! Beat-locked enemy timing and movement math.
//!
//! Every enemy action happens on a `Cadence` (every N beats, plus an offset) computed
//! from the song beat, never from counted signals, so enemies stay on the grid through
//! frame drops, pauses, checkpoint rewinds and seeks. Movement shapes (hops, steps,
//! surges, bounces) are closed-form functions of the beat.

use crate::core::danger::V2;
use std::f32::consts::{PI, TAU};

/// Actions on beats `offset + k * every` for integer `k`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cadence {
    pub every: f64,
    pub offset: f64,
}

impl Default for Cadence {
    fn default() -> Self {
        Self::new(4.0, 0.0)
    }
}

impl Cadence {
    /// The shortest allowed period, a quarter beat.
    pub const MIN_EVERY: f64 = 0.25;

    pub fn new(every: f64, offset: f64) -> Self {
        Self {
            every: every.max(Self::MIN_EVERY),
            offset,
        }
    }

    /// Index of the latest action beat at or before `beat`.
    pub fn index_at(&self, beat: f64) -> i64 {
        ((beat - self.offset) / self.every + 1e-9).floor() as i64
    }

    pub fn beat_of(&self, index: i64) -> f64 {
        self.offset + index as f64 * self.every
    }

    /// Index of the first action beat at or after `beat`.
    pub fn first_index_from(&self, beat: f64) -> i64 {
        ((beat - self.offset) / self.every - 1e-9).ceil() as i64
    }

    /// Position within the current period, `0..1` (0 right on an action beat).
    pub fn phase(&self, beat: f64) -> f64 {
        ((beat - self.offset) / self.every).rem_euclid(1.0)
    }
}

/// Fires each cadence action once as the song beat passes it.
///
/// The first action comes at least `lead` beats plus the wind-up after `start` (so a
/// fresh enemy settles in, then shows its full wind-up). A backward jump (rewind, seek) restarts from the new beat; a
/// forward jump skips actions that are more than `MAX_LATE` beats old instead of
/// firing a burst of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CadenceTracker {
    pub cadence: Cadence,
    /// Beats of wind-up before each action.
    pub windup: f64,
    /// Index of the last action that fired (or was skipped).
    last: i64,
    previous_beat: f64,
    started: bool,
}

impl Default for CadenceTracker {
    fn default() -> Self {
        Self::new(Cadence::default(), 1.0)
    }
}

impl CadenceTracker {
    /// Actions older than this many beats are skipped rather than fired late.
    pub const MAX_LATE: f64 = 0.5;

    pub fn new(cadence: Cadence, windup: f64) -> Self {
        Self {
            cadence,
            windup: windup.max(0.0),
            last: i64::MIN,
            previous_beat: f64::NEG_INFINITY,
            started: false,
        }
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Restarts at `beat`: the next action is the first one at least `lead + windup`
    /// beats away.
    pub fn start(&mut self, beat: f64, lead: f64) {
        let lead = lead.max(0.0) + self.windup;
        self.last = self.cadence.first_index_from(beat + lead) - 1;
        self.previous_beat = beat;
        self.started = true;
    }

    /// Advances to `beat`; returns the action index that fires now, if any.
    pub fn update(&mut self, beat: f64, lead: f64) -> Option<i64> {
        if !self.started || beat < self.previous_beat - 1e-3 {
            self.start(beat, lead);
            return None;
        }
        self.previous_beat = beat;
        let index = self.cadence.index_at(beat);
        if index <= self.last {
            return None;
        }
        self.last = index;
        (beat - self.cadence.beat_of(index) <= Self::MAX_LATE).then_some(index)
    }

    pub fn next_index(&self) -> i64 {
        self.last.saturating_add(1)
    }

    pub fn next_beat(&self) -> f64 {
        self.cadence.beat_of(self.next_index())
    }

    pub fn last_index(&self) -> i64 {
        self.last
    }

    /// Beats until the next action (0 when it is due).
    pub fn beats_until_next(&self, beat: f64) -> f64 {
        (self.next_beat() - beat).max(0.0)
    }

    /// Wind-up progress toward the next action: 0 before the wind-up window, rising
    /// to 1 on the action beat.
    pub fn windup_progress(&self, beat: f64) -> f32 {
        if !self.started || self.windup <= 0.0 {
            return 0.0;
        }
        (1.0 - self.beats_until_next(beat) / self.windup).clamp(0.0, 1.0) as f32
    }

    /// True while inside the wind-up window of the next action.
    pub fn in_windup(&self, beat: f64) -> bool {
        self.started && self.windup > 0.0 && self.beats_until_next(beat) <= self.windup
    }
}

pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Ease-out that overshoots past 1 by about `10% * s / 1.7` before settling.
pub fn ease_out_back(t: f32, s: f32) -> f32 {
    let t = t.clamp(0.0, 1.0) - 1.0;
    1.0 + t * t * ((s + 1.0) * t + s)
}

/// A damped spring kick: 0 at `t = 0`, a peak early, ringing down to 0.
pub fn spring_kick(t: f32, frequency: f32, damping: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    (-t * damping).exp() * (t * frequency * TAU).sin()
}

/// Shape of a hop that lands on a beat, in beats: crouch, then airtime ending on the
/// landing beat, then settle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HopShape {
    pub crouch: f64,
    pub air: f64,
    pub settle: f64,
}

impl Default for HopShape {
    fn default() -> Self {
        Self {
            crouch: 0.55,
            air: 0.45,
            settle: 0.5,
        }
    }
}

/// Where a hop is at a moment: `travel` along the hop (0 start, 1 target, briefly
/// past 1 after landing), `lift` (0..1 height), `squash` (1 rest, <1 flattened,
/// >1 stretched along the travel axis).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HopPose {
    pub travel: f32,
    pub lift: f32,
    pub squash: f32,
}

impl HopShape {
    /// Beats of wind-up (crouch plus airtime) before the landing beat.
    pub fn lead(&self) -> f64 {
        self.crouch + self.air
    }

    /// Pose at `u` beats relative to the landing beat (negative before it).
    pub fn pose(&self, u: f64) -> HopPose {
        let takeoff = -self.air;
        let crouch_start = takeoff - self.crouch;
        if u < crouch_start {
            HopPose {
                travel: 0.0,
                lift: 0.0,
                squash: 1.0,
            }
        } else if u < takeoff {
            // Anticipation: sink and flatten.
            let t = ((u - crouch_start) / self.crouch.max(1e-6)) as f32;
            HopPose {
                travel: -0.06 * smoothstep(t),
                lift: 0.0,
                squash: 1.0 - 0.28 * smoothstep(t),
            }
        } else if u < 0.0 {
            let t = ((u - takeoff) / self.air.max(1e-6)) as f32;
            HopPose {
                travel: -0.06 + 1.06 * ease_out_cubic(t),
                lift: 4.0 * t * (1.0 - t),
                squash: 1.0 + 0.3 * (1.0 - (2.0 * t - 1.0).abs()),
            }
        } else if u < self.settle {
            // Landing: overshoot a little along the hop and squash flat, then settle.
            let t = (u / self.settle.max(1e-6)) as f32;
            HopPose {
                travel: 1.0 + 0.08 * spring_kick(t, 1.0, 4.0),
                lift: 0.0,
                squash: 1.0 - 0.35 * (1.0 - t).powi(2) * (t * PI * 1.5).cos(),
            }
        } else {
            HopPose {
                travel: 1.0,
                lift: 0.0,
                squash: 1.0,
            }
        }
    }
}

/// Speed multiplier for beat surges at `phase` (0..1 through a beat): a push right on
/// the beat that decays toward a slow glide. Averages 1 over a beat.
pub fn surge(phase: f64) -> f32 {
    const FLOOR: f32 = 0.35;
    const DECAY: f32 = 4.0;
    let p = phase.rem_euclid(1.0) as f32;
    let peak = (1.0 - FLOOR) * DECAY / (1.0 - (-DECAY).exp());
    FLOOR + peak * (-p * DECAY).exp()
}

/// Steps taken by `beat` for a mover that steps once per beat from `start_beat`, each
/// step eased over the last `move_beats` before its beat (so it lands on the beat).
pub fn steps_at(beat: f64, start_beat: f64, move_beats: f64) -> f64 {
    let elapsed = beat - start_beat;
    if elapsed <= 0.0 {
        return 0.0;
    }
    let whole = elapsed.floor();
    let frac = elapsed - whole;
    let m = move_beats.clamp(0.01, 1.0);
    let t = ((frac - (1.0 - m)) / m).clamp(0.0, 1.0);
    whole + ease_out_back(t as f32, 1.2) as f64
}

/// Reflects `x` into `[lo, hi]` as if bouncing off both ends (a triangle wave).
pub fn fold(x: f32, lo: f32, hi: f32) -> f32 {
    let span = hi - lo;
    if span <= 0.0 {
        return lo;
    }
    let m = (x - lo).rem_euclid(2.0 * span);
    lo + if m <= span { m } else { 2.0 * span - m }
}

/// Position of a bouncer that has travelled `distance` along `direction` from `origin`
/// inside the box `lo..hi`, reflecting off the edges.
pub fn bounce_position(origin: V2, direction: V2, distance: f32, lo: V2, hi: V2) -> V2 {
    let raw = origin + direction * distance;
    V2::new(fold(raw.x, lo.x, hi.x), fold(raw.y, lo.y, hi.y))
}

/// Index into a color list of `n` entries after `beat`: the color advances every
/// `every_beats`, starting from `seed`. Changes land on multiples of `every_beats`.
pub fn cycle_index(beat: f64, every_beats: f64, seed: usize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let step = (beat / every_beats.max(Cadence::MIN_EVERY)).floor() as i64;
    (seed as i64 + step).rem_euclid(n as i64) as usize
}

/// Moves `velocity` toward `desired` by at most `accel * dt` (inertia: turns become
/// arcs instead of snaps).
pub fn steer(velocity: V2, desired: V2, accel: f32, dt: f32) -> V2 {
    let delta = desired - velocity;
    let max = accel * dt;
    let length = delta.length();
    if length <= max || length < 1e-6 {
        desired
    } else {
        velocity + delta * (max / length)
    }
}

/// Unit-ish push away from neighbors closer than `radius` (stronger when closer).
pub fn separation(position: V2, neighbors: &[V2], radius: f32) -> V2 {
    let mut push = V2::ZERO;
    for other in neighbors {
        let away = position - *other;
        let distance = away.length();
        if distance < 1e-3 || distance >= radius {
            continue;
        }
        push = push + away * ((1.0 - distance / radius) / distance);
    }
    push
}

/// Heading toward a target offset by `arc` radians, fading to straight as the target
/// gets close (`full_at` px or farther gives the full arc). Chasers circle in.
pub fn arc_heading(to_target: V2, arc: f32, full_at: f32) -> V2 {
    let distance = to_target.length();
    if distance < 1e-6 {
        return V2::ZERO;
    }
    let weight = (distance / full_at.max(1.0)).clamp(0.0, 1.0);
    (to_target * (1.0 / distance)).rotated(arc * weight)
}

/// Angles (radians) of a ring of `count` evenly spaced bullets with `gap` consecutive
/// bullets left out, centered on `gap_angle`.
pub fn ring_angles(count: u32, gap: u32, gap_angle: f32) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    let step = TAU / count as f32;
    let gap = gap.min(count.saturating_sub(1));
    // Slots sit at `gap_angle + rel * step`, offset by half a slot for an even gap so
    // the gap is symmetric around `gap_angle`; the `gap` slots nearest it stay empty.
    let shift = if gap.is_multiple_of(2) { 0.5 } else { 0.0 };
    let half = gap as f32 / 2.0;
    (0..count)
        .filter_map(|k| {
            let mut rel = k as f32 + shift;
            if rel > count as f32 / 2.0 {
                rel -= count as f32;
            }
            (rel.abs() >= half).then_some(gap_angle + rel * step)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn cadence_indices_and_phase() {
        let c = Cadence::new(4.0, 2.0);
        assert_eq!(c.index_at(2.0), 0);
        assert_eq!(c.index_at(5.99), 0);
        assert_eq!(c.index_at(6.0), 1);
        assert_eq!(c.index_at(1.0), -1);
        assert_eq!(c.beat_of(3), 14.0);
        assert_eq!(c.first_index_from(6.0), 1);
        assert_eq!(c.first_index_from(6.1), 2);
        assert!((c.phase(4.0) - 0.5).abs() < 1e-9);
        assert_eq!(Cadence::new(0.0, 0.0).every, Cadence::MIN_EVERY);
    }

    /// Feeds beats in small steps and returns the beats at which actions fired.
    fn fire_beats(tracker: &mut CadenceTracker, from: f64, to: f64, lead: f64) -> Vec<f64> {
        let mut fired = Vec::new();
        let mut beat = from;
        while beat <= to {
            if let Some(index) = tracker.update(beat, lead) {
                fired.push(tracker.cadence.beat_of(index));
            }
            beat += 0.05;
        }
        fired
    }

    #[test]
    fn tracker_fires_once_per_action_after_the_lead() {
        let mut tracker = CadenceTracker::new(Cadence::new(2.0, 0.0), 1.0);
        let fired = fire_beats(&mut tracker, 9.5, 16.2, 1.0);
        // Spawned at 9.5: beat 10 is only half a beat away, so the first is 12.
        assert_eq!(fired, vec![12.0, 14.0, 16.0]);
    }

    #[test]
    fn tracker_windup_rises_to_the_action() {
        let mut tracker = CadenceTracker::new(Cadence::new(4.0, 0.0), 1.0);
        tracker.update(1.0, 0.0);
        assert_eq!(tracker.next_beat(), 4.0);
        assert_eq!(tracker.windup_progress(2.0), 0.0);
        assert!(near(tracker.windup_progress(3.5), 0.5));
        assert!(tracker.in_windup(3.5));
        assert!(!tracker.in_windup(2.5));
    }

    #[test]
    fn tracker_restarts_after_a_rewind() {
        let mut tracker = CadenceTracker::new(Cadence::new(4.0, 0.0), 1.0);
        let fired = fire_beats(&mut tracker, 0.0, 9.0, 0.0);
        assert_eq!(fired, vec![4.0, 8.0]);
        // Rewound to beat 2.5: the action at 4 plays again, with its wind-up.
        assert_eq!(tracker.update(2.5, 0.0), None);
        let fired = fire_beats(&mut tracker, 2.55, 4.5, 0.0);
        assert_eq!(fired, vec![4.0]);
    }

    #[test]
    fn tracker_skips_stale_actions_on_a_forward_jump() {
        let mut tracker = CadenceTracker::new(Cadence::new(1.0, 0.0), 0.0);
        tracker.update(0.0, 0.0);
        assert_eq!(tracker.update(0.2, 0.0), Some(0));
        // Jumped from 0.2 to 7.8: beats 1..7 are stale, nothing fires.
        assert_eq!(tracker.update(7.8, 0.0), None);
        assert_eq!(tracker.update(8.1, 0.0), Some(8));
    }

    #[test]
    fn hop_pose_lands_on_the_beat() {
        let hop = HopShape::default();
        let before = hop.pose(-hop.lead() - 0.1);
        assert_eq!(before.travel, 0.0);
        let crouch = hop.pose(-hop.air - 0.01);
        assert!(crouch.squash < 0.8, "crouch flattens: {crouch:?}");
        assert!(crouch.travel < 0.0, "crouch leans back");
        let mid_air = hop.pose(-hop.air / 2.0);
        assert!(mid_air.lift > 0.9 && mid_air.squash > 1.2, "{mid_air:?}");
        let landing = hop.pose(0.0);
        assert!(near(landing.travel, 1.0));
        assert!(landing.squash < 0.7, "landing squashes: {landing:?}");
        let overshoot = hop.pose(hop.settle * 0.25);
        assert!(overshoot.travel > 1.0, "overshoots: {overshoot:?}");
        let rest = hop.pose(hop.settle + 0.1);
        assert_eq!(rest.travel, 1.0);
        assert_eq!(rest.squash, 1.0);
    }

    #[test]
    fn surge_peaks_on_the_beat_and_averages_one() {
        let n = 1000;
        let mean: f32 = (0..n).map(|i| surge(i as f64 / n as f64)).sum::<f32>() / n as f32;
        assert!((mean - 1.0).abs() < 0.01, "mean {mean}");
        assert!(surge(0.0) > 2.5);
        assert!(surge(0.95) < 0.5);
    }

    #[test]
    fn steps_land_on_beats() {
        assert_eq!(steps_at(10.0, 10.0, 0.3), 0.0);
        assert_eq!(steps_at(10.5, 10.0, 0.3), 0.0);
        assert!((steps_at(11.0, 10.0, 0.3) - 1.0).abs() < 1e-6);
        assert!((steps_at(13.0, 10.0, 0.3) - 3.0).abs() < 1e-6);
        // Mid-step it is between 0 and a slight overshoot.
        let mid = steps_at(10.85, 10.0, 0.3);
        assert!(mid > 0.3 && mid < 1.1, "{mid}");
    }

    #[test]
    fn fold_reflects_off_both_edges() {
        assert!(near(fold(5.0, 0.0, 10.0), 5.0));
        assert!(near(fold(12.0, 0.0, 10.0), 8.0));
        assert!(near(fold(-3.0, 0.0, 10.0), 3.0));
        assert!(near(fold(23.0, 0.0, 10.0), 3.0));
        let p = bounce_position(
            V2::new(90.0, 50.0),
            V2::new(1.0, 1.0),
            30.0,
            V2::ZERO,
            V2::new(100.0, 100.0),
        );
        assert!(near(p.x, 80.0) && near(p.y, 80.0), "{p:?}");
    }

    #[test]
    fn cycle_index_advances_on_its_period() {
        assert_eq!(cycle_index(0.0, 8.0, 0, 3), 0);
        assert_eq!(cycle_index(7.9, 8.0, 0, 3), 0);
        assert_eq!(cycle_index(8.0, 8.0, 0, 3), 1);
        assert_eq!(cycle_index(24.0, 8.0, 1, 3), 1);
        assert_eq!(cycle_index(-1.0, 8.0, 0, 3), 2);
        assert_eq!(cycle_index(5.0, 8.0, 0, 0), 0);
    }

    #[test]
    fn steering_has_inertia() {
        let v = steer(V2::new(100.0, 0.0), V2::new(0.0, 100.0), 100.0, 0.1);
        assert!(v.x > 80.0 && v.y > 5.0 && v.y < 15.0, "{v:?}");
        let v = steer(V2::new(0.0, 0.0), V2::new(1.0, 0.0), 100.0, 0.1);
        assert_eq!(v, V2::new(1.0, 0.0));
    }

    #[test]
    fn separation_pushes_apart() {
        let push = separation(V2::ZERO, &[V2::new(10.0, 0.0), V2::new(500.0, 0.0)], 60.0);
        assert!(push.x < 0.0 && push.y == 0.0);
        assert_eq!(separation(V2::ZERO, &[V2::ZERO], 60.0), V2::ZERO);
    }

    #[test]
    fn arc_heading_straightens_up_close() {
        let far = arc_heading(V2::new(400.0, 0.0), 0.5, 300.0);
        assert!(near(far.y, 0.5f32.sin()));
        let close = arc_heading(V2::new(30.0, 0.0), 0.5, 300.0);
        assert!(close.y < far.y * 0.2);
        assert_eq!(arc_heading(V2::ZERO, 0.5, 300.0), V2::ZERO);
    }

    #[test]
    fn ring_leaves_a_gap_around_its_angle() {
        let angles = ring_angles(12, 3, 0.0);
        assert_eq!(angles.len(), 9);
        let step = TAU / 12.0;
        for a in &angles {
            let wrapped = (a + PI).rem_euclid(TAU) - PI;
            assert!(
                wrapped.abs() > step * 1.4,
                "bullet at {wrapped} inside the gap"
            );
        }
        let even = ring_angles(12, 2, 1.0);
        assert_eq!(even.len(), 10);
        for a in &even {
            let wrapped = (a - 1.0 + PI).rem_euclid(TAU) - PI;
            assert!(
                wrapped.abs() > step * 0.9,
                "bullet at {wrapped} inside the gap"
            );
        }
        assert_eq!(ring_angles(8, 0, 0.0).len(), 8);
    }
}
