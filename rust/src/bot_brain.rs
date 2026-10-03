//! `BotBrain`: drives its parent `Player` (input type `GameConfig.BOT`) from
//! `core::bot::decide`.
//!
//! Every `decision_interval` seconds of game time it builds a `BotInput` from the
//! level's `DangerField` snapshot (or, without one, from the `danger` group directly)
//! plus pending enemy spawn effects,
//! the `enemies` group (shield state from each `HealthComponent`) and the other
//! `players`, then queues the decided heading in a `ReactionBuffer`. Each physics frame
//! the heading released by the reaction delay is smoothed toward and exposed through
//! `get_move_direction()`, which the Player reads instead of `Input`.

use crate::core::bot::{
    self, BotEnemy, BotInput, BotTeammate, BotTuning, ReactionBuffer, steer_toward,
};
use crate::core::danger::{DangerShape, DangerSnapshot, V2};
use crate::danger_field::DangerField;
use crate::groups;
use crate::hazards::{PLAYER_HIT_RADIUS, to_v2, to_vector2};
use crate::player::Player;
use crate::util::dict_set;
use godot::classes::{INode, Node, Node2D, Time, Timer};
use godot::prelude::*;

/// Smallest distance bots keep from an enemy they attack; enemies with a longer reach
/// report it through `get_threat_radius()` (their shapes are in the danger snapshot).
const ENEMY_CONTACT_RADIUS: f32 = 24.0;
/// Danger radius around a pending enemy spawn (the largest enemy contact circle).
const SPAWN_DANGER_RADIUS: f32 = 28.0;
/// Seconds of float error tolerated when a decision slot comes due.
const DECISION_SLACK: f64 = 1e-4;

#[derive(GodotClass)]
#[class(base = Node)]
pub struct BotBrain {
    /// `GameConfig.BOT_EASY` / `BOT_NORMAL` / `BOT_HARD`.
    #[var(get = get_skill, set = set_skill)]
    skill: i32,
    /// Deterministic noise seed (the manager uses the seat index).
    #[var]
    seed: i64,
    /// When false the bot stands still.
    #[var]
    enabled: bool,

    tuning: BotTuning,
    buffer: ReactionBuffer,
    heading: V2,
    /// Latest decided heading (before the reaction delay), for hysteresis.
    intent: V2,
    time: f64,
    next_decision: f64,
    tick: u64,
    field: Option<Gd<DangerField>>,
    field_searched: bool,
    danger: DangerSnapshot,
    enemies: Vec<BotEnemy>,
    teammates: Vec<BotTeammate>,

    decisions: i64,
    decide_usec: u64,
    max_decide_usec: u64,
    hit_predictions: i64,
    last_danger: f32,

    base: Base<Node>,
}

#[godot_api]
impl INode for BotBrain {
    fn init(base: Base<Node>) -> Self {
        Self {
            skill: bot::SKILL_NORMAL,
            seed: 0,
            enabled: true,
            tuning: BotTuning::for_skill(bot::SKILL_NORMAL),
            buffer: ReactionBuffer::default(),
            heading: V2::ZERO,
            intent: V2::ZERO,
            time: 0.0,
            next_decision: 0.0,
            tick: 0,
            field: None,
            field_searched: false,
            danger: DangerSnapshot::default(),
            enemies: Vec::new(),
            teammates: Vec::new(),
            decisions: 0,
            decide_usec: 0,
            max_decide_usec: 0,
            hit_predictions: 0,
            last_danger: 0.0,
            base,
        }
    }

    fn physics_process(&mut self, delta: f64) {
        self.time += delta;
        let Some(player) = self.player() else {
            return;
        };
        let dead = player.bind().is_dead;
        let origin = player.get_global_position();
        if dead || !self.enabled {
            self.buffer.clear();
            self.heading = V2::ZERO;
            self.intent = V2::ZERO;
            return;
        }
        // Fixed cadence from the previous slot, so float drift never skips a frame.
        if self.time + DECISION_SLACK >= self.next_decision {
            let interval = f64::from(self.tuning.decision_interval);
            self.next_decision = (self.next_decision + interval).max(self.time + interval * 0.5);
            let direction = self.decide(&player, origin);
            self.buffer.push(self.time, direction);
        }
        let target = self.buffer.sample(self.time, self.tuning.reaction_delay);
        self.heading = steer_toward(self.heading, target, self.tuning.turn_rate, delta as f32);
    }
}

#[godot_api]
impl BotBrain {
    /// Heading the Player should move along this frame (length `0..=1`).
    #[func]
    pub fn get_move_direction(&self) -> Vector2 {
        to_vector2(self.heading)
    }

    #[func]
    pub fn get_skill(&self) -> i32 {
        self.skill
    }

    #[func]
    pub fn set_skill(&mut self, skill: i32) {
        self.skill = skill.clamp(bot::SKILL_EASY, bot::SKILL_HARD);
        self.tuning = BotTuning::for_skill(self.skill);
    }

    /// Decision stats: `decisions`, `avg_decide_usec`, `max_decide_usec`,
    /// `hit_predictions` (decisions whose best option still predicted a hit),
    /// `last_danger`, `skill`.
    #[func]
    pub fn get_stats(&self) -> VarDictionary {
        let mut dict = VarDictionary::new();
        let avg = if self.decisions > 0 {
            self.decide_usec as f64 / self.decisions as f64
        } else {
            0.0
        };
        dict_set(&mut dict, "decisions", self.decisions);
        dict_set(&mut dict, "avg_decide_usec", avg);
        dict_set(&mut dict, "max_decide_usec", self.max_decide_usec as i64);
        dict_set(&mut dict, "hit_predictions", self.hit_predictions);
        dict_set(&mut dict, "last_danger", self.last_danger);
        dict_set(&mut dict, "skill", self.skill);
        dict
    }
}

impl BotBrain {
    fn player(&self) -> Option<Gd<Player>> {
        self.base().get_parent()?.try_cast::<Player>().ok()
    }

    fn decide(&mut self, player: &Gd<Player>, origin: Vector2) -> V2 {
        let started = Time::singleton().get_ticks_usec();
        let (speed, range_radius, color) = {
            let p = player.bind();
            (p.speed, p.range_radius, p.team_color)
        };
        self.gather_enemies(color);
        self.gather_teammates(player);
        let arena = self
            .base()
            .get_viewport()
            .map(|viewport| viewport.get_visible_rect())
            .unwrap_or(Rect2::new(Vector2::ZERO, Vector2::new(1280.0, 720.0)));

        self.gather_danger();
        let decision = bot::decide_scored(&BotInput {
            position: to_v2(origin),
            radius: PLAYER_HIT_RADIUS,
            speed,
            range_radius,
            arena_min: to_v2(arena.position),
            arena_max: to_v2(arena.end()),
            danger: &self.danger,
            enemies: &self.enemies,
            teammates: &self.teammates,
            previous: self.intent,
            tuning: &self.tuning,
            seed: self.seed as u64,
            tick: self.tick,
        });

        self.intent = decision.direction;
        self.tick += 1;
        self.decisions += 1;
        self.last_danger = decision.danger;
        if decision.predicts_hit {
            self.hit_predictions += 1;
        }
        let spent = Time::singleton().get_ticks_usec().saturating_sub(started);
        self.decide_usec += spent;
        self.max_decide_usec = self.max_decide_usec.max(spent);
        decision.direction
    }

    /// The level's `DangerField`: a sibling of the Player, or anywhere in the current
    /// scene. Searched once; `None` when the level has none.
    fn danger_field(&mut self) -> Option<Gd<DangerField>> {
        if let Some(field) = &self.field
            && field.is_instance_valid()
        {
            return Some(field.clone());
        }
        if self.field_searched {
            return None;
        }
        self.field_searched = true;
        let sibling = self
            .player()
            .and_then(|player| player.get_parent())
            .and_then(|level| level.get_node_or_null("DangerField"));
        let found = sibling.or_else(|| {
            self.base()
                .get_tree()
                .get_current_scene()
                .and_then(|scene| scene.find_child_ex("DangerField").owned(false).done())
        });
        self.field = found.and_then(|node| node.try_cast::<DangerField>().ok());
        self.field.clone()
    }

    /// The `DangerField` snapshot (or the `danger` group read directly when the level
    /// has no field) plus pending enemy spawns.
    fn gather_danger(&mut self) {
        self.danger.clear();
        match self.danger_field() {
            Some(field) => self
                .danger
                .shapes
                .extend_from_slice(&field.bind().snapshot().shapes),
            None => self.gather_danger_group(),
        }
        self.gather_spawn_effects();
    }

    /// Enemies appear where their spawn effect plays; treat each as a circle that
    /// activates when its timer runs out.
    fn gather_spawn_effects(&mut self) {
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::SPAWN_EFFECTS).iter_shared() {
            if node.is_queued_for_deletion() {
                continue;
            }
            let Ok(effect) = node.try_cast::<Node2D>() else {
                continue;
            };
            let activates_in = effect
                .get_children()
                .iter_shared()
                .find_map(|child| child.try_cast::<Timer>().ok())
                .map_or(0.0, |timer| timer.get_time_left() as f32);
            self.danger.shapes.push(DangerShape::Circle {
                center: to_v2(effect.get_global_position()),
                radius: SPAWN_DANGER_RADIUS,
                velocity: V2::ZERO,
                activates_in,
            });
        }
    }

    fn gather_danger_group(&mut self) {
        let tree = self.base().get_tree();
        for mut node in tree.get_nodes_in_group(groups::DANGER).iter_shared() {
            if node.is_queued_for_deletion() || !node.has_method("danger_shapes") {
                continue;
            }
            if let Ok(records) = node
                .call("danger_shapes", &[])
                .try_to::<PackedFloat32Array>()
            {
                self.danger.extend_from_records(records.as_slice());
            }
        }
    }

    fn gather_enemies(&mut self, color: Color) {
        self.enemies.clear();
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            if node.is_queued_for_deletion() {
                continue;
            }
            let Ok(enemy) = node.try_cast::<Node2D>() else {
                continue;
            };
            let damageable = match enemy.get_node_or_null("HealthComponent") {
                Some(mut health) => {
                    let layer = health.call("get_active_layer", &[]).try_to::<i32>();
                    match layer {
                        Ok(layer) if layer >= 0 => health
                            .call("get_active_color", &[])
                            .try_to::<Color>()
                            .is_ok_and(|shield| shield_matches(shield, color)),
                        _ => true,
                    }
                }
                None => true,
            };
            let contact_radius = if enemy.has_method("get_threat_radius") {
                enemy
                    .clone()
                    .call("get_threat_radius", &[])
                    .try_to::<f32>()
                    .unwrap_or(ENEMY_CONTACT_RADIUS)
                    .max(ENEMY_CONTACT_RADIUS)
            } else {
                ENEMY_CONTACT_RADIUS
            };
            self.enemies.push(BotEnemy {
                position: to_v2(enemy.get_global_position()),
                contact_radius,
                damageable,
            });
        }
    }

    fn gather_teammates(&mut self, me: &Gd<Player>) {
        self.teammates.clear();
        let my_id = me.instance_id();
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::PLAYERS).iter_shared() {
            if node.instance_id() == my_id {
                continue;
            }
            let Ok(mate) = node.try_cast::<Player>() else {
                continue;
            };
            self.teammates.push(BotTeammate {
                position: to_v2(mate.get_global_position()),
                is_dead: mate.bind().is_dead,
            });
        }
    }
}

/// Same rule as `HealthComponent`: RGB equality.
fn shield_matches(shield: Color, color: Color) -> bool {
    shield.r == color.r && shield.g == color.g && shield.b == color.b
}
