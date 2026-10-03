//! Pure hazard geometry (no Godot types), shared by the hazard nodes and unit tested.
//!
//! Every function is a closed-form function of time, so hazards stay correct after
//! frame drops, pauses and seeks: a bullet's position is computed from its launch
//! time, never integrated frame by frame.

use crate::core::danger::{DangerShape, V2};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// The arena in global pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: V2,
    pub max: V2,
}

impl Bounds {
    pub fn new(min: V2, max: V2) -> Self {
        Self { min, max }
    }

    pub fn size(&self) -> V2 {
        self.max - self.min
    }

    pub fn center(&self) -> V2 {
        (self.min + self.max) * 0.5
    }

    pub fn diagonal(&self) -> f32 {
        self.size().length()
    }

    /// Whether `p` lies inside the bounds grown by `margin`.
    pub fn contains(&self, p: V2, margin: f32) -> bool {
        p.x >= self.min.x - margin
            && p.x <= self.max.x + margin
            && p.y >= self.min.y - margin
            && p.y <= self.max.y + margin
    }
}

impl Default for Bounds {
    fn default() -> Self {
        Self::new(V2::ZERO, V2::new(1280.0, 720.0))
    }
}

/// `0..=1` smoothstep.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ease-out with overshoot (peaks about 10% past 1, settles at 1).
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Endpoints of a line through `point` at `angle`, long enough to cross the arena.
pub fn line_through(point: V2, angle: f32, bounds: &Bounds) -> (V2, V2) {
    let reach = bounds.diagonal() * 1.2;
    let dir = V2::from_angle(angle);
    (point - dir * reach, point + dir * reach)
}

/// Directions (radians) of a ring of `count` bullets starting at `angle`. With `gap`,
/// the bullets around `angle` are left out so the ring has an opening to slip through.
pub fn ring_angles(count: u32, angle: f32, gap: bool) -> Vec<f32> {
    let count = count.max(1);
    let gap_half = if gap { (count / 6).max(2) } else { 0 };
    (0..count)
        .filter(|&i| i.min(count - i) >= gap_half)
        .map(|i| angle + i as f32 * TAU / count as f32)
        .collect()
}

/// Position of a bullet launched from `origin` along `angle`, `t` seconds after launch.
pub fn bullet_position(origin: V2, angle: f32, start_radius: f32, speed: f32, t: f32) -> V2 {
    origin + V2::from_angle(angle) * (start_radius + speed * t.max(0.0))
}

/// Shots of a rotating multi-arm emitter: `(launch offset seconds, angle)`.
/// Arms are spread evenly; the pattern turns `turn_rate` radians per second (sign is
/// the direction).
pub fn spiral_shots(
    arms: u32,
    start_angle: f32,
    turn_rate: f32,
    interval: f32,
    emit_seconds: f32,
) -> Vec<(f32, f32)> {
    let arms = arms.max(1);
    let interval = interval.max(1e-3);
    let shots = (emit_seconds.max(0.0) / interval).floor() as u32 + 1;
    let mut out = Vec::with_capacity((shots * arms) as usize);
    for k in 0..shots {
        let t = k as f32 * interval;
        for arm in 0..arms {
            out.push((
                t,
                start_angle + arm as f32 * TAU / arms as f32 + turn_rate * t,
            ));
        }
    }
    out
}

/// Snaps an angle to the nearest cardinal direction: 0 right, 1 down, 2 left, 3 up.
pub fn cardinal(angle: f32) -> u32 {
    ((angle / FRAC_PI_2).round() as i32).rem_euclid(4) as u32
}

/// Unit vector of a cardinal direction.
pub fn cardinal_vector(dir: u32) -> V2 {
    match dir % 4 {
        0 => V2::new(1.0, 0.0),
        1 => V2::new(0.0, 1.0),
        2 => V2::new(-1.0, 0.0),
        _ => V2::new(0.0, -1.0),
    }
}

/// A sliding wall: two axis-aligned slabs with a gap between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallLayout {
    /// `(center, half extents)` of each slab.
    pub slabs: [(V2, V2); 2],
    /// Center of the gap (on the wall's current line).
    pub gap_center: V2,
    /// Wall travel velocity in px/s.
    pub velocity: V2,
}

/// Wall moving along cardinal `dir`, `progress` `0..=1` of the way from just outside
/// one edge to just outside the opposite one over `seconds`. `gap` is the gap center
/// as a fraction along the wall, `gap_width` and `thickness` in pixels.
pub fn wall_layout(
    bounds: &Bounds,
    dir: u32,
    thickness: f32,
    gap: f32,
    gap_width: f32,
    progress: f32,
    seconds: f32,
) -> WallLayout {
    let size = bounds.size();
    let moving_x = dir.is_multiple_of(2);
    let (travel_len, across) = if moving_x {
        (size.x, size.y)
    } else {
        (size.y, size.x)
    };
    let half_gap = (gap_width * 0.5).min(across * 0.45);
    let gap_pos = (gap * across).clamp(half_gap + 4.0, across - half_gap - 4.0);
    let forward = dir < 2;
    let start = if forward {
        -thickness * 0.5
    } else {
        travel_len + thickness * 0.5
    };
    let end = if forward {
        travel_len + thickness * 0.5
    } else {
        -thickness * 0.5
    };
    let along = start + (end - start) * progress.clamp(0.0, 1.0);
    let overhang = 40.0;
    let low_len = gap_pos - half_gap + overhang;
    let high_len = across - (gap_pos + half_gap) + overhang;
    let low_mid = gap_pos - half_gap - low_len * 0.5;
    let high_mid = gap_pos + half_gap + high_len * 0.5;
    let speed = (end - start) / seconds.max(1e-3);
    let make = |line: f32, mid: f32| {
        if moving_x {
            bounds.min + V2::new(line, mid)
        } else {
            bounds.min + V2::new(mid, line)
        }
    };
    let half = |len: f32| {
        if moving_x {
            V2::new(thickness * 0.5, len * 0.5)
        } else {
            V2::new(len * 0.5, thickness * 0.5)
        }
    };
    let velocity = if moving_x {
        V2::new(speed, 0.0)
    } else {
        V2::new(0.0, speed)
    };
    WallLayout {
        slabs: [
            (make(along, low_mid), half(low_len)),
            (make(along, high_mid), half(high_len)),
        ],
        gap_center: make(along, gap_pos),
        velocity,
    }
}

/// One edge spike: base center, tip, half of the base width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spike {
    pub base: V2,
    pub tip: V2,
    pub half_width: f32,
    pub inward: V2,
}

impl Spike {
    /// Danger approximation of the triangle: a capsule along its axis.
    pub fn shape(&self, activates_in: f32) -> DangerShape {
        let radius = self.half_width * 0.45;
        let length = (self.tip - self.base).length();
        let b = self.tip - self.inward * radius.min(length * 0.5);
        DangerShape::Capsule {
            a: self.base - self.inward * 20.0,
            b,
            radius,
            activates_in,
        }
    }
}

/// Spikes along one side (0 left, 1 top, 2 right, 3 bottom), thrust `depth * extension`
/// px inward. With `comb`, only every other spike is present.
pub fn spike_layout(
    bounds: &Bounds,
    side: u32,
    count: u32,
    depth: f32,
    extension: f32,
    comb: bool,
) -> Vec<Spike> {
    let count = count.max(1);
    let size = bounds.size();
    let (origin, along, inward, length) = match side % 4 {
        0 => (bounds.min, V2::new(0.0, 1.0), V2::new(1.0, 0.0), size.y),
        1 => (bounds.min, V2::new(1.0, 0.0), V2::new(0.0, 1.0), size.x),
        2 => (
            V2::new(bounds.max.x, bounds.min.y),
            V2::new(0.0, 1.0),
            V2::new(-1.0, 0.0),
            size.y,
        ),
        _ => (
            V2::new(bounds.min.x, bounds.max.y),
            V2::new(1.0, 0.0),
            V2::new(0.0, -1.0),
            size.x,
        ),
    };
    let step = length / count as f32;
    (0..count)
        .filter(|i| !comb || i % 2 == 0)
        .map(|i| {
            let base = origin + along * (step * (i as f32 + 0.5));
            Spike {
                base,
                tip: base + inward * depth * extension.max(0.0),
                half_width: step * 0.5,
                inward,
            }
        })
        .collect()
}

/// Spike extension over the active phase: a fast overshooting thrust, a hold, then a
/// retract.
pub fn spike_extension(progress: f32) -> f32 {
    const THRUST: f32 = 0.12;
    const RETRACT: f32 = 0.8;
    if progress < THRUST {
        ease_out_back(progress / THRUST)
    } else if progress < RETRACT {
        1.0
    } else {
        1.0 - smoothstep((progress - RETRACT) / (1.0 - RETRACT))
    }
}

/// Aimed volley: `(launch offset seconds, angle)` for `count` shots cycling through
/// `targets`, each pass fanned slightly so a volley covers a small spread.
pub fn barrage_shots(emitter: V2, targets: &[V2], count: u32, interval: f32) -> Vec<(f32, f32)> {
    let fallback = [emitter + V2::new(1.0, 0.0)];
    let targets = if targets.is_empty() {
        &fallback[..]
    } else {
        targets
    };
    (0..count.max(1))
        .map(|k| {
            let target = targets[k as usize % targets.len()];
            let pass = k as usize / targets.len();
            let spread = match pass % 3 {
                0 => 0.0,
                1 => 0.09,
                _ => -0.09,
            };
            let aim = target - emitter;
            (k as f32 * interval, aim.y.atan2(aim.x) + spread)
        })
        .collect()
}

/// Rotating sweep angle: a quarter turn over the active phase, eased.
pub fn sweep_angle(start: f32, clockwise: bool, progress: f32) -> f32 {
    let sign = if clockwise { 1.0 } else { -1.0 };
    start + sign * FRAC_PI_2 * smoothstep(progress)
}

/// Translating sweep: unit push direction from `origin` toward the arena center
/// (perpendicular to the beam at `angle`) and the travel distance, capped so the beam
/// never crosses more than `max_fraction` of the arena.
pub fn sweep_push(
    origin: V2,
    angle: f32,
    bounds: &Bounds,
    travel: f32,
    max_fraction: f32,
) -> (V2, f32) {
    let mut normal = V2::from_angle(angle + FRAC_PI_2);
    if (bounds.center() - origin).dot(normal) < 0.0 {
        normal = -normal;
    }
    let extent = (normal.x.abs() * bounds.size().x + normal.y.abs() * bounds.size().y).max(1.0);
    (normal, travel.min(extent * max_fraction))
}

/// Shortest angular distance between two angles.
pub fn angle_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(TAU);
    d.min(TAU - d)
}

/// Bomb fall during the telegraph: from above the arena to the target, landing at
/// `LAND_AT` of the telegraph and sitting there until it explodes.
pub fn bomb_drop(target: V2, bounds: &Bounds, progress: f32) -> V2 {
    const LAND_AT: f32 = 0.65;
    let t = (progress / LAND_AT).clamp(0.0, 1.0);
    let start = V2::new(target.x, bounds.min.y - 60.0);
    let fall = t * t;
    start + (target - start) * fall
}

/// Regular polygon points (for drawing gems/bombs), first vertex at `rotation`.
pub fn polygon(center: V2, radius: f32, sides: u32, rotation: f32) -> Vec<V2> {
    let sides = sides.max(3);
    (0..sides)
        .map(|i| center + V2::from_angle(rotation + i as f32 * TAU / sides as f32) * radius)
        .collect()
}

/// Half-turn helper used for mirrored layouts.
pub fn opposite(angle: f32) -> f32 {
    angle + PI
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena() -> Bounds {
        Bounds::default()
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn line_crosses_the_whole_arena() {
        let (a, b) = line_through(V2::new(640.0, 360.0), 0.3, &arena());
        let bounds = arena();
        assert!(!bounds.contains(a, 0.0) && !bounds.contains(b, 0.0));
        let capsule = DangerShape::Capsule {
            a,
            b,
            radius: 5.0,
            activates_in: 0.0,
        };
        assert!(capsule.signed_distance(V2::new(640.0, 360.0)) < 0.0);
    }

    #[test]
    fn rings_spread_evenly_and_gaps_open_around_the_angle() {
        let full = ring_angles(12, 0.0, false);
        assert_eq!(full.len(), 12);
        assert!(close(full[1] - full[0], TAU / 12.0));
        let gapped = ring_angles(12, 1.0, true);
        assert_eq!(gapped.len(), 9);
        assert!(
            gapped
                .iter()
                .all(|a| angle_distance(*a, 1.0) > TAU / 12.0 * 1.5)
        );
        assert_eq!(ring_angles(0, 0.0, false).len(), 1);
    }

    #[test]
    fn bullets_move_linearly_from_the_start_radius() {
        let p = bullet_position(V2::new(100.0, 100.0), 0.0, 10.0, 50.0, 2.0);
        assert!(close(p.x, 210.0) && close(p.y, 100.0));
        let before = bullet_position(V2::ZERO, FRAC_PI_2, 10.0, 50.0, -1.0);
        assert!(close(before.y, 10.0));
    }

    #[test]
    fn spiral_arms_turn_over_time() {
        let shots = spiral_shots(3, 0.0, 1.0, 0.5, 1.0);
        assert_eq!(shots.len(), 9);
        assert_eq!(shots[0], (0.0, 0.0));
        assert!(close(shots[1].1, TAU / 3.0));
        assert!(close(shots[3].0, 0.5) && close(shots[3].1, 0.5));
        let counter = spiral_shots(1, 0.0, -2.0, 0.25, 0.5);
        assert!(close(counter[2].1, -1.0));
    }

    #[test]
    fn walls_cross_the_arena_and_keep_their_gap() {
        let bounds = arena();
        for dir in 0..4 {
            let start = wall_layout(&bounds, dir, 40.0, 0.5, 200.0, 0.0, 2.0);
            let end = wall_layout(&bounds, dir, 40.0, 0.5, 200.0, 1.0, 2.0);
            // The wall line starts and ends outside the arena.
            let line = |l: &WallLayout| {
                if dir % 2 == 0 {
                    l.gap_center.x
                } else {
                    l.gap_center.y
                }
            };
            let travel = if dir % 2 == 0 { 1280.0 } else { 720.0 };
            assert!(line(&start) < 0.0 || line(&start) > travel);
            assert!(line(&end) < 0.0 || line(&end) > travel);
            assert!(cardinal_vector(dir).dot(end.gap_center - start.gap_center) > 0.0);
            // The gap is free, the slabs are solid.
            let mid = wall_layout(&bounds, dir, 40.0, 0.3, 200.0, 0.5, 2.0);
            let slab = |p: V2| {
                mid.slabs.iter().any(|(c, h)| {
                    DangerShape::Rect {
                        center: *c,
                        half: *h,
                        angle: 0.0,
                        velocity: V2::ZERO,
                        activates_in: 0.0,
                    }
                    .overlaps_circle(p, 11.0)
                })
            };
            assert!(!slab(mid.gap_center), "dir {dir}");
            let across = if dir % 2 == 0 {
                V2::new(0.0, 1.0)
            } else {
                V2::new(1.0, 0.0)
            };
            assert!(slab(mid.gap_center + across * 140.0), "dir {dir}");
            assert!(slab(mid.gap_center - across * 140.0), "dir {dir}");
            // Velocity matches the travel.
            let speed = mid.velocity.length();
            assert!(close(speed, (travel + 40.0) / 2.0));
        }
        assert_eq!(cardinal(0.1), 0);
        assert_eq!(cardinal(PI), 2);
        assert_eq!(cardinal(-FRAC_PI_2), 3);
    }

    #[test]
    fn spikes_line_an_edge_and_point_inward() {
        let bounds = arena();
        let spikes = spike_layout(&bounds, 0, 6, 150.0, 1.0, false);
        assert_eq!(spikes.len(), 6);
        for spike in &spikes {
            assert!(close(spike.base.x, 0.0));
            assert!(close(spike.tip.x, 150.0));
            assert!(close(spike.half_width, 60.0));
        }
        let comb = spike_layout(&bounds, 3, 6, 100.0, 0.5, true);
        assert_eq!(comb.len(), 3);
        assert!(comb.iter().all(|s| close(s.tip.y, 670.0)));
        let shape = spikes[0].shape(0.0);
        assert!(shape.overlaps_circle(V2::new(100.0, spikes[0].base.y), 11.0));
        assert!(!shape.overlaps_circle(V2::new(200.0, spikes[0].base.y), 11.0));
    }

    #[test]
    fn spike_extension_thrusts_holds_and_retracts() {
        assert!(close(spike_extension(0.0), 0.0));
        let peak = (1..12)
            .map(|i| spike_extension(i as f32 / 100.0))
            .fold(0.0, f32::max);
        assert!(peak > 1.0, "overshoots");
        assert!(close(spike_extension(0.5), 1.0));
        assert!(close(spike_extension(1.0), 0.0));
    }

    #[test]
    fn barrage_aims_at_targets_in_turn() {
        let emitter = V2::new(0.0, 0.0);
        let targets = [V2::new(100.0, 0.0), V2::new(0.0, 100.0)];
        let shots = barrage_shots(emitter, &targets, 4, 0.1);
        assert_eq!(shots.len(), 4);
        assert!(close(shots[0].1, 0.0));
        assert!(close(shots[1].1, FRAC_PI_2));
        assert!(close(shots[2].1, 0.09) && close(shots[2].0, 0.2));
        assert_eq!(barrage_shots(emitter, &[], 2, 0.1).len(), 2);
    }

    #[test]
    fn sweeps_turn_a_quarter_and_push_toward_the_center() {
        assert!(close(sweep_angle(0.0, true, 1.0), FRAC_PI_2));
        assert!(close(sweep_angle(0.0, false, 1.0), -FRAC_PI_2));
        assert!(close(sweep_angle(0.3, true, 0.0), 0.3));
        let bounds = arena();
        let (dir, travel) = sweep_push(V2::new(20.0, 360.0), FRAC_PI_2, &bounds, 2000.0, 0.45);
        assert!(close(dir.x, 1.0));
        assert!(close(travel, 1280.0 * 0.45));
        let (dir, travel) = sweep_push(V2::new(640.0, 700.0), 0.0, &bounds, 100.0, 0.45);
        assert!(close(dir.y, -1.0));
        assert!(close(travel, 100.0));
    }

    #[test]
    fn bombs_fall_onto_their_target() {
        let target = V2::new(300.0, 400.0);
        let bounds = arena();
        assert!(bomb_drop(target, &bounds, 0.0).y < 0.0);
        assert_eq!(bomb_drop(target, &bounds, 0.65), target);
        assert_eq!(bomb_drop(target, &bounds, 1.0), target);
        assert_eq!(polygon(V2::ZERO, 1.0, 4, 0.0).len(), 4);
        assert!(close(opposite(0.0), PI));
    }
}
