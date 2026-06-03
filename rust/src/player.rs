use crate::state_machine::StateMachine;
use godot::classes::{
    Area2D, CharacterBody2D, CircleShape2D, CollisionShape2D, ICharacterBody2D, INode, Input,
    Line2D, Node, Node2D, ResourceLoader, ShaderMaterial, Sprite2D, Texture2D,
};
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
        if self.invincible_timer > 0.0 || self.is_dead {
            return false;
        }
        self.lives -= amount.round() as i32;
        self.invincible_timer = INVINCIBILITY_DURATION;
        if self.lives <= 0 {
            self.lives = 0;
            self.is_dead = true;
            self.transition_state(PlayerState::Dead as i32);
            let mut lightning = self
                .base()
                .get_node_as::<LightningComponent>("LightningComponent");
            lightning.bind_mut().clear();
            self.base_mut().set_modulate(Color::from_rgb(0.4, 0.4, 0.4));
            self.base_mut().queue_redraw();
            self.base_mut().emit_signal("died", &[]);
        }
        true
    }

    #[func]
    pub fn revive(&mut self) {
        self.lives = 1;
        self.is_dead = false;
        self.revival_progress = 0.0;
        self.invincible_timer = INVINCIBILITY_DURATION;
        self.base_mut().set_modulate(Color::WHITE);
        self.force_state(PlayerState::Idle as i32);
        self.base_mut().queue_redraw();
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
            .update(delta as f32, &active_targets, self.team_color);

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

    fn draw(&mut self) {
        if !self.is_dead {
            let alpha = 0.12 / TIER_COUNT as f32;
            let mut ring_color = self.team_color;
            ring_color.a = alpha;
            for tier in (0..TIER_COUNT).rev() {
                let radius = self.get_tier_radius(tier);
                self.base_mut()
                    .draw_circle(Vector2::ZERO, radius, ring_color);
            }
        }
        if self.is_dead && self.revival_progress > 0.0 {
            let tau = std::f32::consts::TAU;
            let progress = self.revival_progress;
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    18.0,
                    -std::f32::consts::FRAC_PI_2,
                    -std::f32::consts::FRAC_PI_2 + tau * progress,
                    32,
                    Color::WHITE,
                )
                .width(3.0)
                .done();
        }
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

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct LightningComponent {
    textures: Vec<Gd<Texture2D>>,
    lines: HashMap<i64, Vec<Gd<Line2D>>>,
    frame_timer: f32,
    frame_index: usize,
    base: Base<Node>,
}

struct LightningTarget {
    target: Gd<Node2D>,
    ray_count: i32,
}

const LIGHTNING_FPS: f32 = 12.0;
const LIGHTNING_WIDTH: f32 = 80.0;
const RAY_SPACING: f32 = 10.0;

#[godot_api]
impl LightningComponent {
    fn update(
        &mut self,
        delta: f32,
        active_targets: &HashMap<i64, LightningTarget>,
        team_color: Color,
    ) {
        self.cycle_frame(delta);
        self.sync_lines(active_targets, team_color);
    }

    fn clear(&mut self) {
        for lines in self.lines.values_mut() {
            for line in lines {
                line.queue_free();
            }
        }
        self.lines.clear();
    }

    fn remove_target(&mut self, target_id: i64) {
        if let Some(lines) = self.lines.remove(&target_id) {
            for mut line in lines {
                line.queue_free();
            }
        }
    }

    fn cycle_frame(&mut self, delta: f32) {
        if self.textures.is_empty() {
            return;
        }
        self.frame_timer += delta;
        if self.frame_timer >= 1.0 / LIGHTNING_FPS {
            self.frame_timer -= 1.0 / LIGHTNING_FPS;
            self.frame_index = (self.frame_index + 1) % self.textures.len();
            let tex = self.textures[self.frame_index].clone();
            for lines in self.lines.values_mut() {
                for line in lines {
                    line.set_texture(&tex);
                }
            }
        }
    }

    fn sync_lines(&mut self, active_targets: &HashMap<i64, LightningTarget>, team_color: Color) {
        let Some(mut parent) = self
            .base()
            .get_parent()
            .and_then(|p| p.try_cast::<Node2D>().ok())
        else {
            return;
        };

        // Remove lines for targets no longer active
        let to_remove: Vec<_> = self
            .lines
            .keys()
            .copied()
            .filter(|id| !active_targets.contains_key(id))
            .collect();
        for id in to_remove {
            self.remove_target(id);
        }

        // Update or create lines for active targets
        for (&target_id, info) in active_targets {
            let lines = self.lines.entry(target_id).or_default();

            while lines.len() < info.ray_count as usize {
                let mut line = Line2D::new_alloc();
                if !self.textures.is_empty() {
                    line.set_texture(&self.textures[self.frame_index]);
                }
                line.set_texture_mode(godot::classes::line_2d::LineTextureMode::TILE);
                line.set_width(LIGHTNING_WIDTH);
                let mut line_color = team_color;
                line_color.a = 0.8;
                line.set_default_color(line_color);
                line.set_z_index(-1);
                parent.add_child(&line);
                lines.push(line);
            }
            while lines.len() > info.ray_count as usize {
                let mut extra = lines.pop().unwrap();
                extra.queue_free();
            }

            let target_local = parent.to_local(info.target.get_global_position());
            let perp = target_local
                .normalized()
                .rotated(std::f32::consts::FRAC_PI_2);
            for (i, line) in lines.iter_mut().enumerate() {
                let offset = perp * (i as f32 - (info.ray_count - 1) as f32 / 2.0) * RAY_SPACING;
                line.clear_points();
                line.add_point(target_local + offset);
                line.add_point(Vector2::ZERO + offset);
            }
        }
    }
}

#[godot_api]
impl INode for LightningComponent {
    fn ready(&mut self) {
        let mut loader = ResourceLoader::singleton();
        let tex1 = loader
            .load("res://assets/kenney-particles/Rotated/spark_05_rotated.png")
            .and_then(|r| r.try_cast::<Texture2D>().ok());
        let tex2 = loader
            .load("res://assets/kenney-particles/Rotated/spark_06_rotated.png")
            .and_then(|r| r.try_cast::<Texture2D>().ok());
        if let Some(t) = tex1 {
            self.textures.push(t);
        }
        if let Some(t) = tex2 {
            self.textures.push(t);
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
