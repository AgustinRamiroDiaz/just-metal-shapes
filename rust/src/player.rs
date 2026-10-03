use crate::fx::{BurstStyle, with_fx};
use crate::state_machine::StateMachine;
use godot::classes::{
    Area2D, CharacterBody2D, CircleShape2D, CollisionShape2D, ICharacterBody2D, INode, INode2D,
    Input, Node, Node2D, ShaderMaterial, Sprite2D, Texture2D,
};
use godot::global::randf_range;
use godot::prelude::*;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlayerState {
    Idle = 0,
    Attacking = 1,
    Dead = 2,
}

#[derive(GodotClass)]
#[class(init, base = CharacterBody2D)]
pub struct Player {
    #[export]
    pub speed: f32,
    #[export]
    pub range_radius: f32,
    #[export]
    pub damage_per_second: f32,
    #[export]
    pub team_color: Color,
    #[export]
    pub move_left_action: StringName,
    #[export]
    pub move_right_action: StringName,
    #[export]
    pub move_up_action: StringName,
    #[export]
    pub move_down_action: StringName,
    #[export]
    pub input_type: i32,
    #[export]
    pub joystick_deadzone: f32,
    #[export]
    pub face_tex_idle: Option<Gd<Texture2D>>,
    #[export]
    pub face_tex_attacking: Option<Gd<Texture2D>>,
    #[export]
    pub face_tex_dead: Option<Gd<Texture2D>>,

    #[var]
    pub is_dead: bool,
    #[var]
    pub revival_progress: f32,
    /// Ignores all damage (tests, debugging).
    #[var]
    pub god_mode: bool,

    #[var]
    pub lives: i32,
    pub invincible_timer: f32,
    pub targets_in_range: Vec<Gd<Node2D>>,

    sm: Option<Gd<StateMachine>>,

    base: Base<CharacterBody2D>,
}

const MAX_LIVES: i32 = 3;
const INVINCIBILITY_DURATION: f32 = 3.0;
const TIER_COUNT: i32 = 3;

#[godot_api]
impl Player {
    #[signal]
    fn died();
    #[signal]
    fn hit_enemy();
    #[signal]
    fn state_changed(from: i32, to: i32);
    /// Took a hit; `lives_left` is 0 when this hit downed the player.
    #[signal]
    fn damaged(lives_left: i32);
    #[signal]
    fn revived();

    #[constant]
    pub const MAX_LIVES: i32 = MAX_LIVES;

    fn apply_state_visuals(&mut self, to: i32) {
        let mut face_sprite = self.base().get_node_as::<Sprite2D>("FaceSprite");
        match to {
            0 => {
                // Idle
                if let Some(tex) = &self.face_tex_idle {
                    face_sprite.set_texture(tex);
                }
            }
            1 => {
                // Attacking
                if let Some(tex) = &self.face_tex_attacking {
                    face_sprite.set_texture(tex);
                }
            }
            2 => {
                // Dead
                if let Some(tex) = &self.face_tex_dead {
                    face_sprite.set_texture(tex);
                }
            }
            _ => {}
        }
    }

    fn transition_state(&mut self, next: i32) {
        if let Some(mut sm) = self.sm.clone() {
            let prev = sm.bind().get_current_state();
            if sm.bind_mut().transition(next) {
                self.apply_state_visuals(next);
                self.signals().state_changed().emit(prev, next);
            }
        }
    }

    fn force_state(&mut self, next: i32) {
        if let Some(mut sm) = self.sm.clone() {
            let prev = sm.bind().get_current_state();
            if prev != next {
                sm.bind_mut().force(next);
                self.apply_state_visuals(next);
                self.signals().state_changed().emit(prev, next);
            }
        }
    }

    #[func]
    fn _on_range_body_entered(&mut self, body: Gd<Node>) {
        if let Ok(node2d) = body.try_cast::<Node2D>()
            && node2d.has_method("take_damage")
        {
            self.targets_in_range.push(node2d);
        }
    }

    #[func]
    fn _on_range_body_exited(&mut self, body: Gd<Node>) {
        if let Ok(node2d) = body.try_cast::<Node2D>() {
            let id = node2d.instance_id();
            self.targets_in_range.retain(|t| t.instance_id() != id);

            let mut lightning = self
                .base()
                .get_node_as::<LightningComponent>("LightningComponent");
            lightning.bind_mut().remove_target(id.to_i64());
        }
    }

    #[func]
    pub fn take_damage(
        &mut self,
        amount: f32,
        #[opt(default = Color::WHITE)] _damage_color: Color,
    ) -> bool {
        if self.invincible_timer > 0.0 || self.is_dead || self.god_mode {
            return false;
        }
        self.lives -= amount.round() as i32;
        self.invincible_timer = INVINCIBILITY_DURATION;
        let lives_left = self.lives.max(0);
        self.signals().damaged().emit(lives_left);
        if self.lives <= 0 {
            self.lives = 0;
            self.is_dead = true;
            self.transition_state(PlayerState::Dead as i32);
            let mut lightning = self
                .base()
                .get_node_as::<LightningComponent>("LightningComponent");
            lightning.bind_mut().clear();
            // Downed look (ghosted body, revive zone) is drawn by `PlayerVisual`.
            self.base_mut().set_modulate(Color::WHITE);
            self.base_mut().emit_signal("died", &[]);
        }
        true
    }

    /// Teammate revive: back with one life.
    #[func]
    pub fn revive(&mut self) {
        self.restore(1);
        self.signals().revived().emit();
    }

    /// Back with full lives after a checkpoint rewind (does not emit `revived`).
    #[func]
    pub fn respawn(&mut self) {
        self.restore(MAX_LIVES);
    }

    fn restore(&mut self, lives: i32) {
        self.lives = lives;
        self.is_dead = false;
        self.revival_progress = 0.0;
        self.invincible_timer = INVINCIBILITY_DURATION;
        self.base_mut().set_modulate(Color::WHITE);
        self.force_state(PlayerState::Idle as i32);
        self.base_mut().queue_redraw();
    }

    /// Downs the player immediately, ignoring invincibility and god mode.
    #[func]
    pub fn kill(&mut self) {
        if self.is_dead {
            return;
        }
        self.invincible_timer = 0.0;
        let god_mode = std::mem::replace(&mut self.god_mode, false);
        let lives = self.lives as f32;
        self.take_damage(lives.max(1.0), Color::WHITE);
        self.god_mode = god_mode;
    }

    fn apply_deadzone(&self, value: f32) -> f32 {
        if value.abs() < self.joystick_deadzone {
            return 0.0;
        }
        value.signum() * (value.abs() - self.joystick_deadzone) / (1.0 - self.joystick_deadzone)
    }

    fn get_tier_radius(&self, tier: i32) -> f32 {
        self.range_radius * ((tier + 1) as f32 / TIER_COUNT as f32)
    }

    fn get_ray_count(&self, target: Gd<Node2D>) -> i32 {
        let dist = self
            .base()
            .get_global_position()
            .distance_to(target.get_global_position());
        for tier in (0..TIER_COUNT).rev() {
            if dist <= self.get_tier_radius(tier) {
                return TIER_COUNT - tier;
            }
        }
        1
    }

    fn update_range_shape(&mut self) {
        let mut range_shape = self
            .base()
            .get_node_as::<CollisionShape2D>("RangeArea/CollisionShape2D");
        let shape = range_shape.get_shape();

        let mut circle_shape = if let Some(s) = shape {
            if let Ok(cs) = s.try_cast::<CircleShape2D>() {
                cs
            } else {
                let new_shape = CircleShape2D::new_gd();
                range_shape.set_shape(&new_shape);
                new_shape
            }
        } else {
            let new_shape = CircleShape2D::new_gd();
            range_shape.set_shape(&new_shape);
            new_shape
        };

        circle_shape.set_radius(self.range_radius);
    }

    fn apply_continuous_damage(&mut self, delta: f64) {
        if self.damage_per_second <= 0.0 {
            let mut lightning = self
                .base()
                .get_node_as::<LightningComponent>("LightningComponent");
            lightning.bind_mut().clear();
            self.transition_state(PlayerState::Idle as i32);
            return;
        }

        let mut active_targets = HashMap::new();
        let mut rejected = Vec::new();
        let mut did_hit_any = false;

        let targets = self.targets_in_range.clone();
        for target in targets {
            if !target.is_instance_valid() {
                continue;
            }

            let ray_count = self.get_ray_count(target.clone());
            let damage_amount = self.damage_per_second * ray_count as f32 * delta as f32;

            let result = target.clone().call(
                "take_damage",
                &[damage_amount.to_variant(), self.team_color.to_variant()],
            );
            if result.try_to::<bool>().unwrap_or(false) {
                did_hit_any = true;
                active_targets.insert(
                    target.instance_id().to_i64(),
                    LightningTarget { target, ray_count },
                );
            } else {
                rejected.push(target);
            }
        }

        if did_hit_any {
            self.base_mut().emit_signal("hit_enemy", &[]);
        }

        let mut lightning = self
            .base()
            .get_node_as::<LightningComponent>("LightningComponent");
        lightning
            .bind_mut()
            .update(delta as f32, &active_targets, &rejected, self.team_color);

        if active_targets.is_empty() {
            self.transition_state(PlayerState::Idle as i32);
        } else {
            self.transition_state(PlayerState::Attacking as i32);
        }
    }
}

#[godot_api]
impl ICharacterBody2D for Player {
    fn ready(&mut self) {
        if self.speed <= 0.0 {
            self.speed = 220.0;
        }
        if self.range_radius <= 0.0 {
            self.range_radius = 140.0;
        }
        if self.damage_per_second <= 0.0 {
            self.damage_per_second = 1.0;
        }
        if self.move_left_action.is_empty() {
            self.move_left_action = "ui_left".into();
        }
        if self.move_right_action.is_empty() {
            self.move_right_action = "ui_right".into();
        }
        if self.move_up_action.is_empty() {
            self.move_up_action = "ui_up".into();
        }
        if self.move_down_action.is_empty() {
            self.move_down_action = "ui_down".into();
        }
        if self.joystick_deadzone <= 0.0 {
            self.joystick_deadzone = 0.2;
        }
        self.lives = MAX_LIVES;

        let mut sm = StateMachine::new_gd();
        let mut transitions = Dictionary::<i32, Variant>::new();
        let _ = transitions.insert(
            PlayerState::Idle as i32,
            &Array::from_iter([PlayerState::Attacking as i32, PlayerState::Dead as i32])
                .to_variant(),
        );
        let _ = transitions.insert(
            PlayerState::Attacking as i32,
            &Array::from_iter([PlayerState::Idle as i32, PlayerState::Dead as i32]).to_variant(),
        );
        let _ = transitions.insert(PlayerState::Dead as i32, &Array::<i32>::new().to_variant());

        sm.bind_mut()
            .init_machine(PlayerState::Idle as i32, transitions);

        self.sm = Some(sm);

        self.base_mut().add_to_group("players");
        self.update_range_shape();

        let sprite = self.base().get_node_as::<Sprite2D>("Sprite2D");
        if let Some(mat) = sprite.get_material()
            && let Ok(mut material) = mat.try_cast::<ShaderMaterial>()
        {
            material.set_shader_parameter("player_color", &self.team_color.to_variant());
        }

        let mut range_area = self.base().get_node_as::<Area2D>("RangeArea");
        range_area.connect(
            "body_entered",
            &self.base().callable("_on_range_body_entered"),
        );
        range_area.connect(
            "body_exited",
            &self.base().callable("_on_range_body_exited"),
        );

        self.base_mut().queue_redraw();
    }

    fn physics_process(&mut self, delta: f64) {
        if self.is_dead {
            return;
        }

        if self.invincible_timer > 0.0 {
            self.invincible_timer -= delta as f32;
            let mut modulate = self.base().get_modulate();
            if (self.invincible_timer * 6.0).fract() > 0.5 {
                modulate.a = 0.3;
            } else {
                modulate.a = 1.0;
            }
            self.base_mut().set_modulate(modulate);
            if self.invincible_timer <= 0.0 {
                modulate.a = 1.0;
                self.base_mut().set_modulate(modulate);
            }
        }

        let input = Input::singleton();

        // Assuming GameConfig values for input_type
        // 0: Keyboard1, 1: Keyboard2, 2-9: GamepadLeft, 10-17: GamepadRight
        let input_dir = if self.input_type >= 10 {
            // GamepadRight
            let dev = self.input_type - 10;
            Vector2::new(
                self.apply_deadzone(input.get_joy_axis(dev, godot::global::JoyAxis::RIGHT_X)),
                self.apply_deadzone(input.get_joy_axis(dev, godot::global::JoyAxis::RIGHT_Y)),
            )
        } else if self.input_type >= 2 {
            // GamepadLeft
            let dev = self.input_type - 2;
            Vector2::new(
                self.apply_deadzone(input.get_joy_axis(dev, godot::global::JoyAxis::LEFT_X)),
                self.apply_deadzone(input.get_joy_axis(dev, godot::global::JoyAxis::LEFT_Y)),
            )
        } else {
            input.get_vector(
                &self.move_left_action,
                &self.move_right_action,
                &self.move_up_action,
                &self.move_down_action,
            )
        };

        let velocity = input_dir * self.speed;
        self.base_mut().set_velocity(velocity);
        self.base_mut().move_and_slide();

        // Clamp to viewport
        let viewport = self.base().get_viewport();
        let rect = viewport.expect("Viewport not found").get_visible_rect();
        let mut pos = self.base().get_global_position();
        pos.x = pos.x.clamp(rect.position.x, rect.end().x);
        pos.y = pos.y.clamp(rect.position.y, rect.end().y);
        self.base_mut().set_global_position(pos);

        self.apply_continuous_damage(delta);
    }
}

/// Draws the player's lightning: jittered, re-rolled polylines from the player to each
/// target it damages (one per ray), a glow and sparks at the hit point, and a short
/// flickering stub with deflect sparks when a ray meets a shield of another color.
#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct LightningComponent {
    bolts: HashMap<i64, Bolt>,
    deflects: Vec<Deflect>,
    color: Color,
    jitter_timer: f32,
    spark_timer: f32,
    deflect_sfx_timer: f32,
    time: f32,
    base: Base<Node2D>,
}

struct LightningTarget {
    target: Gd<Node2D>,
    ray_count: i32,
}

struct Bolt {
    target: Gd<Node2D>,
    ray_count: i32,
    /// One jittered polyline per ray, in local coordinates.
    paths: Vec<PackedVector2Array>,
}

struct Deflect {
    /// Shield contact point, local.
    contact: Vector2,
    shield_color: Color,
    path: PackedVector2Array,
}

/// Bolt shapes re-roll this often (seconds): electric flicker without per-frame noise.
const JITTER_INTERVAL: f32 = 1.0 / 30.0;
const SPARK_INTERVAL: f32 = 0.14;
const DEFLECT_SFX_INTERVAL: f32 = 0.3;
const SEGMENT_LENGTH: f32 = 16.0;
const JITTER_AMPLITUDE: f32 = 7.0;
const RAY_SPACING: f32 = 8.0;

/// Random jittered path from `from` to `to`; the bulge is largest mid-bolt.
fn bolt_path(from: Vector2, to: Vector2, amplitude: f32) -> PackedVector2Array {
    let delta = to - from;
    let length = delta.length();
    let segments = ((length / SEGMENT_LENGTH) as i32).clamp(3, 24);
    let normal = if length > 0.0 {
        Vector2::new(-delta.y, delta.x) / length
    } else {
        Vector2::ZERO
    };
    let mut points = PackedVector2Array::new();
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let envelope = (t * std::f32::consts::PI).sin();
        let offset = if i == 0 || i == segments {
            0.0
        } else {
            randf_range(-1.0, 1.0) as f32 * amplitude * envelope
        };
        points.push(from + delta * t + normal * offset);
    }
    points
}

impl LightningComponent {
    fn update(
        &mut self,
        delta: f32,
        active_targets: &HashMap<i64, LightningTarget>,
        rejected: &[Gd<Node2D>],
        team_color: Color,
    ) {
        self.color = team_color;
        self.time += delta;
        self.bolts.retain(|id, _| active_targets.contains_key(id));
        for (&id, info) in active_targets {
            let bolt = self.bolts.entry(id).or_insert_with(|| Bolt {
                target: info.target.clone(),
                ray_count: 0,
                paths: Vec::new(),
            });
            bolt.target = info.target.clone();
            bolt.ray_count = info.ray_count;
        }

        self.jitter_timer -= delta;
        if self.jitter_timer <= 0.0 {
            self.jitter_timer = JITTER_INTERVAL;
            self.reroll_bolts();
            self.collect_deflects(rejected);
        }

        self.spark_timer -= delta;
        self.deflect_sfx_timer -= delta;
        if self.spark_timer <= 0.0 && (!self.bolts.is_empty() || !self.deflects.is_empty()) {
            self.spark_timer = SPARK_INTERVAL;
            self.emit_sparks();
        }
        self.base_mut().queue_redraw();
    }

    fn clear(&mut self) {
        self.bolts.clear();
        self.deflects.clear();
        self.base_mut().queue_redraw();
    }

    fn remove_target(&mut self, target_id: i64) {
        self.bolts.remove(&target_id);
    }

    fn reroll_bolts(&mut self) {
        let base = self.base().clone();
        for bolt in self.bolts.values_mut() {
            if !bolt.target.is_instance_valid() {
                bolt.paths.clear();
                continue;
            }
            let target = base.to_local(bolt.target.get_global_position());
            let perp = target.normalized_or_zero().orthogonal();
            let rays = bolt.ray_count.max(1);
            bolt.paths = (0..rays)
                .map(|i| {
                    let offset = perp * (i as f32 - (rays - 1) as f32 / 2.0) * RAY_SPACING;
                    bolt_path(offset * 0.4, target + offset * 0.3, JITTER_AMPLITUDE)
                })
                .collect();
        }
    }

    /// Rejected targets whose active shield has another color get a deflect stub.
    fn collect_deflects(&mut self, rejected: &[Gd<Node2D>]) {
        self.deflects.clear();
        for target in rejected {
            if !target.is_instance_valid() {
                continue;
            }
            let Some(mut health) = target.get_node_or_null("HealthComponent") else {
                continue;
            };
            let layer = health.call("get_active_layer", &[]).to::<i32>();
            if layer < 0 {
                continue;
            }
            let shield_color = health.call("get_active_color", &[]).to::<Color>();
            let radius = health
                .call("get_layer_radius", &[layer.to_variant()])
                .try_to::<f32>()
                .unwrap_or(34.0);
            let center = self.base().to_local(target.get_global_position());
            if center.length() <= radius {
                continue;
            }
            let contact = center - center.normalized_or_zero() * radius;
            let path = bolt_path(Vector2::ZERO, contact, JITTER_AMPLITUDE * 0.8);
            self.deflects.push(Deflect {
                contact,
                shield_color,
                path,
            });
        }
    }

    fn emit_sparks(&mut self) {
        let base = self.base().clone();
        let color = self.color;
        let hits: Vec<Vector2> = self
            .bolts
            .values()
            .filter(|b| b.target.is_instance_valid())
            .map(|b| b.target.get_global_position())
            .collect();
        let deflects: Vec<(Vector2, Color)> = self
            .deflects
            .iter()
            .map(|d| (base.to_global(d.contact), d.shield_color))
            .collect();
        let play_deflect = !deflects.is_empty() && self.deflect_sfx_timer <= 0.0;
        if play_deflect {
            self.deflect_sfx_timer = DEFLECT_SFX_INTERVAL;
        }
        with_fx(|fx| {
            for pos in hits {
                fx.burst_style(pos, color, 4, BurstStyle::Sparks as i32, 0.7);
            }
            for (pos, shield_color) in &deflects {
                fx.burst_style(*pos, Color::WHITE, 5, BurstStyle::Sparks as i32, 1.1);
                fx.burst_style(*pos, *shield_color, 3, BurstStyle::Dots as i32, 0.8);
            }
            if play_deflect && let Some((pos, _)) = deflects.first() {
                fx.play_sfx("shield_deflect".into(), *pos, 0.12, 0.0);
            }
        });
    }

    fn draw_bolt(&mut self, path: &PackedVector2Array, color: Color, strength: f32) {
        let glow = Color::from_rgba(color.r, color.g, color.b, 0.22 * strength);
        let core = color.lightened(0.45);
        let core = Color::from_rgba(core.r, core.g, core.b, 0.95 * strength);
        let hot = Color::from_rgba(1.0, 1.0, 1.0, 0.85 * strength);
        self.base_mut()
            .draw_polyline_ex(path, glow)
            .width(12.0)
            .done();
        self.base_mut()
            .draw_polyline_ex(path, core)
            .width(4.0)
            .antialiased(true)
            .done();
        self.base_mut()
            .draw_polyline_ex(path, hot)
            .width(1.0)
            .done();
    }
}

#[godot_api]
impl INode2D for LightningComponent {
    fn ready(&mut self) {
        // Under the player sprites, over the arena.
        self.base_mut().set_z_index(-1);
    }

    fn draw(&mut self) {
        let color = self.color;
        let flicker = 0.8 + 0.2 * (self.time * 53.0).sin();
        let bolts: Vec<(Vec<PackedVector2Array>, Option<Vector2>)> = self
            .bolts
            .values()
            .map(|b| {
                let end = b.paths.first().and_then(|p| p.as_slice().last().copied());
                (b.paths.clone(), end)
            })
            .collect();
        for (paths, end) in bolts {
            for path in &paths {
                self.draw_bolt(path, color, flicker);
            }
            if let Some(end) = end {
                let r = 7.0 + 3.0 * (self.time * 41.0).sin().abs();
                self.base_mut().draw_circle(
                    end,
                    r * 1.8,
                    Color::from_rgba(color.r, color.g, color.b, 0.25),
                );
                self.base_mut()
                    .draw_circle(end, r * 0.6, Color::from_rgba(1.0, 1.0, 1.0, 0.9));
            }
        }
        let deflects: Vec<(PackedVector2Array, Vector2, Color)> = self
            .deflects
            .iter()
            .map(|d| (d.path.clone(), d.contact, d.shield_color))
            .collect();
        for (path, contact, shield_color) in deflects {
            // A weak, stuttering stub that never reaches the body.
            if (self.time * 24.0).sin() > -0.3 {
                self.draw_bolt(&path, color, 0.45);
            }
            let r = 9.0 + 4.0 * (self.time * 37.0).sin().abs();
            self.base_mut()
                .draw_arc_ex(
                    contact,
                    r,
                    0.0,
                    std::f32::consts::TAU,
                    16,
                    Color::from_rgba(shield_color.r, shield_color.g, shield_color.b, 0.8),
                )
                .width(2.0)
                .done();
        }
    }
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct RevivalComponent {
    timer: f32,
    base: Base<Node>,
}

const REVIVAL_DISTANCE: f32 = 60.0;
const REVIVAL_TIME: f32 = 2.0;

#[godot_api]
impl RevivalComponent {
    fn reset(&mut self) {
        if self.timer > 0.0 {
            self.timer = 0.0;
            if let Some(mut player) = self
                .base()
                .get_owner()
                .and_then(|o| o.try_cast::<Player>().ok())
            {
                player.bind_mut().revival_progress = 0.0;
                player.queue_redraw();
            }
        }
    }

    fn find_nearest_alive_player(&self, origin: Vector2) -> Option<Gd<Player>> {
        let mut nearest: Option<Gd<Player>> = None;
        let mut nearest_dist = REVIVAL_DISTANCE + 1.0;
        let owner = self.base().get_owner();

        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group("players").iter_shared() {
            if Some(node.clone()) == owner {
                continue;
            }
            if let Ok(p) = node.try_cast::<Player>() {
                if p.bind().is_dead {
                    continue;
                }
                let d = p.get_global_position().distance_to(origin);
                if d <= REVIVAL_DISTANCE && d < nearest_dist {
                    nearest_dist = d;
                    nearest = Some(p);
                }
            }
        }
        nearest
    }
}

#[godot_api]
impl INode for RevivalComponent {
    fn process(&mut self, delta: f64) {
        let mut player = self
            .base()
            .get_owner()
            .and_then(|o| o.try_cast::<Player>().ok())
            .unwrap();

        if !player.bind().is_dead {
            self.reset();
            return;
        }

        let reviver = self.find_nearest_alive_player(player.get_global_position());
        if reviver.is_some() {
            self.timer += delta as f32;
            {
                let mut p_bind = player.bind_mut();
                p_bind.revival_progress = self.timer / REVIVAL_TIME;
            }
            player.queue_redraw();
            if self.timer >= REVIVAL_TIME {
                self.reset();
                player.bind_mut().revive();
            }
        } else {
            self.reset();
        }
    }
}
