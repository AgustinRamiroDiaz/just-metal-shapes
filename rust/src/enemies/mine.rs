//! `Mine` (dropped by `MineDropperComponent`) and `SpawnEffect` (the telegraph that
//! plays before an inside spawn).

use crate::core::danger::{DangerShape, V2};
use crate::groups;
use crate::hazards::{encode_shapes, to_v2};
use godot::classes::{Area2D, GpuParticles2D, IArea2D, IGpuParticles2D, Node2D, Timer};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;

#[derive(GodotClass)]
#[class(init, base = Area2D)]
struct Mine {
    armed: bool,
    #[init(val = ARM_TIME)]
    arm_remaining: f32,
    age: f32,
    base: Base<Area2D>,
}

#[godot_api]
impl Mine {
    /// The mine's blast circle; `activates_in` counts down until it arms.
    #[func]
    fn danger_shapes(&self) -> PackedFloat32Array {
        encode_shapes(&[DangerShape::Circle {
            center: to_v2(self.base().get_global_position()),
            radius: MINE_RADIUS,
            velocity: V2::ZERO,
            activates_in: if self.armed { 0.0 } else { self.arm_remaining },
        }])
    }
}

#[godot_api]
impl IArea2D for Mine {
    fn ready(&mut self) {
        self.base_mut().set_collision_layer(0);
        self.base_mut().set_collision_mask(1);
        self.base_mut().add_to_group(groups::MINES);
        self.base_mut().add_to_group(groups::DANGER);

        let mine = self.to_gd();
        self.base_mut()
            .signals()
            .body_entered()
            .connect_other(&mine, Self::on_body_entered);

        let mut arm_timer = Timer::new_alloc();
        arm_timer.set_wait_time(ARM_TIME as f64);
        arm_timer.set_one_shot(true);
        arm_timer
            .signals()
            .timeout()
            .connect_other(&mine, Self::arm);
        self.base_mut().add_child(&arm_timer);
        arm_timer.start();

        let mut lifetime_timer = Timer::new_alloc();
        lifetime_timer.set_wait_time(MINE_LIFETIME as f64);
        lifetime_timer.set_one_shot(true);
        lifetime_timer
            .signals()
            .timeout()
            .connect_other(&mine, |mine: &mut Mine| {
                mine.base_mut().queue_free();
            });
        self.base_mut().add_child(&lifetime_timer);
        lifetime_timer.start();

        self.base_mut().queue_redraw();
    }

    fn process(&mut self, delta: f64) {
        self.arm_remaining = (self.arm_remaining - delta as f32).max(0.0);
        self.age += delta as f32;
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let glow = MINE_GLOW;
        if !self.armed {
            // Arming: fast blink with a ring closing in on the blast radius.
            let t = 1.0 - (self.arm_remaining / ARM_TIME).clamp(0.0, 1.0);
            let on = (self.age * 16.0).fract() < 0.5;
            let core = if on {
                Color::from_rgba(1.0, 0.95, 0.85, 0.9)
            } else {
                Color::from_rgba(glow.r, glow.g, glow.b, 0.5)
            };
            self.base_mut()
                .draw_circle(Vector2::ZERO, MINE_RADIUS * 0.55, core);
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    MINE_RADIUS * (2.2 - 1.2 * t),
                    0.0,
                    TAU,
                    24,
                    Color::from_rgba(glow.r, glow.g, glow.b, 0.3 + 0.5 * t),
                )
                .width(1.5)
                .done();
            return;
        }
        let pulse = (self.age * 5.0).sin() * 0.5 + 0.5;
        self.base_mut().draw_circle(
            Vector2::ZERO,
            MINE_RADIUS * (1.5 + 0.3 * pulse),
            Color::from_rgba(glow.r, glow.g, glow.b, 0.10 + 0.12 * pulse),
        );
        // Spikes, slowly turning.
        let spin = self.age * 0.8;
        for i in 0..6 {
            let dir = Vector2::from_angle(spin + i as f32 * TAU / 6.0);
            self.base_mut()
                .draw_line_ex(
                    dir * MINE_RADIUS * 0.6,
                    dir * MINE_RADIUS * 1.35,
                    Color::from_rgba(glow.r, glow.g, glow.b, 0.9),
                )
                .width(2.5)
                .done();
        }
        self.base_mut().draw_circle(
            Vector2::ZERO,
            MINE_RADIUS,
            Color::from_rgba(0.16, 0.07, 0.03, 1.0),
        );
        self.base_mut()
            .draw_arc_ex(Vector2::ZERO, MINE_RADIUS, 0.0, TAU, 24, glow)
            .width(2.5)
            .antialiased(true)
            .done();
        let blink = Color::from_rgba(1.0, 0.95, 0.8, 0.4 + 0.6 * pulse);
        self.base_mut()
            .draw_circle(Vector2::ZERO, MINE_RADIUS * 0.35, blink);
    }
}

impl Mine {
    fn arm(&mut self) {
        self.armed = true;
        self.base_mut().queue_redraw();
    }

    fn on_body_entered(&mut self, mut body: Gd<Node2D>) {
        if !self.armed {
            return;
        }

        if body.has_method("take_damage") {
            body.call("take_damage", &[1.0f32.to_variant()]);
        }

        self.base_mut().queue_free();
    }
}

const ARM_TIME: f32 = 0.5;
const MINE_LIFETIME: f32 = 15.0;
const MINE_RADIUS: f32 = 10.0;
/// Enemy family amber, shared with projectiles and `EnemyVisual`.
const MINE_GLOW: Color = crate::visuals::enemy_visual::ENEMY_GLOW;

#[derive(GodotClass)]
#[class(init, base = GpuParticles2D)]
struct SpawnEffect {
    #[var]
    #[init(val = 0.8)]
    duration: f64,
    elapsed: f64,

    base: Base<GpuParticles2D>,
}

#[godot_api]
impl SpawnEffect {
    #[signal]
    fn spawn_ready();
}

#[godot_api]
impl IGpuParticles2D for SpawnEffect {
    fn process(&mut self, delta: f64) {
        self.elapsed += delta;
        if self.elapsed <= self.duration + 0.2 {
            self.base_mut().queue_redraw();
        }
    }

    /// Telegraph: a ring and four ticks closing in on the spawn point, landing as the
    /// enemy appears.
    fn draw(&mut self) {
        let t = (self.elapsed / self.duration.max(0.01)).clamp(0.0, 1.0) as f32;
        if self.elapsed > self.duration {
            return;
        }
        let glow = MINE_GLOW;
        let radius = 18.0 + 62.0 * (1.0 - t) * (1.0 - t);
        let alpha = 0.25 + 0.6 * t;
        self.base_mut()
            .draw_arc_ex(
                Vector2::ZERO,
                radius,
                0.0,
                TAU,
                40,
                Color::from_rgba(glow.r, glow.g, glow.b, alpha),
            )
            .width(2.0)
            .antialiased(true)
            .done();
        let spin = t * 1.5;
        for i in 0..4 {
            let dir = Vector2::from_angle(spin + i as f32 * TAU / 4.0);
            self.base_mut()
                .draw_line_ex(
                    dir * (radius + 4.0),
                    dir * (radius + 14.0),
                    Color::from_rgba(1.0, 0.95, 0.85, alpha),
                )
                .width(2.0)
                .done();
        }
        self.base_mut().draw_circle(
            Vector2::ZERO,
            4.0 + 6.0 * t,
            Color::from_rgba(glow.r, glow.g, glow.b, 0.2 + 0.5 * t),
        );
    }

    fn ready(&mut self) {
        self.base_mut().set_emitting(true);

        let effect = self.to_gd();
        let mut ready_timer = Timer::new_alloc();
        ready_timer.set_wait_time(self.duration);
        ready_timer.set_one_shot(true);
        ready_timer
            .signals()
            .timeout()
            .connect_other(&effect, |effect: &mut SpawnEffect| {
                effect.signals().spawn_ready().emit();
                effect.base_mut().set_emitting(false);

                let mut free_timer = Timer::new_alloc();
                free_timer.set_wait_time(effect.base().get_lifetime());
                free_timer.set_one_shot(true);
                let effect_gd = effect.to_gd();
                free_timer.signals().timeout().connect_other(
                    &effect_gd,
                    |effect: &mut SpawnEffect| {
                        effect.base_mut().queue_free();
                    },
                );
                effect.base_mut().add_child(&free_timer);
                free_timer.start();
            });
        self.base_mut().add_child(&ready_timer);
        ready_timer.start();
    }
}
