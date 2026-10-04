use crate::core::danger::DangerShape;
use crate::groups;
use crate::hazards::{encode_shapes, to_v2};
use godot::classes::{
    Area2D, CircleShape2D, CollisionShape2D, IArea2D, Node2D, Texture2D, Timer,
    VisibleOnScreenNotifier2D,
};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;
const PROJECTILE_GLOW: Color = crate::visuals::enemy_visual::ENEMY_GLOW;
const TRAIL_STEPS: i32 = 4;
/// Spin of a skinned projectile (radians per second).
const SKIN_SPIN: f32 = 5.0;
/// Skin diameter as a multiple of the collision radius.
const SKIN_SCALE: f32 = 2.8;

#[derive(GodotClass)]
#[class(init, base = Area2D)]
struct Projectile {
    #[var]
    #[init(val = Vector2::RIGHT)]
    direction: Vector2,

    #[var]
    #[init(val = 100.0)]
    speed: f32,

    #[var]
    #[init(val = 30.0)]
    lifetime: f32,

    /// Drawn spinning instead of the orb (themed shots: potatoes, pills). Set from the
    /// firing enemy's `projectile_skin` metadata by `spawn_projectile`.
    #[var]
    skin: Option<Gd<Texture2D>>,

    #[init(val = 12.0)]
    radius: f32,
    age: f32,

    base: Base<Area2D>,
}

#[godot_api]
impl IArea2D for Projectile {
    fn ready(&mut self) {
        self.radius = self.collision_radius();
        self.base_mut().add_to_group(groups::ENEMY_PROJECTILES);
        self.base_mut().add_to_group(groups::DANGER);

        let projectile = self.to_gd();

        self.base_mut()
            .signals()
            .body_entered()
            .connect_other(&projectile, Self::on_body_entered);

        let notifier = VisibleOnScreenNotifier2D::new_alloc();
        notifier.signals().screen_exited().connect_other(
            &projectile,
            |projectile: &mut Projectile| {
                projectile.base_mut().queue_free();
            },
        );
        self.base_mut().add_child(&notifier);

        let mut timer = Timer::new_alloc();
        timer.set_wait_time(self.lifetime as f64);
        timer.set_one_shot(true);
        timer
            .signals()
            .timeout()
            .connect_other(&projectile, |projectile: &mut Projectile| {
                projectile.base_mut().queue_free();
            });
        self.base_mut().add_child(&timer);
        timer.start();

        self.base_mut().queue_redraw();
    }

    fn process(&mut self, delta: f64) {
        self.age += delta as f32;
        self.base_mut().queue_redraw();
        let movement = self.direction * self.speed * delta as f32;
        let new_position = self.base().get_position() + movement;
        self.base_mut().set_position(new_position);
    }

    /// Amber orb (enemy family, distinct from level-accent hazards) with a white-hot
    /// core and a tapered trail behind its direction of travel.
    fn draw(&mut self) {
        let radius = self.radius;
        let back = -self.direction.normalized_or_zero();
        let glow = PROJECTILE_GLOW;
        let pulse = 0.5 + 0.5 * (self.age * 18.0).sin();

        for i in (1..=TRAIL_STEPS).rev() {
            let t = i as f32 / TRAIL_STEPS as f32;
            let center = back * radius * 2.6 * t;
            self.base_mut().draw_circle(
                center,
                radius * (1.0 - 0.75 * t),
                Color::from_rgba(glow.r, glow.g * 0.8, glow.b * 0.6, 0.35 * (1.0 - t)),
            );
        }
        self.base_mut().draw_circle(
            Vector2::ZERO,
            radius * (1.45 + 0.1 * pulse),
            Color::from_rgba(glow.r, glow.g, glow.b, 0.22),
        );
        if let Some(skin) = self.skin.clone() {
            self.draw_skin(&skin);
            return;
        }
        self.base_mut().draw_circle(Vector2::ZERO, radius, glow);
        self.base_mut()
            .draw_arc_ex(
                Vector2::ZERO,
                radius,
                0.0,
                TAU,
                20,
                Color::from_rgba(1.0, 0.85, 0.6, 1.0),
            )
            .width(1.5)
            .antialiased(true)
            .done();
        self.base_mut().draw_circle(
            Vector2::ZERO,
            radius * 0.5,
            Color::from_rgba(1.0, 0.97, 0.88, 1.0),
        );
    }
}

#[godot_api]
impl Projectile {
    /// The projectile circle moving along `direction * speed`.
    #[func]
    fn danger_shapes(&self) -> PackedFloat32Array {
        encode_shapes(&[DangerShape::Circle {
            center: to_v2(self.base().get_global_position()),
            radius: self.radius,
            velocity: to_v2(self.direction * self.speed),
            activates_in: 0.0,
        }])
    }
}

impl Projectile {
    /// The skin texture in warm amber-white over a solid amber disc, so themed shots
    /// still read as enemy fire.
    fn draw_skin(&mut self, skin: &Gd<Texture2D>) {
        let radius = self.radius;
        let glow = PROJECTILE_GLOW;
        self.base_mut().draw_circle(
            Vector2::ZERO,
            radius * 0.95,
            Color::from_rgba(glow.r, glow.g, glow.b, 0.9),
        );
        let size = Vector2::splat(radius * SKIN_SCALE);
        let angle = self.age * SKIN_SPIN;
        let mut base = self.base_mut();
        base.draw_set_transform_ex(Vector2::ZERO)
            .rotation(angle)
            .done();
        base.draw_texture_rect_ex(skin, Rect2::new(-size / 2.0, size), false)
            .modulate(Color::from_rgba(1.0, 0.93, 0.8, 1.0))
            .done();
        base.draw_set_transform(Vector2::ZERO);
    }

    fn collision_radius(&self) -> f32 {
        let shape_node = self
            .base()
            .get_node_as::<CollisionShape2D>("CollisionShape2D");
        let Some(shape) = shape_node.get_shape() else {
            return self.radius;
        };

        shape
            .try_cast::<CircleShape2D>()
            .map(|circle| circle.get_radius())
            .unwrap_or(self.radius)
    }

    fn on_body_entered(&mut self, mut body: Gd<Node2D>) {
        if body.has_method("take_damage") {
            body.call("take_damage", &[1.0f32.to_variant()]);
        }

        self.base_mut().queue_free();
    }
}
