//! `EnemyVisual`: presentation for an enemy scene (child of the `BaseEnemy` root).
//!
//! Works with any component mix, reading siblings by name: the `Sprite2D` pivot
//! (rotated by `TurnComponent` or here), `HealthComponent`, and any shooter
//! component's `fired(projectile)` signal. It owns the spawn pop, the damage flash, the
//! aim snap with recoil and muzzle flash, the shield-layer break animation and the
//! death explosion. It never changes gameplay state.

use crate::core::feel::decay;
use crate::fx::{BurstStyle, with_fx};
use godot::classes::tween::{EaseType, TransitionType};
use godot::classes::{INode2D, Node, Node2D, ShaderMaterial, Sprite2D};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;
/// Enemy family glow (amber): enemy cores, muzzle flashes, projectiles and mines.
pub const ENEMY_GLOW: Color = Color::from_rgb(1.0, 0.55, 0.15);
/// Enemy body metal, used for death shards.
pub const ENEMY_METAL: Color = Color::from_rgb(0.82, 0.85, 0.95);
const MUZZLE_SECONDS: f32 = 0.09;
const BREAK_SECONDS: f32 = 0.45;
const BREAK_SEGMENTS: i32 = 10;
const HIT_SFX_INTERVAL: f32 = 0.2;
const SHOOTERS: [&str; 3] = [
    "ShooterComponent",
    "ShotgunShooterComponent",
    "TurretShooterComponent",
];

struct Muzzle {
    direction: Vector2,
    age: f32,
}

struct ShieldBreak {
    color: Color,
    radius: f32,
    age: f32,
    spin: f32,
}

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct EnemyVisual {
    /// Snap the pivot toward each shot's direction; the body repeats every
    /// `TAU / aim_symmetry` (1 = one muzzle, 4 = four-way turret). 0 disables.
    #[export]
    pub aim_symmetry: i32,
    /// Rotate the pivot toward the direction of travel (enemies without a
    /// `TurnComponent`).
    #[export]
    pub face_motion: bool,
    /// Body radius in pixels: glow size and muzzle distance.
    #[export]
    #[init(val = 24.0)]
    pub body_radius: f32,

    enemy: Option<Gd<Node2D>>,
    pivot: Option<Gd<Sprite2D>>,
    body_material: Option<Gd<ShaderMaterial>>,
    health: Option<Gd<Node>>,

    flash: f32,
    hit_sfx_timer: f32,
    recoil: Vector2,
    aim_angle: Option<f32>,
    last_position: Vector2,
    #[init(val = -2)]
    active_layer: i32,
    muzzles: Vec<Muzzle>,
    breaks: Vec<ShieldBreak>,
    time: f32,

    base: Base<Node2D>,
}

#[godot_api]
impl EnemyVisual {
    #[func]
    fn _on_damaged(&mut self, _amount: f32) {
        self.flash = 1.0;
        if self.hit_sfx_timer <= 0.0 {
            self.hit_sfx_timer = HIT_SFX_INTERVAL;
            let pos = self.base().get_global_position();
            with_fx(|fx| {
                fx.play_sfx("enemy_hit".into(), pos, 0.15, 0.0);
            });
        }
    }

    #[func]
    fn _on_fired(&mut self, projectile: Gd<Node>) {
        let direction = projectile
            .get("direction")
            .try_to::<Vector2>()
            .unwrap_or(Vector2::RIGHT)
            .normalized_or_zero();
        self.recoil = -direction * 7.0;
        self.muzzles.push(Muzzle {
            direction,
            age: 0.0,
        });
        if self.aim_symmetry > 0 {
            self.aim_angle = Some(direction.angle());
        }
        let muzzle = self.base().get_global_position() + direction * self.body_radius;
        with_fx(|fx| fx.burst_style(muzzle, ENEMY_GLOW, 3, BurstStyle::Sparks as i32, 0.6));
    }

    #[func]
    fn _on_died(&mut self) {
        let pos = self.base().get_global_position();
        with_fx(|fx| {
            fx.burst_style(pos, ENEMY_METAL, 22, BurstStyle::Shards as i32, 1.3);
            fx.burst_style(pos, ENEMY_GLOW, 18, BurstStyle::Sparks as i32, 1.5);
            fx.burst_style(pos, ENEMY_GLOW, 10, BurstStyle::Dots as i32, 1.2);
            fx.ring(pos, ENEMY_GLOW, 95.0, 0.45);
            fx.ring(pos, Color::WHITE, 55.0, 0.25);
            fx.shake(0.4);
            fx.hitstop(0.05);
            fx.play_sfx("enemy_die".into(), pos, 0.08, 0.0);
            fx.play_sfx("explosion_small".into(), pos, 0.1, 0.0);
        });
    }

    #[func]
    pub fn get_flash(&self) -> f32 {
        self.flash
    }
}

impl EnemyVisual {
    fn connect_signals(&mut self, enemy: &Gd<Node2D>) {
        let this = self.to_gd();
        let mut enemy = enemy.clone();
        if enemy.has_signal("died") {
            enemy.connect("died", &this.callable("_on_died"));
        }
        if let Some(mut health) = self.health.clone() {
            health.connect("damaged", &this.callable("_on_damaged"));
        }
        for name in SHOOTERS {
            if let Some(mut shooter) = enemy.get_node_or_null(name) {
                shooter.connect("fired", &this.callable("_on_fired"));
            }
        }
    }

    fn spawn_pop(&mut self) {
        let pos = self.base().get_global_position();
        let on_screen = self
            .base()
            .get_viewport()
            .is_some_and(|v| v.get_visible_rect().contains_point(pos));
        let mut tween = self.base_mut().create_tween();
        tween.set_parallel();
        if let Some(mut pivot) = self.pivot.clone() {
            pivot.set_scale(Vector2::new(0.05, 0.05));
            tween
                .tween_property(&pivot, "scale", &Vector2::ONE.to_variant(), 0.6)
                .set_trans(TransitionType::ELASTIC)
                .set_ease(EaseType::OUT);
        }
        if let Some(health) = self.health.clone()
            && let Ok(mut health) = health.try_cast::<Node2D>()
        {
            health.set_scale(Vector2::new(0.3, 0.3));
            tween
                .tween_property(&health, "scale", &Vector2::ONE.to_variant(), 0.4)
                .set_trans(TransitionType::BACK)
                .set_ease(EaseType::OUT)
                .set_delay(0.08);
        }
        if on_screen {
            with_fx(|fx| {
                fx.burst_style(pos, ENEMY_GLOW, 10, BurstStyle::Dots as i32, 1.0);
                fx.ring(pos, ENEMY_GLOW, 70.0, 0.35);
                fx.play_sfx("enemy_spawn".into(), pos, 0.08, 0.0);
            });
        }
    }

    fn watch_shields(&mut self) {
        let Some(mut health) = self.health.clone() else {
            return;
        };
        let layer = health.call("get_active_layer", &[]).to::<i32>();
        if self.active_layer == -2 {
            self.active_layer = layer;
            return;
        }
        if layer == self.active_layer {
            return;
        }
        let broken = self.active_layer;
        self.active_layer = layer;
        if broken < 0 {
            return;
        }
        let colors = health.get("shield_colors").to::<PackedColorArray>();
        let color = colors.get(broken as usize).unwrap_or(Color::WHITE);
        let radius = health
            .call("get_layer_radius", &[broken.to_variant()])
            .try_to::<f32>()
            .unwrap_or(34.0);
        self.breaks.push(ShieldBreak {
            color,
            radius,
            age: 0.0,
            spin: self.time,
        });
        let pos = self.base().get_global_position();
        let exposed = layer < 0;
        with_fx(|fx| {
            fx.burst_style(pos, color, 16, BurstStyle::Shards as i32, 1.0);
            fx.burst_style(pos, Color::WHITE, 8, BurstStyle::Sparks as i32, 1.2);
            fx.ring(pos, color, radius + 36.0, 0.4);
            fx.shake(if exposed { 0.3 } else { 0.18 });
            fx.play_sfx("shield_break".into(), pos, 0.06, 0.0);
        });
    }

    fn animate(&mut self, dt: f32) {
        self.flash = decay(self.flash, dt, 0.05);
        self.hit_sfx_timer -= dt;
        self.recoil *= 0.5f32.powf(dt / 0.05);
        for muzzle in self.muzzles.iter_mut() {
            muzzle.age += dt;
        }
        self.muzzles.retain(|m| m.age < MUZZLE_SECONDS);
        for b in self.breaks.iter_mut() {
            b.age += dt;
        }
        self.breaks.retain(|b| b.age < BREAK_SECONDS);

        let position = self.enemy.as_ref().map(|e| e.get_global_position());
        let velocity = match position {
            Some(p) if dt > 0.0 => (p - self.last_position) / dt,
            _ => Vector2::ZERO,
        };
        if let Some(p) = position {
            self.last_position = p;
        }

        if let Some(mut pivot) = self.pivot.clone() {
            pivot.set_position(self.recoil);
            let current = pivot.get_rotation();
            let target = if let Some(aim) = self.aim_angle {
                let step = TAU / self.aim_symmetry.max(1) as f32;
                // Nearest equivalent orientation for symmetric bodies.
                let diff = (aim - current).rem_euclid(step);
                Some(current + if diff > step / 2.0 { diff - step } else { diff })
            } else if self.face_motion && velocity.length() > 4.0 {
                Some(velocity.angle())
            } else {
                None
            };
            if let Some(target) = target {
                let diff = (target - current + std::f32::consts::PI).rem_euclid(TAU)
                    - std::f32::consts::PI;
                pivot.set_rotation(current + diff * (1.0 - (-dt * 14.0).exp()));
            }
        }
        if let Some(material) = self.body_material.as_mut() {
            material.set_shader_parameter("flash", &(self.flash * 0.85).to_variant());
        }
    }
}

#[godot_api]
impl INode2D for EnemyVisual {
    fn ready(&mut self) {
        // Glow, muzzle flash and break plates draw under the body sprite.
        self.base_mut().set_z_index(-1);
        let Some(enemy) = self
            .base()
            .get_parent()
            .and_then(|p| p.try_cast::<Node2D>().ok())
        else {
            return;
        };
        self.pivot = enemy.try_get_node_as::<Sprite2D>("Sprite2D");
        self.body_material = self
            .pivot
            .as_ref()
            .and_then(|p| p.try_get_node_as::<Sprite2D>("Body"))
            .and_then(|body| body.get_material())
            .and_then(|m| m.try_cast::<ShaderMaterial>().ok());
        self.health = enemy.get_node_or_null("HealthComponent");
        self.last_position = enemy.get_global_position();
        self.connect_signals(&enemy);
        self.enemy = Some(enemy);
        self.spawn_pop();
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        self.time += dt;
        self.watch_shields();
        self.animate(dt);
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let r = self.body_radius;
        let pulse = 0.5 + 0.5 * (self.time * 3.0).sin();
        // Dark core behind the metal body plus a thin amber ring: the enemy family
        // marker, kept clear of the player-colored shield rings outside it.
        let glow = ENEMY_GLOW;
        let center = self.recoil;
        let flash = self.flash;
        self.base_mut()
            .draw_circle(center, r * 0.95, Color::from_rgba(0.03, 0.03, 0.07, 0.85));
        self.base_mut()
            .draw_arc_ex(
                center,
                r * 0.95,
                0.0,
                TAU,
                32,
                Color::from_rgba(glow.r, glow.g, glow.b, 0.10 + 0.08 * pulse + 0.3 * flash),
            )
            .width(7.0)
            .done();
        self.base_mut()
            .draw_arc_ex(
                center,
                r * 0.95,
                0.0,
                TAU,
                32,
                Color::from_rgba(glow.r, glow.g, glow.b, 0.65 + 0.25 * pulse),
            )
            .width(1.5)
            .antialiased(true)
            .done();

        let muzzles: Vec<(Vector2, f32)> = self
            .muzzles
            .iter()
            .map(|m| (m.direction, 1.0 - m.age / MUZZLE_SECONDS))
            .collect();
        for (direction, life) in muzzles {
            let tip = direction * (r + 6.0);
            self.base_mut().draw_circle(
                tip,
                9.0 * life + 3.0,
                Color::from_rgba(glow.r, glow.g, glow.b, 0.7 * life),
            );
            self.base_mut().draw_circle(
                tip,
                4.0 * life + 1.0,
                Color::from_rgba(1.0, 1.0, 0.9, life),
            );
        }

        let breaks: Vec<(Color, f32, f32, f32)> = self
            .breaks
            .iter()
            .map(|b| (b.color, b.radius, b.age / BREAK_SECONDS, b.spin))
            .collect();
        for (color, radius, t, spin) in breaks {
            let ease = 1.0 - (1.0 - t).powi(2);
            let seg = TAU / BREAK_SEGMENTS as f32;
            for i in 0..BREAK_SEGMENTS {
                // Each plate flies out along its own normal and tumbles a little.
                let mid = spin * 0.5 + seg * (i as f32 + 0.5);
                let out = Vector2::from_angle(mid) * (ease * 46.0);
                let twist = ease * if i % 2 == 0 { 0.5 } else { -0.5 };
                let half = seg * 0.38 * (1.0 - 0.5 * t);
                let c = Color::from_rgba(color.r, color.g, color.b, 1.0 - t);
                self.base_mut()
                    .draw_arc_ex(out, radius, mid - half + twist, mid + half + twist, 6, c)
                    .width(5.0 * (1.0 - 0.6 * t))
                    .done();
            }
        }
    }
}
