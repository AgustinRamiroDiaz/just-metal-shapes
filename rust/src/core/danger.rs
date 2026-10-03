//! Danger shapes: what every hazard, projectile, mine and contact-damage enemy reports
//! so bots (and tests) can reason about the arena in pure Rust.
//!
//! Godot nodes return shapes from `danger_shapes() -> PackedFloat32Array` as flat
//! records of `RECORD_LEN` floats; `DangerField` decodes them into a `DangerSnapshot`.
//!
//! Record layout (`[tag, ...]`, unused slots are 0):
//! - Circle  `[0, cx, cy, radius, vx, vy, activates_in, 0, 0, 0]`
//! - Capsule `[1, ax, ay, bx, by, radius, activates_in, 0, 0, 0]`
//! - Rect    `[2, cx, cy, half_x, half_y, angle, vx, vy, activates_in, 0]`
//!
//! Positions are global pixels, velocities pixels/second, `activates_in` seconds until
//! the shape can hurt (0 = already active).

use std::ops::{Add, Mul, Neg, Sub};

pub const RECORD_LEN: usize = 10;
const TAG_CIRCLE: f32 = 0.0;
const TAG_CAPSULE: f32 = 1.0;
const TAG_RECT: f32 = 2.0;

/// Distance (px) over which danger fades from 1 (touching) to 0.
pub const DANGER_MARGIN: f32 = 48.0;
/// Seconds of warning over which a not-yet-active shape ramps from 0 to full danger.
pub const ACTIVATION_RAMP: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const ZERO: V2 = V2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dot(self, other: V2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn distance_to(self, other: V2) -> f32 {
        (self - other).length()
    }

    pub fn normalized_or_zero(self) -> V2 {
        let length = self.length();
        if length > 1e-6 {
            self * (1.0 / length)
        } else {
            V2::ZERO
        }
    }

    /// Rotates by `angle` radians.
    pub fn rotated(self, angle: f32) -> V2 {
        let (sin, cos) = angle.sin_cos();
        V2::new(self.x * cos - self.y * sin, self.x * sin + self.y * cos)
    }

    pub fn from_angle(angle: f32) -> V2 {
        V2::new(angle.cos(), angle.sin())
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, rhs: V2) -> V2 {
        V2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for V2 {
    type Output = V2;
    fn sub(self, rhs: V2) -> V2 {
        V2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, rhs: f32) -> V2 {
        V2::new(self.x * rhs, self.y * rhs)
    }
}

impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        V2::new(-self.x, -self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DangerShape {
    Circle {
        center: V2,
        radius: f32,
        velocity: V2,
        activates_in: f32,
    },
    /// A segment with thickness: lasers and beams.
    Capsule {
        a: V2,
        b: V2,
        radius: f32,
        activates_in: f32,
    },
    /// Oriented box (`angle` radians): walls.
    Rect {
        center: V2,
        half: V2,
        angle: f32,
        velocity: V2,
        activates_in: f32,
    },
}

impl DangerShape {
    pub fn activates_in(&self) -> f32 {
        match *self {
            DangerShape::Circle { activates_in, .. }
            | DangerShape::Capsule { activates_in, .. }
            | DangerShape::Rect { activates_in, .. } => activates_in,
        }
    }

    pub fn is_active(&self) -> bool {
        self.activates_in() <= 0.0
    }

    /// The shape `t` seconds from now (moved by its velocity).
    pub fn advanced(&self, t: f32) -> DangerShape {
        match *self {
            DangerShape::Circle {
                center,
                radius,
                velocity,
                activates_in,
            } => DangerShape::Circle {
                center: center + velocity * t,
                radius,
                velocity,
                activates_in: activates_in - t,
            },
            DangerShape::Capsule {
                a,
                b,
                radius,
                activates_in,
            } => DangerShape::Capsule {
                a,
                b,
                radius,
                activates_in: activates_in - t,
            },
            DangerShape::Rect {
                center,
                half,
                angle,
                velocity,
                activates_in,
            } => DangerShape::Rect {
                center: center + velocity * t,
                half,
                angle,
                velocity,
                activates_in: activates_in - t,
            },
        }
    }

    /// Distance from `p` to the shape's surface; negative inside.
    pub fn signed_distance(&self, p: V2) -> f32 {
        match *self {
            DangerShape::Circle { center, radius, .. } => p.distance_to(center) - radius,
            DangerShape::Capsule { a, b, radius, .. } => {
                let ab = b - a;
                let len_sq = ab.dot(ab);
                let t = if len_sq > 1e-9 {
                    ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                p.distance_to(a + ab * t) - radius
            }
            DangerShape::Rect {
                center,
                half,
                angle,
                ..
            } => {
                let local = (p - center).rotated(-angle);
                let qx = local.x.abs() - half.x;
                let qy = local.y.abs() - half.y;
                let outside = V2::new(qx.max(0.0), qy.max(0.0)).length();
                outside + qx.max(qy).min(0.0)
            }
        }
    }

    /// Whether a circle of `radius` at `p` touches the shape (ignores activation).
    pub fn overlaps_circle(&self, p: V2, radius: f32) -> bool {
        self.signed_distance(p) <= radius
    }

    pub fn encode_into(&self, out: &mut Vec<f32>) {
        let record: [f32; RECORD_LEN] = match *self {
            DangerShape::Circle {
                center,
                radius,
                velocity,
                activates_in,
            } => [
                TAG_CIRCLE,
                center.x,
                center.y,
                radius,
                velocity.x,
                velocity.y,
                activates_in,
                0.0,
                0.0,
                0.0,
            ],
            DangerShape::Capsule {
                a,
                b,
                radius,
                activates_in,
            } => [
                TAG_CAPSULE,
                a.x,
                a.y,
                b.x,
                b.y,
                radius,
                activates_in,
                0.0,
                0.0,
                0.0,
            ],
            DangerShape::Rect {
                center,
                half,
                angle,
                velocity,
                activates_in,
            } => [
                TAG_RECT,
                center.x,
                center.y,
                half.x,
                half.y,
                angle,
                velocity.x,
                velocity.y,
                activates_in,
                0.0,
            ],
        };
        out.extend_from_slice(&record);
    }

    pub fn decode_record(record: &[f32]) -> Option<DangerShape> {
        if record.len() < RECORD_LEN {
            return None;
        }
        let r = record;
        if r[0] == TAG_CIRCLE {
            Some(DangerShape::Circle {
                center: V2::new(r[1], r[2]),
                radius: r[3],
                velocity: V2::new(r[4], r[5]),
                activates_in: r[6],
            })
        } else if r[0] == TAG_CAPSULE {
            Some(DangerShape::Capsule {
                a: V2::new(r[1], r[2]),
                b: V2::new(r[3], r[4]),
                radius: r[5],
                activates_in: r[6],
            })
        } else if r[0] == TAG_RECT {
            Some(DangerShape::Rect {
                center: V2::new(r[1], r[2]),
                half: V2::new(r[3], r[4]),
                angle: r[5],
                velocity: V2::new(r[6], r[7]),
                activates_in: r[8],
            })
        } else {
            None
        }
    }
}

pub fn encode(shapes: &[DangerShape]) -> Vec<f32> {
    let mut out = Vec::with_capacity(shapes.len() * RECORD_LEN);
    for shape in shapes {
        shape.encode_into(&mut out);
    }
    out
}

/// Decodes whole records; unknown tags and a trailing partial record are skipped.
pub fn decode(data: &[f32]) -> Vec<DangerShape> {
    data.chunks_exact(RECORD_LEN)
        .filter_map(DangerShape::decode_record)
        .collect()
}

/// All danger in the arena at one physics frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DangerSnapshot {
    pub shapes: Vec<DangerShape>,
}

impl DangerSnapshot {
    pub fn new(shapes: Vec<DangerShape>) -> Self {
        Self { shapes }
    }

    pub fn from_records(data: &[f32]) -> Self {
        Self::new(decode(data))
    }

    pub fn clear(&mut self) {
        self.shapes.clear();
    }

    pub fn extend_from_records(&mut self, data: &[f32]) {
        self.shapes.extend(decode(data));
    }

    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// Danger in `0..=1` at `pos`, `t_ahead` seconds from now. 1 means inside an active
    /// shape; it fades to 0 at `DANGER_MARGIN` px from the surface, and shapes that
    /// are still telegraphing at `t_ahead` contribute less the longer they have left
    /// (`ACTIVATION_RAMP`).
    pub fn danger_at(&self, pos: V2, t_ahead: f32) -> f32 {
        self.shapes
            .iter()
            .map(|shape| {
                let future = shape.advanced(t_ahead);
                let pending = future.activates_in().max(0.0);
                let readiness = (1.0 - pending / ACTIVATION_RAMP).clamp(0.0, 1.0);
                let distance = future.signed_distance(pos);
                let proximity = (1.0 - distance / DANGER_MARGIN).clamp(0.0, 1.0);
                readiness * proximity
            })
            .fold(0.0, f32::max)
    }

    /// Smallest signed distance from `pos` to any shape that is active `t_ahead`
    /// seconds from now. `f32::INFINITY` if there is none.
    pub fn min_distance(&self, pos: V2, t_ahead: f32) -> f32 {
        self.shapes
            .iter()
            .map(|shape| shape.advanced(t_ahead))
            .filter(DangerShape::is_active)
            .map(|shape| shape.signed_distance(pos))
            .fold(f32::INFINITY, f32::min)
    }

    /// Smallest signed distance to any shape, active or not.
    pub fn min_distance_any(&self, pos: V2, t_ahead: f32) -> f32 {
        self.shapes
            .iter()
            .map(|shape| shape.advanced(t_ahead).signed_distance(pos))
            .fold(f32::INFINITY, f32::min)
    }

    /// Whether a circle of `radius` at `pos` would be hit `t_ahead` seconds from now.
    pub fn is_hit(&self, pos: V2, radius: f32, t_ahead: f32) -> bool {
        self.min_distance(pos, t_ahead) <= radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(x: f32, y: f32, r: f32) -> DangerShape {
        DangerShape::Circle {
            center: V2::new(x, y),
            radius: r,
            velocity: V2::ZERO,
            activates_in: 0.0,
        }
    }

    fn samples() -> Vec<DangerShape> {
        vec![
            DangerShape::Circle {
                center: V2::new(1.0, 2.0),
                radius: 3.0,
                velocity: V2::new(4.0, 5.0),
                activates_in: 0.5,
            },
            DangerShape::Capsule {
                a: V2::new(-1.0, 0.0),
                b: V2::new(100.0, 50.0),
                radius: 8.0,
                activates_in: 0.0,
            },
            DangerShape::Rect {
                center: V2::new(640.0, 360.0),
                half: V2::new(20.0, 300.0),
                angle: 0.3,
                velocity: V2::new(-60.0, 0.0),
                activates_in: 1.25,
            },
        ]
    }

    #[test]
    fn encode_decode_round_trip() {
        let shapes = samples();
        let data = encode(&shapes);
        assert_eq!(data.len(), shapes.len() * RECORD_LEN);
        assert_eq!(decode(&data), shapes);
    }

    #[test]
    fn decode_skips_garbage() {
        let mut data = encode(&samples());
        let mut bogus = vec![9.0; RECORD_LEN];
        bogus.extend_from_slice(&data);
        data.extend_from_slice(&[0.0, 1.0, 2.0]);
        assert_eq!(decode(&bogus).len(), 3);
        assert_eq!(decode(&data).len(), 3);
        assert!(decode(&[]).is_empty());
    }

    #[test]
    fn signed_distances() {
        assert!((circle(0.0, 0.0, 10.0).signed_distance(V2::new(15.0, 0.0)) - 5.0).abs() < 1e-5);
        assert!((circle(0.0, 0.0, 10.0).signed_distance(V2::ZERO) + 10.0).abs() < 1e-5);

        let capsule = DangerShape::Capsule {
            a: V2::new(0.0, 0.0),
            b: V2::new(100.0, 0.0),
            radius: 5.0,
            activates_in: 0.0,
        };
        assert!((capsule.signed_distance(V2::new(50.0, 20.0)) - 15.0).abs() < 1e-4);
        assert!((capsule.signed_distance(V2::new(110.0, 0.0)) - 5.0).abs() < 1e-4);
        assert!(capsule.signed_distance(V2::new(50.0, 0.0)) < 0.0);

        let rect = DangerShape::Rect {
            center: V2::new(0.0, 0.0),
            half: V2::new(10.0, 50.0),
            angle: std::f32::consts::FRAC_PI_2,
            velocity: V2::ZERO,
            activates_in: 0.0,
        };
        // Rotated 90 degrees: long axis is now horizontal.
        assert!(rect.signed_distance(V2::new(40.0, 0.0)) < 0.0);
        assert!((rect.signed_distance(V2::new(0.0, 30.0)) - 20.0).abs() < 1e-3);
        assert!(rect.overlaps_circle(V2::new(0.0, 15.0), 6.0));
    }

    #[test]
    fn advanced_moves_and_counts_down() {
        let shape = samples()[0].advanced(0.5);
        match shape {
            DangerShape::Circle {
                center,
                activates_in,
                ..
            } => {
                assert_eq!(center, V2::new(3.0, 4.5));
                assert_eq!(activates_in, 0.0);
            }
            _ => unreachable!(),
        }
        assert!(shape.is_active());
    }

    #[test]
    fn snapshot_queries() {
        let snapshot = DangerSnapshot::new(vec![
            circle(100.0, 100.0, 20.0),
            DangerShape::Circle {
                center: V2::new(300.0, 100.0),
                radius: 20.0,
                velocity: V2::ZERO,
                activates_in: 2.0,
            },
            DangerShape::Circle {
                center: V2::new(500.0, 100.0),
                radius: 10.0,
                velocity: V2::new(-100.0, 0.0),
                activates_in: 0.0,
            },
        ]);
        assert_eq!(snapshot.danger_at(V2::new(100.0, 100.0), 0.0), 1.0);
        assert_eq!(snapshot.danger_at(V2::new(100.0, 400.0), 0.0), 0.0);
        let edge = snapshot.danger_at(V2::new(100.0, 144.0), 0.0);
        assert!(edge > 0.0 && edge < 1.0);
        // Still telegraphing: harmless now, full danger once active.
        assert_eq!(snapshot.danger_at(V2::new(300.0, 100.0), 0.0), 0.0);
        assert_eq!(snapshot.danger_at(V2::new(300.0, 100.0), 2.0), 1.0);
        assert!(!snapshot.is_hit(V2::new(300.0, 100.0), 5.0, 0.0));
        assert!(snapshot.is_hit(V2::new(300.0, 100.0), 5.0, 2.5));
        // The moving circle reaches x=400 after one second.
        assert!(!snapshot.is_hit(V2::new(400.0, 100.0), 1.0, 0.0));
        assert!(snapshot.is_hit(V2::new(400.0, 100.0), 1.0, 1.0));
        assert!((snapshot.min_distance(V2::new(100.0, 150.0), 0.0) - 30.0).abs() < 1e-4);
        assert!(
            snapshot.min_distance_any(V2::new(300.0, 150.0), 0.0)
                < snapshot.min_distance(V2::new(300.0, 150.0), 0.0)
        );
        assert_eq!(
            DangerSnapshot::default().min_distance(V2::ZERO, 0.0),
            f32::INFINITY
        );
        assert_eq!(
            DangerSnapshot::from_records(&encode(&snapshot.shapes)),
            snapshot
        );
    }
}
