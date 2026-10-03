//! Shield-puzzle components.
//!
//! - `SplitComponent`: on death the enemy breaks into smaller enemies, each with a
//!   single shield in a different player's color.
//! - `ChameleonComponent`: the enemy's shield colors rotate through the players'
//!   colors every `every_beats`, so the team has to hand it off.
//! - `WardComponent`: every volley, enemies inside its ring gain a ward (an outer
//!   shield layer in the Warden's color) that lasts until the Warden dies.

use super::{BeatDriver, HealthComponent, load_packed_scene, parent_as_node2d, player_colors};
use crate::core::beat_motion::cycle_index;
use crate::enemy_spawn;
use crate::fx::{BurstStyle, with_fx};
use crate::groups;
use godot::classes::{INode, INode2D, Node, Node2D, PackedScene};
use godot::global::randi_range;
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct SplitComponent {
    #[var]
    #[init(val = GString::from("res://scenes/splitter_mini_enemy.tscn"))]
    piece_scene: GString,
    #[var]
    #[init(val = 2)]
    pieces: i32,
    /// Distance (px) each piece appears from the center.
    #[var]
    #[init(val = 34.0)]
    spread: f32,

    scene: Option<Gd<PackedScene>>,
    /// Player colors, read at spawn: deaths happen while the killing player is busy
    /// (its properties cannot be read then).
    colors: Vec<Color>,
    base: Base<Node>,
}

#[godot_api]
impl SplitComponent {
    #[func]
    fn _on_parent_died(&mut self) {
        let Some(parent) = parent_as_node2d(self.base().get_parent()) else {
            return;
        };
        let Some(container) = parent.get_parent() else {
            return;
        };
        let Some(scene) = self.scene.clone() else {
            return;
        };
        let colors = self.colors.clone();
        let seed = randi_range(0, 7) as usize;
        let origin = parent.get_global_position();
        // Who listens to the parent's death (director, manager) hears the pieces too;
        // the parent's own children are skipped.
        let listeners: Vec<Callable> = parent
            .get_signal_connection_list("died")
            .iter_shared()
            .filter_map(|c| c.get("callable")?.try_to::<Callable>().ok())
            .filter(|callable| {
                callable
                    .object()
                    .and_then(|o| o.try_cast::<Node>().ok())
                    .is_none_or(|n| n != parent.clone().upcast() && !parent.is_ancestor_of(&n))
            })
            .collect();
        let count = self.pieces.max(1);
        let facing = (seed as f32) * TAU / 8.0;
        for k in 0..count {
            let Some(mut piece) = scene.try_instantiate_as::<Node2D>() else {
                continue;
            };
            let angle = facing + k as f32 * TAU / count as f32;
            piece.set_position(origin + Vector2::from_angle(angle) * self.spread);
            if let Some(mut health) = piece.get_node_or_null("HealthComponent")
                && !colors.is_empty()
            {
                let mut layer = PackedColorArray::new();
                layer.push(colors[(seed + k as usize) % colors.len()]);
                health.set("shield_colors", &layer.to_variant());
            }
            enemy_spawn::scale_health(&piece.clone().upcast(), &container);
            for callable in &listeners {
                piece.connect("died", callable);
            }
            // Deferred: deaths happen inside physics callbacks.
            container
                .clone()
                .call_deferred("add_child", &[piece.to_variant()]);
        }
        with_fx(|fx| {
            fx.burst_style(
                origin,
                crate::visuals::enemy_visual::ENEMY_METAL,
                12,
                BurstStyle::Shards as i32,
                1.0,
            )
        });
    }
}

#[godot_api]
impl INode for SplitComponent {
    fn ready(&mut self) {
        self.scene = load_packed_scene(&self.piece_scene.to_string());
        self.colors = player_colors(&self.base().get_tree());
        if let Some(mut parent) = self.base().get_parent()
            && parent.has_signal("died")
        {
            let this = self.to_gd();
            parent.connect("died", &this.callable("_on_parent_died"));
        }
    }
}

/// Rotates the enemy's shield colors through the players' colors every
/// `every_beats` (on the bar line). Draws the coming color as a flickering outer ring
/// during the wind-up. With fewer than two player colors nothing changes.
#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct ChameleonComponent {
    #[var]
    #[init(val = 8.0)]
    every_beats: f64,
    #[var]
    #[init(val = 1.0)]
    windup_beats: f64,

    driver: BeatDriver,
    colors: Vec<Color>,
    seed: usize,
    current: Option<usize>,
    base: Base<Node2D>,
}

#[godot_api]
impl ChameleonComponent {
    /// The shield colors changed.
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        if self.colors.len() < 2 {
            0.0
        } else {
            self.driver.windup()
        }
    }

    /// Active-layer color after the next change.
    #[func]
    fn get_next_color(&self) -> Color {
        self.color_at(self.driver.tracker.next_beat() + 0.01, 0)
    }
}

impl ChameleonComponent {
    fn color_at(&self, beat: f64, layer: usize) -> Color {
        if self.colors.is_empty() {
            return Color::WHITE;
        }
        let index = cycle_index(beat, self.every_beats, self.seed, self.colors.len());
        self.colors[(index + layer) % self.colors.len()]
    }

    fn health(&self) -> Option<Gd<HealthComponent>> {
        self.base()
            .get_parent()?
            .try_get_node_as::<HealthComponent>("HealthComponent")
    }

    fn recolor(&mut self) {
        let Some(mut health) = self.health() else {
            return;
        };
        let index = cycle_index(
            self.driver.beat,
            self.every_beats,
            self.seed,
            self.colors.len(),
        );
        if self.current == Some(index) {
            return;
        }
        self.current = Some(index);
        let beat = self.driver.beat;
        let mut health = health.bind_mut();
        let first = usize::from(health.has_ward());
        for layer in first..health.get_layer_count() as usize {
            let color = self.color_at(beat, layer - first);
            health.set_layer_color(layer as i32, color);
        }
    }
}

#[godot_api]
impl INode2D for ChameleonComponent {
    fn ready(&mut self) {
        self.driver
            .configure(self.every_beats, 0.0, self.windup_beats);
        self.seed = randi_range(0, 7) as usize;
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        let fired = self.driver.tick(&node, delta);
        if self.colors.is_empty() {
            self.colors = player_colors(&self.base().get_tree());
        }
        if self.colors.len() >= 2 {
            self.recolor();
            if let Some(index) = fired {
                let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
                self.signals().acted().emit(action, beat);
                let pos = self.base().get_global_position();
                let color = self.color_at(beat, 0);
                with_fx(|fx| fx.ring(pos, color, 70.0, 0.3));
            }
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let w = self.get_windup();
        if w <= 0.0 {
            return;
        }
        let next = self.get_next_color();
        let on = w < 0.75 || (self.driver.beat * 12.0).fract() < 0.5;
        let alpha = if on { 0.3 + 0.6 * w } else { 0.15 };
        let radius = 62.0 - 14.0 * w;
        self.base_mut()
            .draw_arc_ex(
                Vector2::ZERO,
                radius,
                0.0,
                TAU,
                40,
                Color::from_rgba(next.r, next.g, next.b, alpha),
            )
            .width(3.0 + 2.0 * w)
            .antialiased(true)
            .done();
    }
}

/// Every volley, wards every other enemy inside `radius`: an outer shield layer in
/// the Warden's color (its outermost shield at spawn). Wards drop when the Warden
/// dies or leaves the tree. Draws its ring and a tether to each warded enemy.
#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct WardComponent {
    #[var]
    #[init(val = 4.0)]
    every_beats: f64,
    #[var]
    offset_beats: f64,
    #[var]
    #[init(val = 1.0)]
    windup_beats: f64,
    #[var]
    #[init(val = 220.0)]
    radius: f32,

    driver: BeatDriver,
    color: Option<Color>,
    warded: Vec<InstanceId>,
    pulse: f32,
    base: Base<Node2D>,
}

#[godot_api]
impl WardComponent {
    /// A ward pulse went out.
    #[signal]
    fn acted(action_beat: f64, song_beat: f64);

    #[func]
    fn get_cadence(&self) -> Vector3 {
        self.driver.cadence_vector()
    }

    #[func]
    fn get_windup(&self) -> f32 {
        self.driver.windup()
    }

    #[func]
    fn get_ward_color(&self) -> Color {
        self.color.unwrap_or(Color::WHITE)
    }

    /// Number of living enemies currently carrying this Warden's ward.
    #[func]
    fn get_warded_count(&self) -> i32 {
        self.warded_healths().len() as i32
    }

    #[func]
    fn _on_parent_died(&mut self) {
        for health in self.warded_healths() {
            let pos = health.get_global_position();
            let color = self.get_ward_color();
            with_fx(|fx| {
                fx.burst_style(pos, color, 10, BurstStyle::Shards as i32, 0.9);
            });
        }
        self.revoke_all();
    }
}

impl WardComponent {
    fn source_id(&self) -> i64 {
        self.base()
            .get_parent()
            .map_or(0, |p| p.instance_id().to_i64())
    }

    fn warded_healths(&self) -> Vec<Gd<HealthComponent>> {
        self.warded
            .iter()
            .filter_map(|id| Gd::<Node>::try_from_instance_id(*id).ok())
            .filter(|n| n.is_inside_tree() && !n.is_queued_for_deletion())
            .filter_map(|n| n.try_get_node_as::<HealthComponent>("HealthComponent"))
            .filter(|h| h.bind().has_ward())
            .collect()
    }

    fn revoke_all(&mut self) {
        let source = self.source_id();
        for id in std::mem::take(&mut self.warded) {
            if let Ok(node) = Gd::<Node>::try_from_instance_id(id)
                && let Some(mut health) = node.try_get_node_as::<HealthComponent>("HealthComponent")
            {
                health.bind_mut().revoke_ward(source);
            }
        }
    }

    fn ward_nearby(&mut self, parent: &Gd<Node2D>) {
        let Some(color) = self.color else {
            return;
        };
        let source = self.source_id();
        let origin = parent.get_global_position();
        let parent_id = parent.instance_id();
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            if node.instance_id() == parent_id || node.is_queued_for_deletion() {
                continue;
            }
            let Ok(enemy) = node.try_cast::<Node2D>() else {
                continue;
            };
            if enemy.get_global_position().distance_to(origin) > self.radius {
                continue;
            }
            // Wardens never ward each other.
            if enemy.has_node("WardComponent") {
                continue;
            }
            let Some(mut health) = enemy.try_get_node_as::<HealthComponent>("HealthComponent")
            else {
                continue;
            };
            if health.bind_mut().grant_ward(color, source) {
                let id = enemy.instance_id();
                if !self.warded.contains(&id) {
                    self.warded.push(id);
                }
                let pos = enemy.get_global_position();
                with_fx(|fx| fx.ring(pos, color, 50.0, 0.25));
            }
        }
    }
}

#[godot_api]
impl INode2D for WardComponent {
    fn ready(&mut self) {
        self.driver
            .configure(self.every_beats, self.offset_beats, self.windup_beats);
        if let Some(mut parent) = self.base().get_parent()
            && parent.has_signal("died")
        {
            let this = self.to_gd();
            parent.connect("died", &this.callable("_on_parent_died"));
        }
    }

    fn exit_tree(&mut self) {
        self.revoke_all();
    }

    fn process(&mut self, delta: f64) {
        let node = self.to_gd().upcast::<Node>();
        let fired = self.driver.tick(&node, delta);
        if self.color.is_none() {
            self.color = self
                .base()
                .get_parent()
                .and_then(|p| p.try_get_node_as::<HealthComponent>("HealthComponent"))
                .and_then(|h| h.bind().shield_colors.get(0));
        }
        self.pulse = (self.pulse - delta as f32 * 3.0).max(0.0);
        if let Some(index) = fired
            && let Some(parent) = parent_as_node2d(self.base().get_parent())
        {
            self.ward_nearby(&parent);
            self.pulse = 1.0;
            let (action, beat) = (self.driver.action_beat(index), self.driver.beat);
            self.signals().acted().emit(action, beat);
            let pos = parent.get_global_position();
            let (color, radius) = (self.get_ward_color(), self.radius);
            with_fx(|fx| fx.ring(pos, color, radius, 0.35));
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let Some(color) = self.color else {
            return;
        };
        let w = self.driver.windup();
        let alpha = 0.12 + 0.25 * w + 0.4 * self.pulse;
        // Dashed boundary turning slowly.
        let dashes = 24;
        let spin = self.driver.beat as f32 * 0.15;
        for i in 0..dashes {
            let a0 = spin + i as f32 * TAU / dashes as f32;
            let radius = self.radius;
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    radius,
                    a0,
                    a0 + TAU / dashes as f32 * 0.5,
                    4,
                    Color::from_rgba(color.r, color.g, color.b, alpha),
                )
                .width(2.0)
                .done();
        }
        let origin = self.base().get_global_position();
        let tethers: Vec<Vector2> = self
            .warded_healths()
            .iter()
            .map(|h| h.get_global_position() - origin)
            .collect();
        let pulse = self.pulse;
        for end in tethers {
            self.base_mut()
                .draw_line_ex(
                    Vector2::ZERO,
                    end,
                    Color::from_rgba(color.r, color.g, color.b, 0.3 + 0.3 * pulse),
                )
                .width(2.0)
                .antialiased(true)
                .done();
        }
    }
}
