//! Bot decision logic over a `DangerSnapshot`: utility-scored steering.
//!
//! Every decision evaluates *plans*: a heading (sampled `directions` ways at full and
//! reduced speed) followed for a stop time, then holding still until the end of the
//! `horizon` and for a further `hold`; plus standing still. Each plan is simulated against the snapshot (shapes
//! move by their velocity and count down `activates_in`) and scored:
//!
//! - danger (dominant): the worst and mean proximity to shapes that are active, or
//!   about to be, along the plan, plus a large penalty per predicted hit. Far-future
//!   samples weigh less because the bot re-plans long before reaching them;
//! - revive: reach a downed teammate and hold inside revive distance;
//! - attack: get the nearest damageable enemy (matching shield color, or no shield)
//!   inside the range ring, close enough for extra rays but clear of its contact radius;
//! - cohesion: stay loosely near living teammates without stacking on them, and drift
//!   back toward the arena center when idle;
//! - arena edges: corners leave no escape routes;
//! - hysteresis toward the previous heading, plus seeded per-skill noise.
//!
//! The heading of the best plan is the decision. Stop times let the bot plan to reach a
//! safe pocket (a wall's gap) and wait there. Goal terms score *progress* toward a
//! target band, so their pull is as strong across the arena as next to the target.
//! Everything is deterministic for a given input.

use super::danger::{ACTIVATION_RAMP, DangerShape, DangerSnapshot, V2};
use super::rng::Rng;
use std::collections::VecDeque;

pub const SKILL_EASY: i32 = 0;
pub const SKILL_NORMAL: i32 = 1;
pub const SKILL_HARD: i32 = 2;

/// Lightning range tiers (see `Player`): the inner third of the ring fires 3 rays.
const RANGE_TIERS: f32 = 3.0;
/// A downed teammate is revived by a living player within this distance (px).
pub const REVIVE_DISTANCE: f32 = 60.0;
/// Danger samples over a plan's `hold` window.
const HOLD_STEPS: usize = 3;
/// Relative travel (px) between a shape and the bot per danger sample, as a fraction
/// of the combined thickness of both, so neither can pass through the other unseen.
const SWEEP_FRACTION: f32 = 0.5;
const MIN_SWEEP_STEP: f32 = 8.0;
const MAX_SWEEP_STEP: f32 = 48.0;
const MAX_SUBSTEPS: f32 = 12.0;
/// Seconds ahead at which goal progress is measured.
const GOAL_LOOKAHEAD: f32 = 0.3;

/// Tunables for one bot. `for_skill` gives the easy/normal/hard presets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotTuning {
    /// Seconds between perceiving the world and acting on it.
    pub reaction_delay: f32,
    /// Seconds between decisions; the heading is smoothed in between.
    pub decision_interval: f32,
    /// Seconds of prediction per plan.
    pub horizon: f32,
    /// Danger samples along the horizon (moving shapes get extra swept samples).
    pub horizon_steps: usize,
    /// Seconds the end position is held and checked after the horizon, so plans that
    /// end in a dead end (a full-width wall still coming) lose to ones that end safe.
    pub hold: f32,
    /// Stop times tried per heading: `horizon * i / plans` for `i` in `1..=plans`.
    pub plans: usize,
    /// Headings sampled per speed.
    pub directions: usize,
    /// Reduced speed as a fraction of full speed.
    pub slow_speed: f32,
    /// Max random score added per plan (score units).
    pub noise: f32,
    /// How fast the applied heading follows the decided one (1/s).
    pub turn_rate: f32,
    /// Distance (px) beyond the body radius at which danger fades to zero.
    pub danger_margin: f32,
    pub danger_weight: f32,
    pub hit_weight: f32,
    pub attack_weight: f32,
    pub revive_weight: f32,
    pub cohesion_weight: f32,
    pub edge_weight: f32,
    /// Distance (px) from the arena border where the edge penalty starts.
    pub edge_margin: f32,
    pub hysteresis: f32,
}

impl BotTuning {
    pub fn for_skill(skill: i32) -> Self {
        let normal = BotTuning {
            reaction_delay: 0.12,
            decision_interval: 1.0 / 20.0,
            horizon: 1.2,
            horizon_steps: 8,
            hold: 1.0,
            plans: 2,
            directions: 16,
            slow_speed: 0.4,
            noise: 0.15,
            turn_rate: 15.0,
            danger_margin: 36.0,
            danger_weight: 8.0,
            hit_weight: 20.0,
            attack_weight: 1.0,
            revive_weight: 2.0,
            cohesion_weight: 0.3,
            edge_weight: 1.5,
            edge_margin: 80.0,
            hysteresis: 0.25,
        };
        match skill {
            SKILL_EASY => BotTuning {
                reaction_delay: 0.25,
                decision_interval: 1.0 / 12.0,
                horizon: 0.6,
                horizon_steps: 5,
                hold: 0.3,
                plans: 1,
                directions: 12,
                noise: 0.6,
                turn_rate: 8.0,
                danger_weight: 6.0,
                ..normal
            },
            SKILL_HARD => BotTuning {
                reaction_delay: 0.04,
                decision_interval: 1.0 / 30.0,
                horizon: 1.6,
                horizon_steps: 10,
                hold: 1.5,
                plans: 3,
                directions: 20,
                noise: 0.0,
                turn_rate: 30.0,
                ..normal
            },
            _ => normal,
        }
    }

    /// `easy`, `normal` or `hard`.
    pub fn skill_name(skill: i32) -> &'static str {
        match skill {
            SKILL_EASY => "easy",
            SKILL_HARD => "hard",
            _ => "normal",
        }
    }
}

impl Default for BotTuning {
    fn default() -> Self {
        Self::for_skill(SKILL_NORMAL)
    }
}

/// An enemy as a bot sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotEnemy {
    pub position: V2,
    /// Contact-damage radius (stay outside it).
    pub contact_radius: f32,
    /// Whether this bot's lightning damages it: its active shield matches the bot's
    /// color, or it has no shield left.
    pub damageable: bool,
}

/// A teammate as a bot sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotTeammate {
    pub position: V2,
    pub is_dead: bool,
}

/// Everything `decide` needs for one decision.
#[derive(Clone, Copy, Debug)]
pub struct BotInput<'a> {
    pub position: V2,
    /// Player body radius (px).
    pub radius: f32,
    /// Max speed (px/s).
    pub speed: f32,
    /// Lightning range ring radius (px).
    pub range_radius: f32,
    /// Arena bounds in global pixels.
    pub arena_min: V2,
    pub arena_max: V2,
    pub danger: &'a DangerSnapshot,
    pub enemies: &'a [BotEnemy],
    /// Other players (not this bot).
    pub teammates: &'a [BotTeammate],
    /// Heading chosen by the previous decision (hysteresis). Pass the decided heading,
    /// not a delayed or smoothed one, or the bot dithers between near-equal options.
    pub previous: V2,
    pub tuning: &'a BotTuning,
    /// Per-bot deterministic seed (e.g. player index).
    pub seed: u64,
    /// Decision counter, mixed into the noise seed.
    pub tick: u64,
}

/// The chosen heading and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotDecision {
    /// Move direction, length `0..=1`.
    pub direction: V2,
    pub score: f32,
    /// Danger cost of the chosen plan (0 = clear).
    pub danger: f32,
    /// Whether the chosen plan still predicts a hit.
    pub predicts_hit: bool,
}

/// Move direction for this decision, length `0..=1`.
pub fn decide(input: &BotInput) -> V2 {
    decide_scored(input).direction
}

pub fn decide_scored(input: &BotInput) -> BotDecision {
    let idle = BotDecision {
        direction: V2::ZERO,
        score: 0.0,
        danger: 0.0,
        predicts_hit: false,
    };
    if !finite(input.position) || !input.speed.is_finite() || input.speed <= 0.0 {
        return idle;
    }
    let tuning = sanitize(input.tuning);
    let ctx = Context::new(input, &tuning);

    let mut rng = Rng::new(
        input
            .seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(input.tick),
    );
    let previous = if finite(input.previous) {
        input.previous
    } else {
        V2::ZERO
    };
    let previous_dir = previous.normalized_or_zero();

    let mut best = idle;
    let mut best_score = f32::NEG_INFINITY;
    let mut evaluate = |direction: V2, stop: f32| {
        let keep = if direction.length() > 0.0 {
            direction.normalized_or_zero().dot(previous_dir).max(0.0)
        } else {
            1.0 - previous.length().min(1.0)
        };
        let bonus = tuning.hysteresis * keep + rng.next_f32() * tuning.noise;
        let plan = Plan { direction, stop };
        if let Some((score, danger, hit)) = ctx.score(&plan, bonus, best_score)
            && score > best_score
        {
            best_score = score;
            best = BotDecision {
                direction,
                score,
                danger,
                predicts_hit: hit,
            };
        }
    };

    // The previous heading usually stays best: trying it first lets pruning cut the
    // rest early.
    let dirs = tuning.directions;
    let first = if previous_dir == V2::ZERO {
        0
    } else {
        let turns = previous_dir.y.atan2(previous_dir.x) / std::f32::consts::TAU;
        ((turns * dirs as f32).round() as i64).rem_euclid(dirs as i64) as usize
    };
    for speed in [1.0, tuning.slow_speed] {
        for offset in 0..dirs {
            let i = (first + offset) % dirs;
            let angle = i as f32 / dirs as f32 * std::f32::consts::TAU;
            let direction = V2::from_angle(angle) * speed;
            for plan in 1..=tuning.plans {
                evaluate(
                    direction,
                    tuning.horizon * plan as f32 / tuning.plans as f32,
                );
            }
        }
    }
    evaluate(V2::ZERO, 0.0);
    if !finite(best.direction) || !best.score.is_finite() {
        return idle;
    }
    best
}

fn finite(v: V2) -> bool {
    v.x.is_finite() && v.y.is_finite()
}

fn sanitize(tuning: &BotTuning) -> BotTuning {
    BotTuning {
        horizon: if tuning.horizon.is_finite() {
            tuning.horizon.clamp(0.05, 3.0)
        } else {
            1.0
        },
        horizon_steps: tuning.horizon_steps.clamp(1, 32 - HOLD_STEPS),
        hold: if tuning.hold.is_finite() {
            tuning.hold.clamp(0.0, 3.0)
        } else {
            0.0
        },
        plans: tuning.plans.clamp(1, 6),
        directions: tuning.directions.clamp(4, 64),
        slow_speed: tuning.slow_speed.clamp(0.05, 1.0),
        noise: tuning.noise.max(0.0),
        danger_margin: tuning.danger_margin.max(1.0),
        edge_margin: tuning.edge_margin.max(1.0),
        ..*tuning
    }
}

/// Move along `direction` (length = speed fraction) for `stop` seconds, then hold.
struct Plan {
    direction: V2,
    stop: f32,
}

/// Per-decision precomputed state shared by every plan.
struct Context<'a> {
    input: &'a BotInput<'a>,
    tuning: &'a BotTuning,
    /// Shapes that can reach the bot within the horizon.
    shapes: Vec<Threat>,
    arena_min: V2,
    arena_max: V2,
    revive_target: Option<V2>,
    /// `(position, inner, outer)`: the distance band to hold around the target.
    attack_target: Option<(V2, f32, f32)>,
    team_center: Option<V2>,
    living_mates: Vec<V2>,
    arena_center: V2,
}

impl<'a> Context<'a> {
    fn new(input: &'a BotInput<'a>, tuning: &'a BotTuning) -> Self {
        let mut arena_min = input.arena_min;
        let mut arena_max = input.arena_max;
        if !finite(arena_min) || !finite(arena_max) {
            arena_min = input.position;
            arena_max = input.position;
        }
        arena_max = V2::new(arena_max.x.max(arena_min.x), arena_max.y.max(arena_min.y));

        let samples = sample_count(tuning);
        let shapes = input
            .danger
            .shapes
            .iter()
            .filter_map(|shape| Threat::new(shape, input, tuning, samples))
            .collect();

        let clamp = |p: V2| clamp_to(p, arena_min, arena_max);
        let revive_target = input
            .teammates
            .iter()
            .filter(|mate| mate.is_dead && finite(mate.position))
            .map(|mate| clamp(mate.position))
            .min_by(|a, b| {
                a.distance_to(input.position)
                    .total_cmp(&b.distance_to(input.position))
            });

        let range = input.range_radius.max(input.radius * 2.0);
        let attack_target = input
            .enemies
            .iter()
            .filter(|enemy| enemy.damageable && finite(enemy.position))
            .min_by(|a, b| {
                a.position
                    .distance_to(input.position)
                    .total_cmp(&b.position.distance_to(input.position))
            })
            .map(|enemy| {
                let contact = if enemy.contact_radius.is_finite() {
                    enemy.contact_radius.max(0.0)
                } else {
                    0.0
                };
                let inner = contact + input.radius + tuning.danger_margin * 0.75;
                let outer = (range * 2.0 / RANGE_TIERS).max(inner + 20.0);
                (enemy.position, inner, outer)
            });

        let living_mates: Vec<V2> = input
            .teammates
            .iter()
            .filter(|mate| !mate.is_dead && finite(mate.position))
            .map(|mate| mate.position)
            .collect();
        let team_center = if living_mates.is_empty() {
            None
        } else {
            let sum = living_mates.iter().fold(V2::ZERO, |acc, p| acc + *p);
            Some(sum * (1.0 / living_mates.len() as f32))
        };

        Self {
            input,
            tuning,
            shapes,
            arena_min,
            arena_max,
            revive_target,
            attack_target,
            team_center,
            living_mates,
            arena_center: (arena_min + arena_max) * 0.5,
        }
    }

    fn position_at(&self, plan: &Plan, t: f32) -> V2 {
        let travelled = self.input.speed * t.min(plan.stop);
        clamp_to(
            self.input.position + plan.direction * travelled,
            self.arena_min,
            self.arena_max,
        )
    }

    /// `(score, danger cost, predicts hit)` for `plan` with `bonus` added, or `None`
    /// once the plan provably cannot beat `to_beat` (costs only grow along the plan).
    fn score(&self, plan: &Plan, bonus: f32, to_beat: f32) -> Option<(f32, f32, bool)> {
        let tuning = self.tuning;
        let utility = self.utility(plan) + bonus;
        if utility < to_beat {
            return None;
        }
        let count = sample_count(tuning);
        let samples = count as f32;
        let mut worst: f32 = 0.0;
        let mut total = 0.0;
        let mut hits = 0.0;
        let mut edge: f32 = 0.0;
        let mut cost = 0.0;
        let mut from = (self.input.position, 0.0);
        for k in 1..=count {
            let t = sample_time(tuning, k);
            let to = (self.position_at(plan, t), t);
            // The bot re-plans long before far samples: they weigh less.
            let certainty = 1.0 / (1.0 + t / tuning.horizon);
            let (danger, hit) = self.segment_danger(k, from, to);
            from = to;
            worst = worst.max(danger * certainty);
            total += danger * certainty;
            if hit {
                hits += certainty;
            }
            edge = edge.max(self.edge_penalty(to.0));
            cost = tuning.danger_weight * (0.7 * worst + 0.3 * total / samples)
                + tuning.hit_weight * hits
                + tuning.edge_weight * edge;
            if utility - cost < to_beat {
                return None;
            }
        }
        let danger_cost = 0.7 * worst + 0.3 * total / samples;
        Some((utility - cost, danger_cost, hits > 0.0))
    }

    /// Goal terms (revive, attack, cohesion) at the plan's position shortly ahead.
    fn utility(&self, plan: &Plan) -> f32 {
        let tuning = self.tuning;
        let goal_t = GOAL_LOOKAHEAD.min(tuning.horizon);
        let goal = self.position_at(plan, goal_t);
        let step = (self.input.speed * goal_t).max(1.0);
        let mut utility = 0.0;
        let reviving = self.revive_target.is_some();
        if let Some(target) = self.revive_target {
            let hold = REVIVE_DISTANCE * 0.6;
            utility += tuning.revive_weight * self.band_progress(target, goal, 0.0, hold, step);
        }
        if let Some((target, inner, outer)) = self.attack_target {
            let weight = if reviving { 0.3 } else { 1.0 };
            utility += tuning.attack_weight
                * weight
                * self.band_progress(target, goal, inner, outer, step);
        }
        if let Some(center) = self.team_center {
            utility += tuning.cohesion_weight * self.band_progress(center, goal, 0.0, 220.0, step);
        }
        utility += tuning.cohesion_weight
            * 0.5
            * self.band_progress(self.arena_center, goal, 0.0, 200.0, step);
        let crowding = self
            .living_mates
            .iter()
            .map(|mate| (1.0 - goal.distance_to(*mate) / 45.0).clamp(0.0, 1.0))
            .fold(0.0, f32::max);
        utility - tuning.cohesion_weight * crowding
    }

    /// 1 when `goal` lies in the distance band `[inner, outer]` around `target`,
    /// otherwise the progress toward the band since now, in `-0.8..=0.8`.
    fn band_progress(&self, target: V2, goal: V2, inner: f32, outer: f32, step: f32) -> f32 {
        let error = |p: V2| {
            let d = p.distance_to(target);
            if d < inner {
                inner - d
            } else if d > outer {
                d - outer
            } else {
                0.0
            }
        };
        let after = error(goal);
        if after <= 0.0 {
            return 1.0;
        }
        let before = error(self.input.position);
        ((before - after) / step).clamp(-1.0, 1.0) * 0.8
    }

    /// Worst `(danger 0..=1, hit)` while moving in a straight line between two
    /// `(position, seconds ahead)` samples ending at sample `k`. Each shape is sampled
    /// often enough that the bot and the shape cannot pass through each other.
    fn segment_danger(&self, k: usize, from: (V2, f32), to: (V2, f32)) -> (f32, bool) {
        let (p0, t0) = from;
        let (p1, t1) = to;
        let bit = 1u32 << (k - 1);
        let mut danger: f32 = 0.0;
        let mut hit = false;
        for threat in &self.shapes {
            if threat.samples & bit == 0 {
                continue;
            }
            let shape = &threat.shape;
            let relative = (p1 - p0) - velocity(shape) * (t1 - t0);
            let substeps = (relative.length() / threat.sweep_step)
                .ceil()
                .clamp(1.0, MAX_SUBSTEPS);
            for j in 1..=substeps as usize {
                let f = j as f32 / substeps;
                let (d, h) = self.shape_danger(shape, p0 + (p1 - p0) * f, t0 + (t1 - t0) * f);
                danger = danger.max(d);
                hit |= h;
            }
        }
        (danger, hit)
    }

    /// `(danger 0..=1, hit)` from one shape at `p`, `t` seconds ahead.
    fn shape_danger(&self, shape: &DangerShape, p: V2, t: f32) -> (f32, bool) {
        let future = shape.advanced(t);
        let pending = future.activates_in().max(0.0);
        let readiness = (1.0 - pending / ACTIVATION_RAMP).clamp(0.0, 1.0);
        if readiness <= 0.0 {
            return (0.0, false);
        }
        let distance = future.signed_distance(p) - self.input.radius;
        if !distance.is_finite() {
            return (0.0, false);
        }
        let hit = distance <= 0.0 && pending <= 0.0;
        let proximity = (1.0 - distance / self.tuning.danger_margin).clamp(0.0, 1.0);
        (readiness * proximity, hit)
    }

    fn edge_penalty(&self, p: V2) -> f32 {
        let margin = self.tuning.edge_margin;
        let near = |d: f32| ((margin - d) / margin).clamp(0.0, 1.0).powi(2);
        near(p.x - self.arena_min.x)
            + near(self.arena_max.x - p.x)
            + near(p.y - self.arena_min.y)
            + near(self.arena_max.y - p.y)
    }
}

/// Danger samples per plan: `horizon_steps` over the horizon, then the hold window.
fn sample_count(tuning: &BotTuning) -> usize {
    tuning.horizon_steps + if tuning.hold > 0.0 { HOLD_STEPS } else { 0 }
}

/// Seconds ahead of sample `k` (`1..=sample_count`).
fn sample_time(tuning: &BotTuning, k: usize) -> f32 {
    let steps = tuning.horizon_steps;
    if k <= steps {
        tuning.horizon * k as f32 / steps as f32
    } else {
        tuning.horizon + tuning.hold * (k - steps) as f32 / HOLD_STEPS as f32
    }
}

/// A shape that matters for this decision.
struct Threat {
    shape: DangerShape,
    /// Max relative travel (px) between the shape and the bot per danger sample.
    sweep_step: f32,
    /// Bit `k - 1` set when the shape can touch anywhere the bot could be during the
    /// window ending at sample `k` (and is not still far from activating).
    samples: u32,
}

impl Threat {
    fn new(
        shape: &DangerShape,
        input: &BotInput,
        tuning: &BotTuning,
        sample_count: usize,
    ) -> Option<Threat> {
        let mut samples = 0u32;
        for k in 1..=sample_count {
            let start = if k == 1 {
                0.0
            } else {
                sample_time(tuning, k - 1)
            };
            let t = sample_time(tuning, k);
            let window = shape.advanced(start);
            if window.activates_in() - (t - start) >= ACTIVATION_RAMP {
                continue;
            }
            let (a, b, radius) = swept_bounds(&window, t - start);
            let path = DangerShape::Capsule {
                a,
                b,
                radius,
                activates_in: 0.0,
            };
            let reach = input.speed * t.min(tuning.horizon) + input.radius + tuning.danger_margin;
            let distance = path.signed_distance(input.position);
            if distance.is_finite() && distance <= reach {
                samples |= 1 << (k - 1);
            }
        }
        if samples == 0 {
            return None;
        }
        let step = (thickness(shape) + input.radius * 2.0) * SWEEP_FRACTION;
        let sweep_step = if step.is_finite() {
            step.clamp(MIN_SWEEP_STEP, MAX_SWEEP_STEP)
        } else {
            MIN_SWEEP_STEP
        };
        Some(Threat {
            shape: *shape,
            sweep_step,
            samples,
        })
    }
}

fn velocity(shape: &DangerShape) -> V2 {
    match *shape {
        DangerShape::Circle { velocity, .. } | DangerShape::Rect { velocity, .. } => velocity,
        DangerShape::Capsule { .. } => V2::ZERO,
    }
}

fn clamp_to(p: V2, min: V2, max: V2) -> V2 {
    V2::new(p.x.clamp(min.x, max.x), p.y.clamp(min.y, max.y))
}

/// A capsule `(a, b, radius)` covering everywhere `shape` can be during the next
/// `lookahead` seconds.
fn swept_bounds(shape: &DangerShape, lookahead: f32) -> (V2, V2, f32) {
    match *shape {
        DangerShape::Circle {
            center,
            radius,
            velocity,
            ..
        } => (center, center + velocity * lookahead, radius),
        DangerShape::Capsule { a, b, radius, .. } => (a, b, radius),
        DangerShape::Rect {
            center,
            half,
            velocity,
            ..
        } => (center, center + velocity * lookahead, half.length()),
    }
}

/// The shape's narrowest cross-section (px).
fn thickness(shape: &DangerShape) -> f32 {
    match *shape {
        DangerShape::Circle { radius, .. } | DangerShape::Capsule { radius, .. } => radius * 2.0,
        DangerShape::Rect { half, .. } => half.x.min(half.y) * 2.0,
    }
}

/// Delays decided headings by the reaction time: `sample` returns the newest heading
/// decided at least `delay` seconds ago.
#[derive(Clone, Debug, Default)]
pub struct ReactionBuffer {
    pending: VecDeque<(f64, V2)>,
    current: V2,
}

impl ReactionBuffer {
    pub fn push(&mut self, time: f64, direction: V2) {
        self.pending.push_back((time, direction));
    }

    pub fn sample(&mut self, now: f64, delay: f32) -> V2 {
        let ready = now - f64::from(delay.max(0.0));
        while let Some(&(time, direction)) = self.pending.front() {
            if time > ready + 1e-6 {
                break;
            }
            self.current = direction;
            self.pending.pop_front();
        }
        self.current
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.current = V2::ZERO;
    }
}

/// Moves `current` toward `target` at `rate` per second (exponential smoothing),
/// keeping the result inside the unit circle.
pub fn steer_toward(current: V2, target: V2, rate: f32, dt: f32) -> V2 {
    let blend = (rate.max(0.0) * dt.max(0.0)).min(1.0);
    let out = current + (target - current) * blend;
    if !finite(out) {
        return V2::ZERO;
    }
    let length = out.length();
    if length > 1.0 {
        out * (1.0 / length)
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARENA_MAX: V2 = V2::new(1280.0, 720.0);
    const CENTER: V2 = V2::new(640.0, 360.0);

    struct World {
        position: V2,
        shapes: Vec<DangerShape>,
        enemies: Vec<BotEnemy>,
        teammates: Vec<BotTeammate>,
        tuning: BotTuning,
    }

    impl World {
        fn new(position: V2, shapes: Vec<DangerShape>) -> Self {
            Self {
                position,
                shapes,
                enemies: vec![],
                teammates: vec![],
                tuning: BotTuning::for_skill(SKILL_NORMAL),
            }
        }

        fn decide_with(&self, snapshot: &DangerSnapshot, previous: V2, tick: u64) -> V2 {
            decide(&BotInput {
                position: self.position,
                radius: 11.0,
                speed: 220.0,
                range_radius: 140.0,
                arena_min: V2::ZERO,
                arena_max: ARENA_MAX,
                danger: snapshot,
                enemies: &self.enemies,
                teammates: &self.teammates,
                previous,
                tuning: &self.tuning,
                seed: 7,
                tick,
            })
        }

        fn decide(&self) -> V2 {
            self.decide_with(&DangerSnapshot::new(self.shapes.clone()), V2::ZERO, 0)
        }

        /// Runs the bot for `seconds` at 60 Hz the way `BotBrain` drives it (decisions
        /// every `decision_interval`, reaction delay, heading smoothing) with shapes
        /// advancing in time. Returns the number of frames it was hit.
        fn simulate(&mut self, seconds: f32) -> u32 {
            let dt = 1.0 / 60.0;
            let mut elapsed = 0.0f32;
            let mut next_decision = 0.0f32;
            let mut buffer = ReactionBuffer::default();
            let mut heading = V2::ZERO;
            let mut intent = V2::ZERO;
            let mut hits = 0;
            let mut tick = 0;
            while elapsed < seconds {
                if elapsed >= next_decision {
                    next_decision = elapsed + self.tuning.decision_interval;
                    let snapshot = DangerSnapshot::new(
                        self.shapes.iter().map(|s| s.advanced(elapsed)).collect(),
                    );
                    intent = self.decide_with(&snapshot, intent, tick);
                    buffer.push(f64::from(elapsed), intent);
                    tick += 1;
                }
                let target = buffer.sample(f64::from(elapsed), self.tuning.reaction_delay);
                heading = steer_toward(heading, target, self.tuning.turn_rate, dt);
                self.position =
                    clamp_to(self.position + heading * (220.0 * dt), V2::ZERO, ARENA_MAX);
                elapsed += dt;
                let now =
                    DangerSnapshot::new(self.shapes.iter().map(|s| s.advanced(elapsed)).collect());
                if now.is_hit(self.position, 11.0, 0.0) {
                    hits += 1;
                }
            }
            hits
        }

        /// `simulate` for every skill except easy, from the same start.
        fn simulate_skilled(&self, seconds: f32) -> Vec<(i32, u32, V2)> {
            [SKILL_NORMAL, SKILL_HARD]
                .into_iter()
                .map(|skill| {
                    let mut world = World {
                        position: self.position,
                        shapes: self.shapes.clone(),
                        enemies: self.enemies.clone(),
                        teammates: self.teammates.clone(),
                        tuning: BotTuning::for_skill(skill),
                    };
                    let hits = world.simulate(seconds);
                    (skill, hits, world.position)
                })
                .collect()
        }
    }

    fn circle(center: V2, radius: f32, velocity: V2, activates_in: f32) -> DangerShape {
        DangerShape::Circle {
            center,
            radius,
            velocity,
            activates_in,
        }
    }

    #[test]
    fn stands_still_when_nothing_to_do() {
        let world = World::new(CENTER, vec![]);
        assert_eq!(world.decide(), V2::ZERO);
    }

    #[test]
    fn escapes_a_telegraphed_pulse_on_top_of_it() {
        let world = World::new(CENTER, vec![circle(CENTER, 90.0, V2::ZERO, 0.8)]);
        assert!(world.decide().length() > 0.9);
        for (skill, hits, end) in world.simulate_skilled(1.5) {
            assert_eq!(hits, 0, "skill {skill}");
            assert!(end.distance_to(CENTER) > 101.0, "skill {skill}");
        }
    }

    #[test]
    fn escapes_a_closing_circle() {
        // A big projectile heading straight at the bot.
        let world = World::new(
            CENTER,
            vec![circle(
                V2::new(340.0, 360.0),
                40.0,
                V2::new(300.0, 0.0),
                0.0,
            )],
        );
        for (skill, hits, _) in world.simulate_skilled(2.5) {
            assert_eq!(hits, 0, "skill {skill}");
        }
    }

    #[test]
    fn dodges_a_ring_of_bullets() {
        let origin = V2::new(560.0, 360.0);
        let shapes = (0..10)
            .map(|i| {
                let dir = V2::from_angle(i as f32 / 10.0 * std::f32::consts::TAU);
                circle(origin + dir * 20.0, 10.0, dir * 180.0, 0.0)
            })
            .collect();
        let world = World::new(CENTER, shapes);
        for (skill, hits, _) in world.simulate_skilled(2.0) {
            assert_eq!(hits, 0, "skill {skill}");
        }
    }

    #[test]
    fn does_not_step_into_a_laser_about_to_fire() {
        let laser = DangerShape::Capsule {
            a: V2::new(0.0, 420.0),
            b: V2::new(1280.0, 420.0),
            radius: 18.0,
            activates_in: 0.3,
        };
        let mut world = World::new(CENTER, vec![laser]);
        // A damageable enemy on the far side of the laser pulls the bot across it.
        world.enemies.push(BotEnemy {
            position: V2::new(640.0, 600.0),
            contact_radius: 16.0,
            damageable: true,
        });
        for (skill, hits, end) in world.simulate_skilled(1.0) {
            assert_eq!(hits, 0, "skill {skill}");
            assert!(end.y < 420.0 - 18.0 - 11.0, "skill {skill}");
        }
    }

    #[test]
    fn leaves_a_laser_cross() {
        let world = World::new(
            CENTER,
            vec![
                DangerShape::Capsule {
                    a: V2::new(0.0, 360.0),
                    b: V2::new(1280.0, 360.0),
                    radius: 22.0,
                    activates_in: 0.6,
                },
                DangerShape::Capsule {
                    a: V2::new(640.0, 0.0),
                    b: V2::new(640.0, 720.0),
                    radius: 22.0,
                    activates_in: 0.9,
                },
            ],
        );
        for (skill, hits, end) in world.simulate_skilled(2.0) {
            assert_eq!(hits, 0, "skill {skill}");
            assert!((end.x - 640.0).abs() > 33.0 && (end.y - 360.0).abs() > 33.0);
        }
    }

    #[test]
    fn reaches_an_enemy_across_open_ground() {
        let world = World::new(CENTER, vec![]);
        let mut world = World {
            enemies: vec![BotEnemy {
                position: V2::new(640.0, 600.0),
                contact_radius: 16.0,
                damageable: true,
            }],
            ..world
        };
        world.simulate(2.0);
        assert!(
            world.position.y > 450.0,
            "bot reached the enemy: {:?}",
            world.position
        );
    }

    /// A wall sweeping left across the whole arena with a 110 px gap at `gap_y`.
    fn wall_with_gap(x: f32, gap_y: f32, speed: f32) -> Vec<DangerShape> {
        let gap_half = 55.0;
        let top_half = (gap_y - gap_half) * 0.5;
        let bottom_half = (720.0 - gap_y - gap_half) * 0.5;
        let wall = |center_y: f32, half_y: f32| DangerShape::Rect {
            center: V2::new(x, center_y),
            half: V2::new(16.0, half_y),
            angle: 0.0,
            velocity: V2::new(-speed, 0.0),
            activates_in: 0.0,
        };
        vec![
            wall(top_half, top_half),
            wall(720.0 - bottom_half, bottom_half),
        ]
    }

    #[test]
    fn finds_the_gap_in_a_wall() {
        for (gap_y, speed) in [(480.0, 260.0), (620.0, 250.0), (120.0, 200.0)] {
            let world = World::new(CENTER, wall_with_gap(1100.0, gap_y, speed));
            for (skill, hits, _) in world.simulate_skilled(6.0) {
                assert_eq!(hits, 0, "skill {skill} gap {gap_y}");
            }
        }
    }

    #[test]
    fn samples_do_not_tunnel_through_a_fast_wall() {
        // Closing at 470 px/s against a 32 px thick wall: one decision would step
        // straight over it without swept sampling.
        let tuning = BotTuning::for_skill(SKILL_HARD);
        let snapshot = DangerSnapshot::new(wall_with_gap(700.0, 620.0, 250.0));
        let input = BotInput {
            position: CENTER,
            radius: 11.0,
            speed: 220.0,
            range_radius: 140.0,
            arena_min: V2::ZERO,
            arena_max: ARENA_MAX,
            danger: &snapshot,
            enemies: &[],
            teammates: &[],
            previous: V2::ZERO,
            tuning: &tuning,
            seed: 0,
            tick: 0,
        };
        let ctx = Context::new(&input, &tuning);
        let plan = Plan {
            direction: V2::new(1.0, 0.0),
            stop: tuning.horizon,
        };
        let (_, _, hit) = ctx.score(&plan, 0.0, f32::NEG_INFINITY).unwrap();
        assert!(hit, "moving into the wall predicts a hit");
    }

    #[test]
    fn prefers_the_matching_color_enemy() {
        let mut world = World::new(CENTER, vec![]);
        world.enemies = vec![
            BotEnemy {
                position: V2::new(400.0, 360.0),
                contact_radius: 16.0,
                damageable: false,
            },
            BotEnemy {
                position: V2::new(900.0, 360.0),
                contact_radius: 16.0,
                damageable: true,
            },
        ];
        assert!(world.decide().x > 0.7);
        world.simulate(3.0);
        let distance = world.position.distance_to(V2::new(900.0, 360.0));
        assert!(
            distance <= 140.0 * 2.0 / 3.0 + 1.0,
            "inside the 2-ray tier: {distance}"
        );
        assert!(distance > 16.0 + 11.0, "outside contact radius: {distance}");
    }

    #[test]
    fn goes_to_revive_a_downed_teammate() {
        let mut world = World::new(CENTER, vec![]);
        world.teammates = vec![BotTeammate {
            position: V2::new(300.0, 200.0),
            is_dead: true,
        }];
        world.enemies = vec![BotEnemy {
            position: V2::new(1000.0, 360.0),
            contact_radius: 16.0,
            damageable: true,
        }];
        assert!(world.decide().x < -0.5);
        world.simulate(3.0);
        assert!(world.position.distance_to(V2::new(300.0, 200.0)) < REVIVE_DISTANCE);
    }

    #[test]
    fn waits_out_danger_on_a_downed_teammate() {
        let mate = V2::new(400.0, 360.0);
        let mut world = World::new(CENTER, vec![circle(mate, 80.0, V2::ZERO, 0.0)]);
        world.teammates = vec![BotTeammate {
            position: mate,
            is_dead: true,
        }];
        assert_eq!(world.simulate(2.0), 0);
        assert!(world.position.distance_to(mate) < 200.0, "waits nearby");
    }

    #[test]
    fn avoids_arena_edges() {
        for position in [
            V2::new(5.0, 360.0),
            V2::new(1275.0, 360.0),
            V2::new(640.0, 5.0),
            V2::new(640.0, 715.0),
        ] {
            let world = World::new(position, vec![]);
            let direction = world.decide();
            let inward = (CENTER - position).normalized_or_zero();
            assert!(direction.dot(inward) > 0.5, "{position:?} -> {direction:?}");
        }
    }

    #[test]
    fn never_returns_nan() {
        let snapshot = DangerSnapshot::new(vec![
            DangerShape::Capsule {
                a: CENTER,
                b: CENTER,
                radius: 0.0,
                activates_in: 0.0,
            },
            circle(CENTER, f32::NAN, V2::ZERO, 0.0),
            circle(V2::new(f32::INFINITY, 0.0), 10.0, V2::ZERO, 0.0),
            DangerShape::Rect {
                center: CENTER,
                half: V2::ZERO,
                angle: f32::NAN,
                velocity: V2::ZERO,
                activates_in: -5.0,
            },
        ]);
        let mates = [BotTeammate {
            position: CENTER,
            is_dead: true,
        }];
        let enemies = [BotEnemy {
            position: CENTER,
            contact_radius: f32::NAN,
            damageable: true,
        }];
        let mut tunings = vec![BotTuning::default()];
        tunings.push(BotTuning {
            horizon: f32::NAN,
            horizon_steps: 0,
            directions: 0,
            danger_margin: 0.0,
            edge_margin: 0.0,
            ..BotTuning::default()
        });
        for tuning in &tunings {
            for (position, speed, arena_max) in [
                (CENTER, 220.0, ARENA_MAX),
                (CENTER, 0.0, ARENA_MAX),
                (V2::new(f32::NAN, 1.0), 220.0, ARENA_MAX),
                (CENTER, 220.0, V2::ZERO),
                (CENTER, f32::INFINITY, ARENA_MAX),
            ] {
                let input = BotInput {
                    position,
                    radius: 11.0,
                    speed,
                    range_radius: 0.0,
                    arena_min: V2::ZERO,
                    arena_max,
                    danger: &snapshot,
                    enemies: &enemies,
                    teammates: &mates,
                    previous: V2::new(f32::NAN, 0.0),
                    tuning,
                    seed: 1,
                    tick: 2,
                };
                let direction = decide(&input);
                assert!(finite(direction), "{direction:?}");
                assert!(direction.length() <= 1.0 + 1e-5);
            }
        }
    }

    #[test]
    fn deterministic_for_a_seed() {
        let mut world = World::new(
            CENTER,
            vec![circle(
                V2::new(700.0, 360.0),
                60.0,
                V2::new(-50.0, 10.0),
                0.2,
            )],
        );
        world.tuning = BotTuning::for_skill(SKILL_EASY);
        let snapshot = DangerSnapshot::new(world.shapes.clone());
        for tick in 0..20 {
            assert_eq!(
                world.decide_with(&snapshot, V2::new(1.0, 0.0), tick),
                world.decide_with(&snapshot, V2::new(1.0, 0.0), tick)
            );
        }
        let mut a = World::new(CENTER, world.shapes.clone());
        let mut b = World::new(CENTER, world.shapes.clone());
        a.simulate(1.0);
        b.simulate(1.0);
        assert_eq!(a.position, b.position);
    }

    #[test]
    fn skills_scale_reaction_and_foresight() {
        let easy = BotTuning::for_skill(SKILL_EASY);
        let normal = BotTuning::for_skill(SKILL_NORMAL);
        let hard = BotTuning::for_skill(SKILL_HARD);
        assert!(easy.reaction_delay > normal.reaction_delay);
        assert!(normal.reaction_delay > hard.reaction_delay);
        assert!(easy.horizon < hard.horizon);
        assert!(easy.noise > hard.noise);
        assert_eq!(BotTuning::for_skill(99), normal);
        assert_eq!(BotTuning::skill_name(SKILL_HARD), "hard");
    }

    #[test]
    fn reaction_buffer_delays_headings() {
        let mut buffer = ReactionBuffer::default();
        buffer.push(0.0, V2::new(1.0, 0.0));
        buffer.push(0.05, V2::new(0.0, 1.0));
        assert_eq!(buffer.sample(0.05, 0.1), V2::ZERO);
        assert_eq!(buffer.sample(0.1, 0.1), V2::new(1.0, 0.0));
        assert_eq!(buffer.sample(0.2, 0.1), V2::new(0.0, 1.0));
        assert_eq!(buffer.sample(0.3, 0.0), V2::new(0.0, 1.0));
        buffer.clear();
        assert_eq!(buffer.sample(1.0, 0.0), V2::ZERO);
    }

    #[test]
    fn steering_converges_and_stays_bounded() {
        let mut heading = V2::ZERO;
        for _ in 0..60 {
            heading = steer_toward(heading, V2::new(0.0, 1.0), 15.0, 1.0 / 60.0);
            assert!(heading.length() <= 1.0 + 1e-6);
        }
        assert!(heading.distance_to(V2::new(0.0, 1.0)) < 0.01);
        assert_eq!(
            steer_toward(heading, V2::new(f32::NAN, 0.0), 15.0, 0.1),
            V2::ZERO
        );
    }
}
