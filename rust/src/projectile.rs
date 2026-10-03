use crate::core::danger::DangerShape;
use crate::groups;
use crate::hazards::{encode_shapes, to_v2};
use godot::classes::{
    Area2D, CircleShape2D, CollisionShape2D, IArea2D, Node2D, Timer, VisibleOnScreenNotifier2D,
};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;

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

    #[init(val = 12.0)]
    radius: f32,

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
        let movement = self.direction * self.speed * delta as f32;
        let new_position = self.base().get_position() + movement;
        self.base_mut().set_position(new_position);
    }

    fn draw(&mut self) {
        let center = Vector2::ZERO;
        let radius = self.radius;

        self.base_mut()
            .draw_circle(center, radius, Color::from_rgba(1.0, 0.9, 0.2, 1.0));
        self.base_mut()
            .draw_arc_ex(
                center,
                radius,
                0.0,
                TAU,
                16,
                Color::from_rgba(1.0, 0.6, 0.0, 1.0),
            )
            .width(1.5)
            .done();
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
