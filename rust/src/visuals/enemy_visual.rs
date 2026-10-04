//! `EnemyVisual`: presentation for an enemy scene (child of the `BaseEnemy` root).
//!
//! Works with any component mix: the `Sprite2D` pivot (rotated by `TurnComponent` or
//! here), `HealthComponent`, and whatever sibling components offer: `fired(projectile)`
//! (muzzle flash, recoil), `acted(...)` (an action pop on the beat), `get_windup()`
//! (anticipation: the body coils and an amber ring closes in toward the action beat),
//! `get_pose()` (hop lift and squash/stretch) and `get_aim()` (turn toward an attack
//! before it fires). It owns the spawn pop, the damage flash, the shield-layer break
//! animation and the death explosion. It never changes
//! gameplay state.

use crate::core::feel::decay;
use crate::enemies::{EnemyClock, LEAVE_WARNING_BEATS};
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
/// Pivot turn rate toward its target heading (1/s); lower lags more.
const TURN_RATE: f32 = 8.0;
/// Seconds for the action pop to fall to half.
const ACT_POP_HALF_LIFE: f32 = 0.06;
const SPAWN_POP_SECONDS: f32 = 0.6;

/// Elastic scale-in from 0.05 to 1 over `SPAWN_POP_SECONDS`.
fn spawn_pop(age: f32) -> f32 {
    let t = (age / SPAWN_POP_SECONDS).clamp(0.0, 1.0);
    if t >= 1.0 {
        return 1.0;
    }
    let elastic = 1.0 + 2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * TAU / 3.0).sin();
    0.05 + 0.95 * elastic
}

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

    /// Seconds since the spawn pop started (the pivot scales in elastically).
    #[init(val = SPAWN_POP_SECONDS)]
    spawn_age: f32,

    enemy: Option<Gd<Node2D>>,
    pivot: Option<Gd<Sprite2D>>,
    /// Sibling components that report wind-up, pose or aim.
    parts: Vec<Gd<Node>>,
    clock: EnemyClock,
    beat: f64,
    windup: f32,
    /// Radius of the outermost shield ring (the wind-up ring closes onto it).
    outer_radius: f32,
    /// Landing markers `(global position, radius)` reported by movers.
    markers: Vec<(Vector2, f32)>,
    lift: f32,
    squash: f32,
    act_pop: f32,
    body_material: Option<Gd<ShaderMaterial>>,
    health: Option<Gd<Node>>,

    flash: f32,
    hit_sfx_timer: f32,
    recoil: Vector2,
    aim_angle: Option<f32>,
    last_position: Vector2,
    #[init(val = -2)]
    active_layer: i32,
    layer_count: i32,
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
    fn _on_acted(&mut self, _action_beat: f64, _song_beat: f64) {
        self.act_pop = 1.0;
    }

    #[func]
    pub fn get_flash(&self) -> f32 {
        self.flash
    }

    /// Largest wind-up progress among the components (0..1).
    #[func]
    pub fn get_windup(&self) -> f32 {
        self.windup
    }
}

impl EnemyVisual {
    /// Blinks the whole enemy (body and shields) in the beats before it leaves.
    fn blink_before_leaving(&mut self) {
        let Some(mut enemy) = self.enemy.clone() else {
            return;
        };
        if !enemy.has_method("get_beats_left") {
            return;
        }
        let left = enemy
            .call("get_beats_left", &[])
            .try_to::<f64>()
            .unwrap_or(-1.0);
        let alpha = if (0.0..LEAVE_WARNING_BEATS).contains(&left) {
            0.35 + 0.65 * (left * std::f64::consts::PI * 2.0).cos().abs() as f32
        } else {
            1.0
        };
        let mut modulate = enemy.get_modulate();
        if modulate.a != alpha {
            modulate.a = alpha;
            enemy.set_modulate(modulate);
        }
    }

    fn connect_signals(&mut self, enemy: &Gd<Node2D>) {
        let this = self.to_gd();
        let mut enemy = enemy.clone();
        if enemy.has_signal("died") {
            enemy.connect("died", &this.callable("_on_died"));
        }
        if let Some(mut health) = self.health.clone() {
            health.connect("damaged", &this.callable("_on_damaged"));
        }
        let me = this.instance_id();
        for mut child in enemy.get_children().iter_shared() {
            if child.instance_id() == me {
                continue;
            }
            if child.has_signal("fired") {
                child.connect("fired", &this.callable("_on_fired"));
            }
            if child.has_signal("acted") {
                child.connect("acted", &this.callable("_on_acted"));
            }
            if ["get_windup", "get_pose", "get_aim", "get_marker"]
                .iter()
                .any(|m| child.has_method(*m))
            {
                self.parts.push(child);
            }
        }
    }

    /// Reads wind-up, pose and aim from the components.
    fn read_parts(&mut self) -> Option<f32> {
        let mut windup = 0.0f32;
        let mut lift = 0.0f32;
        let mut squash = 1.0f32;
        let mut aim = None;
        self.markers.clear();
        for part in self.parts.iter_mut() {
            if !part.is_instance_valid() {
                continue;
            }
            if part.has_method("get_marker")
                && let Ok(marker) = part.call("get_marker", &[]).try_to::<Vector3>()
                && marker.z > 0.0
            {
                self.markers
                    .push((Vector2::new(marker.x, marker.y), marker.z));
            }
            if part.has_method("get_windup") {
                windup = windup.max(part.call("get_windup", &[]).try_to::<f32>().unwrap_or(0.0));
            }
            if part.has_method("get_pose")
                && let Ok(pose) = part.call("get_pose", &[]).try_to::<Vector2>()
            {
                lift += pose.x;
                squash *= pose.y;
            }
            if aim.is_none()
                && part.has_method("get_aim")
                && let Ok(direction) = part.call("get_aim", &[]).try_to::<Vector2>()
                && direction.length_squared() > 0.01
            {
                aim = Some(direction.angle());
            }
        }
        self.windup = windup;
        self.lift = lift;
        self.squash = squash.clamp(0.4, 2.0);
        aim
    }

    fn spawn_pop(&mut self) {
        let pos = self.base().get_global_position();
        let on_screen = self
            .base()
            .get_viewport()
            .is_some_and(|v| v.get_visible_rect().contains_point(pos));
        let mut tween = self.base_mut().create_tween();
        tween.set_parallel();
        self.spawn_age = 0.0;
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
        self.outer_radius = if layer >= 0 {
            health
                .call("get_layer_radius", &[0.to_variant()])
                .try_to::<f32>()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let count = health
            .call("get_layer_count", &[])
            .try_to::<i32>()
            .unwrap_or(0);
        if self.active_layer == -2 || count != self.layer_count {
            // First look, or a ward came or went: resync without a break.
            self.active_layer = layer;
            self.layer_count = count;
            return;
        }
        if layer == self.active_layer {
            return;
        }
        let broken = self.active_layer;
        self.active_layer = layer;
        // Only an outer layer emptying (the active index moving inward) is a break.
        if broken < 0 || (layer >= 0 && layer < broken) {
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
        let part_aim = self.read_parts();
        self.spawn_age += dt;
        self.act_pop = decay(self.act_pop, dt, ACT_POP_HALF_LIFE);
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
            pivot.set_position(self.recoil + Vector2::new(0.0, -self.lift));
            // Squash/stretch along the facing axis, volume kept; coil before an action
            // and pop when it lands.
            let sq = self.squash;
            let size =
                spawn_pop(self.spawn_age) * (1.0 - 0.12 * self.windup) * (1.0 + 0.2 * self.act_pop);
            pivot.set_scale(Vector2::new(sq, 1.0 / sq) * size);
            let current = pivot.get_rotation();
            let sway = if self.face_motion {
                0.04 * (self.beat as f32 * std::f32::consts::PI).sin()
            } else {
                0.0
            };
            if self.aim_symmetry > 0 && part_aim.is_some() {
                self.aim_angle = part_aim;
            }
            let target = if let Some(aim) = part_aim.filter(|_| self.aim_symmetry <= 0) {
                Some(aim)
            } else if let Some(aim) = self.aim_angle {
                let step = TAU / self.aim_symmetry.max(1) as f32;
                // Nearest equivalent orientation for symmetric bodies.
                let diff = (aim - current).rem_euclid(step);
                Some(current + if diff > step / 2.0 { diff - step } else { diff })
            } else if self.face_motion && velocity.length() > 4.0 {
                Some(velocity.angle() + sway)
            } else {
                None
            };
            if let Some(target) = target {
                let diff = (target - current + std::f32::consts::PI).rem_euclid(TAU)
                    - std::f32::consts::PI;
                pivot.set_rotation(current + diff * (1.0 - (-dt * TURN_RATE).exp()));
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
        let node = self.to_gd().upcast::<Node>();
        self.beat = self.clock.beat(&node, delta);
        self.watch_shields();
        self.animate(dt);
        self.blink_before_leaving();
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let r = self.body_radius;
        // Dark core behind the metal body plus a thin amber ring: the enemy family
        // marker, kept clear of the player-colored shield rings outside it.
        let glow = ENEMY_GLOW;
        let center = self.recoil + Vector2::new(0.0, -self.lift);
        let flash = self.flash.max(if self.windup > 0.8 { 0.5 } else { 0.0 });
        if self.lift > 1.0 {
            // Ground shadow shrinking as the body rises.
            let k = (1.0 - self.lift / 60.0).clamp(0.4, 1.0);
            self.base_mut().draw_circle(
                Vector2::new(0.0, 4.0),
                r * 0.8 * k,
                Color::from_rgba(0.0, 0.0, 0.0, 0.35 * k),
            );
        }
        self.base_mut()
            .draw_circle(center, r * 0.95, Color::from_rgba(0.03, 0.03, 0.07, 0.85));
        self.base_mut()
            .draw_arc_ex(
                center,
                r * 0.95,
                0.0,
                TAU,
                32,
                Color::from_rgba(glow.r, glow.g, glow.b, 0.12 + 0.3 * flash),
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
                Color::from_rgba(glow.r, glow.g, glow.b, 0.75),
            )
            .width(1.5)
            .antialiased(true)
            .done();

        let windup = self.windup;
        let origin = self.base().get_global_position();
        let markers = self.markers.clone();
        for (at, radius) in markers {
            // Where a hop or step will land: a turning dashed ring with a center dot.
            let local = at - origin;
            let alpha = 0.18 + 0.6 * windup;
            let color = Color::from_rgba(glow.r, glow.g, glow.b, alpha);
            let dashes = 12;
            let spin = self.time * 1.5;
            for i in 0..dashes {
                let a0 = spin + i as f32 * TAU / dashes as f32;
                self.base_mut()
                    .draw_arc_ex(local, radius, a0, a0 + TAU / dashes as f32 * 0.55, 4, color)
                    .width(2.0)
                    .done();
            }
            self.base_mut().draw_circle(local, 3.0, color);
        }
        if windup > 0.0 {
            // Anticipation: an amber ring closing in on the outer shield, flashing
            // white just before the action beat.
            let rest = self.outer_radius.max(r) + 6.0;
            let radius = rest + 40.0 * (1.0 - windup) * (1.0 - windup);
            let hot = windup > 0.8 && (self.time * 20.0).fract() < 0.5;
            let color = if hot {
                Color::from_rgba(1.0, 0.97, 0.9, 0.9)
            } else {
                Color::from_rgba(glow.r, glow.g, glow.b, 0.2 + 0.7 * windup)
            };
            self.base_mut()
                .draw_arc_ex(center, radius, 0.0, TAU, 36, color)
                .width(2.0 + 2.0 * windup)
                .antialiased(true)
                .done();
        }

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
