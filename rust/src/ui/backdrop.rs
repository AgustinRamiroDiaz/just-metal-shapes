//! `MenuBackdrop`: the animated background behind every menu screen. Drifting
//! diagonal stripes and three outlined squares that kick on each beat of the menu
//! music, tinted with the current accent.

use super::palette;
use godot::classes::control::MouseFilter;
use godot::classes::{Control, IControl};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base = Control)]
pub struct MenuBackdrop {
    time: f64,
    accent: Color,
    base: Base<Control>,
}

#[godot_api]
impl IControl for MenuBackdrop {
    fn ready(&mut self) {
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
        super::full_rect(&mut self.to_gd());
        self.accent = palette::ACCENT;
    }

    fn process(&mut self, delta: f64) {
        self.time += delta
            / godot::classes::Engine::singleton()
                .get_time_scale()
                .max(0.001);
        let target = super::services()
            .map(|ui| ui.bind().get_accent())
            .unwrap_or(palette::ACCENT);
        self.accent = self.accent.lerp(target, (delta * 6.0).min(1.0));
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let size = self.base().get_size();
        let (beat, pulse) = super::services()
            .map(|ui| (ui.bind().music_beat(), ui.bind().beat_pulse()))
            .unwrap_or((self.time * 2.0, 0.0));
        let pulse = pulse as f32;
        let accent = self.accent;
        let t = self.time as f32;

        self.base_mut()
            .draw_rect(Rect2::new(Vector2::ZERO, size), palette::VOID);

        // Diagonal stripes drifting right.
        let spacing = 90.0;
        let lean = size.y * 0.35;
        let drift = (t * 18.0) % spacing;
        let mut x = -lean - spacing + drift;
        let stripe = palette::PANEL.with_alpha(0.55);
        while x < size.x + spacing {
            let points = PackedVector2Array::from(&[
                Vector2::new(x + lean, 0.0),
                Vector2::new(x + lean + 34.0, 0.0),
                Vector2::new(x + 34.0, size.y),
                Vector2::new(x, size.y),
            ]);
            self.base_mut().draw_colored_polygon(&points, stripe);
            x += spacing;
        }

        // Beat-kicked outlined squares, right of center.
        let center = Vector2::new(size.x * 0.74, size.y * 0.48);
        for (i, (radius, speed, alpha)) in [
            (150.0, 0.25, 0.55),
            (250.0, -0.15, 0.32),
            (370.0, 0.08, 0.18),
        ]
        .into_iter()
        .enumerate()
        {
            let kick = 1.0 + pulse * (0.10 - i as f32 * 0.025);
            let r = radius * kick;
            let angle = t * speed + (beat as f32 * 0.02);
            let corners: Vec<Vector2> = (0..5)
                .map(|k| {
                    let a = angle + k as f32 * std::f32::consts::FRAC_PI_2;
                    center + Vector2::new(a.cos(), a.sin()) * r
                })
                .collect();
            let color = accent.with_alpha(alpha * (0.6 + 0.4 * pulse));
            self.base_mut()
                .draw_polyline_ex(&PackedVector2Array::from(corners.as_slice()), color)
                .width(3.0 + 3.0 * pulse * (i == 0) as i32 as f32)
                .antialiased(true)
                .done();
        }

        // Floor glow line.
        let glow = accent.with_alpha(0.12 + 0.18 * pulse);
        self.base_mut().draw_rect(
            Rect2::new(Vector2::new(0.0, size.y - 6.0), Vector2::new(size.x, 6.0)),
            glow,
        );
    }
}

/// A full-screen backdrop for screen `root`.
pub fn add_backdrop(root: &mut Gd<Control>) {
    let backdrop = MenuBackdrop::new_alloc();
    root.add_child(&backdrop);
}
