//! `SpawnEnemy` handler: places an enemy from the level's `enemy_pool`.
//!
//! Inside spawns play the spawn effect for the event's telegraph time and place the
//! enemy when it finishes; outside spawns (chasers) appear just beyond the arena edge
//! at `params.angle`. Enemy health scales with the player count.
//!
//! `params.count` > 1 spawns a group (a swarm) spread around the spawn point, and
//! `params.duration_beats` > 0 is each enemy's lifetime (it leaves after that long).

use crate::core::chart::ChartEvent;
use crate::director::LevelDirector;
use crate::game_config::PlayerConfig;
use crate::groups;
use godot::classes::{GpuParticles2D, Node, Node2D, PackedScene};
use godot::prelude::*;

/// Extra distance beyond the arena's circumscribed circle for outside spawns.
const OUTSIDE_MARGIN: f32 = 50.0;
/// Spacing (px) between members of a group spawned inside the arena.
const GROUP_SPREAD: f32 = 46.0;
/// Angle (radians) between members of a group spawned outside the arena.
const GROUP_ANGLE: f32 = 0.22;

pub fn spawn_enemy(director: &mut LevelDirector, event: &ChartEvent) {
    let index = event.params.variant as usize;
    let Some(entry) = director
        .level()
        .and_then(|level| level.enemy_pool.get(index))
        .cloned()
    else {
        return;
    };
    let Some(scene) = director.enemy_scene(index) else {
        godot_warn!(
            "LevelDirector: enemy scene '{}' failed to load",
            entry.scene
        );
        return;
    };
    let count = event.params.count.max(1);
    let lifetime = event.params.duration_beats;
    if entry.spawn_outside {
        let arena = director.arena_rect();
        for k in 0..count {
            let angle = event.params.angle + GROUP_ANGLE * (k as f32 - (count - 1) as f32 / 2.0);
            let direction = Vector2::new(angle.cos(), angle.sin());
            let position =
                arena.center() + direction * (arena.size.length() / 2.0 + OUTSIDE_MARGIN);
            place_enemy(
                &scene,
                position,
                director.level_root(),
                director.to_gd(),
                lifetime,
            );
        }
        return;
    }

    let center = director.arena_point(event.params.x, event.params.y);
    for k in 0..count {
        let position = center + group_offset(k, count);
        spawn_at(
            director,
            scene.clone(),
            position,
            event.telegraph_beats,
            lifetime,
        );
    }
}

/// Offset of member `k` of a group of `count`: the first at the center, the rest on a
/// ring around it.
fn group_offset(k: u32, count: u32) -> Vector2 {
    if k == 0 || count <= 1 {
        return Vector2::ZERO;
    }
    let angle = std::f32::consts::TAU * (k - 1) as f32 / (count - 1) as f32;
    Vector2::new(angle.cos(), angle.sin()) * GROUP_SPREAD
}

/// Plays the spawn effect at `position` for `telegraph_beats`, then places an enemy
/// from `scene` there (immediately when the effect scene is missing). The enemy
/// leaves after `lifetime_beats` (0 = stays until killed).
pub fn spawn_at(
    director: &mut LevelDirector,
    scene: Gd<PackedScene>,
    position: Vector2,
    telegraph_beats: f64,
    lifetime_beats: f64,
) {
    let parent = director.level_root();
    let director_gd = director.to_gd();
    let Some(effect_scene) = director.spawn_effect_scene() else {
        place_enemy(&scene, position, parent, director_gd, lifetime_beats);
        return;
    };
    let mut effect = effect_scene.instantiate_as::<GpuParticles2D>();
    let telegraph_seconds = director
        .timing()
        .beats_to_duration(telegraph_beats)
        .max(0.1);
    effect.set("duration", &telegraph_seconds.to_variant());
    effect.set_global_position(position);
    effect.add_to_group(groups::SPAWN_EFFECTS);
    let mut effect_parent = parent.clone();
    effect_parent.add_child(&effect);
    effect.connect(
        "spawn_ready",
        &Callable::from_fn("place_enemy", move |_args| {
            place_enemy(
                &scene,
                position,
                parent.clone(),
                director_gd.clone(),
                lifetime_beats,
            );
            Variant::nil()
        }),
    );
}

fn place_enemy(
    scene: &Gd<PackedScene>,
    position: Vector2,
    mut parent: Gd<Node>,
    director: Gd<LevelDirector>,
    lifetime_beats: f64,
) {
    if !parent.is_instance_valid() || !director.is_instance_valid() {
        return;
    }
    let mut enemy = scene.instantiate_as::<Node2D>();
    enemy.set_global_position(position);
    if lifetime_beats > 0.0 {
        enemy.set("lifetime_beats", &lifetime_beats.to_variant());
    }
    parent.add_child(&enemy);

    scale_health(&enemy.clone().upcast(), &parent);

    enemy.connect("died", &director.callable("_on_enemy_died"));
    let mut director = director;
    director.emit_signal("enemy_spawned", &[enemy.to_variant()]);
}

/// Multiplies an enemy's `max_life` (and life) by the number of seats. `node` is any
/// node in the tree (used to reach `GameConfig`).
pub fn scale_health(enemy: &Gd<Node>, node: &Gd<Node>) {
    let players = player_count(node);
    if players > 1
        && let Some(mut health) = enemy.get_node_or_null("HealthComponent")
    {
        let max_life = health.get("max_life").try_to::<f32>().unwrap_or(1.0) * players as f32;
        health.set("max_life", &max_life.to_variant());
        health.set("life", &max_life.to_variant());
    }
}

fn player_count(node: &Gd<Node>) -> usize {
    node.get_node_or_null("/root/GameConfig")
        .and_then(|config| {
            config
                .get("players")
                .try_to::<Array<Gd<PlayerConfig>>>()
                .ok()
        })
        .map_or(1, |players| players.len().max(1))
}
