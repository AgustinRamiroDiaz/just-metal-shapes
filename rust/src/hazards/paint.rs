//! Drawing helpers shared by hazards. All take global-pixel positions: call `begin`
//! first in `draw()` so the canvas maps global coordinates.

use super::to_vector2;
use crate::core::danger::V2;
use godot::classes::Node2D;
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;

/// Maps the canvas to global coordinates for the rest of this `draw()`.
pub fn begin(canvas: &mut Gd<Node2D>) {
    let inverse = canvas.get_global_transform().affine_inverse();
    canvas.draw_set_transform_matrix(inverse);
}

pub fn alpha(color: Color, a: f32) -> Color {
    let mut out = color;
    out.a = (color.a * a).clamp(0.0, 1.0);
    out
}

pub fn whiten(color: Color, amount: f32) -> Color {
    let mut out = color.lerp(Color::WHITE, amount.clamp(0.0, 1.0) as f64);
    out.a = color.a;
    out
}

pub fn line(canvas: &mut Gd<Node2D>, a: V2, b: V2, color: Color, width: f32) {
    canvas
        .draw_line_ex(to_vector2(a), to_vector2(b), color)
        .width(width)
        .antialiased(true)
        .done();
}

pub fn dashed(canvas: &mut Gd<Node2D>, a: V2, b: V2, color: Color, width: f32, dash: f32) {
    canvas
        .draw_dashed_line_ex(to_vector2(a), to_vector2(b), color)
        .width(width)
        .dash(dash)
        .done();
}

/// A solid beam: soft glow, body, and a hot white core.
pub fn beam(canvas: &mut Gd<Node2D>, a: V2, b: V2, width: f32, color: Color) {
    if width <= 0.5 {
        return;
    }
    line(canvas, a, b, alpha(color, 0.22), width * 1.9);
    line(canvas, a, b, color, width);
    line(canvas, a, b, alpha(whiten(color, 0.75), 0.9), width * 0.32);
}

/// Warning strip for a beam: edge lines plus a fill that widens toward the hit.
pub fn warn_beam(
    canvas: &mut Gd<Node2D>,
    a: V2,
    b: V2,
    width: f32,
    color: Color,
    fill_alpha: f32,
    progress: f32,
) {
    let dir = (b - a).normalized_or_zero();
    let normal = V2::new(-dir.y, dir.x) * (width * 0.5);
    line(
        canvas,
        a,
        b,
        alpha(color, fill_alpha * 0.8),
        (width * progress).max(2.0),
    );
    for side in [normal, -normal] {
        line(
            canvas,
            a + side,
            b + side,
            alpha(color, 0.35 + 0.5 * progress),
            2.0,
        );
    }
}

pub fn disc(canvas: &mut Gd<Node2D>, center: V2, radius: f32, color: Color) {
    if radius > 0.0 {
        canvas.draw_circle(to_vector2(center), radius, color);
    }
}

pub fn ring(canvas: &mut Gd<Node2D>, center: V2, radius: f32, width: f32, color: Color) {
    if radius > 0.0 {
        canvas
            .draw_circle_ex(to_vector2(center), radius, color)
            .filled(false)
            .width(width)
            .antialiased(true)
            .done();
    }
}

/// Partial ring from `start` sweeping `sweep` radians.
pub fn arc(
    canvas: &mut Gd<Node2D>,
    center: V2,
    radius: f32,
    start: f32,
    sweep: f32,
    width: f32,
    color: Color,
) {
    let points = ((sweep.abs() / TAU) * 64.0).ceil().max(4.0) as i32;
    canvas
        .draw_arc_ex(
            to_vector2(center),
            radius,
            start,
            start + sweep,
            points,
            color,
        )
        .width(width)
        .antialiased(true)
        .done();
}

/// A bullet: glow, body and white core.
pub fn bullet(canvas: &mut Gd<Node2D>, center: V2, radius: f32, color: Color) {
    disc(canvas, center, radius * 1.55, alpha(color, 0.25));
    disc(canvas, center, radius, color);
    disc(canvas, center, radius * 0.45, alpha(Color::WHITE, color.a));
}

pub fn polygon(canvas: &mut Gd<Node2D>, points: &[V2], color: Color) {
    if points.len() < 3 {
        return;
    }
    let packed: PackedVector2Array = points.iter().map(|p| to_vector2(*p)).collect();
    canvas.draw_colored_polygon(&packed, color);
}

pub fn outline(canvas: &mut Gd<Node2D>, points: &[V2], color: Color, width: f32) {
    if points.len() < 2 {
        return;
    }
    let mut packed: PackedVector2Array = points.iter().map(|p| to_vector2(*p)).collect();
    packed.push(to_vector2(points[0]));
    canvas
        .draw_polyline_ex(&packed, color)
        .width(width)
        .antialiased(true)
        .done();
}

/// Corners of an oriented rectangle.
pub fn rect_points(center: V2, half: V2, angle: f32) -> [V2; 4] {
    [
        center + V2::new(-half.x, -half.y).rotated(angle),
        center + V2::new(half.x, -half.y).rotated(angle),
        center + V2::new(half.x, half.y).rotated(angle),
        center + V2::new(-half.x, half.y).rotated(angle),
    ]
}

/// A chevron (">" shape) at `tip` pointing along `dir`.
pub fn chevron(canvas: &mut Gd<Node2D>, tip: V2, dir: V2, size: f32, color: Color, width: f32) {
    let dir = dir.normalized_or_zero();
    let normal = V2::new(-dir.y, dir.x);
    let back = tip - dir * size;
    line(canvas, back + normal * size, tip, color, width);
    line(canvas, tip, back - normal * size, color, width);
}
