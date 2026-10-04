//! `HealthComponent`: life plus colored shield layers. Only lightning in a layer's
//! color breaks it; once every layer is gone, any color damages life. A Warden can add
//! a temporary outer layer (a ward) in its own color.

use super::colors_match;
use godot::classes::{INode2D, Node2D, Time};
use godot::global::randi_range;
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct HealthComponent {
    #[var]
    #[init(val = 3.0)]
    max_life: f32,

    #[var]
    pub shield_colors: PackedColorArray,

    #[var]
    auto_shield_layers: i32,

    #[var]
    pub life: f32,

    shield_fills: Vec<f32>,
    /// Instance id of the Warden whose ward is layer 0, if any.
    ward_source: Option<i64>,
    damage_flash_timer: f32,
    base: Base<Node2D>,
}

#[godot_api]
impl HealthComponent {
    #[signal]
    fn damaged(amount: f32);

    #[signal]
    fn died();

    #[func]
    pub fn get_active_layer(&self) -> i32 {
        self.shield_fills
            .iter()
            .position(|fill| *fill > 0.0)
            .map(|idx| idx as i32)
            .unwrap_or(-1)
    }

    #[func]
    pub fn get_active_color(&self) -> Color {
        let idx = self.get_active_layer();
        if idx >= 0 {
            self.shield_colors.get(idx as usize).unwrap_or(Color::WHITE)
        } else {
            Color::WHITE
        }
    }

    /// Ring radius of shield layer `layer` (0 = outermost).
    #[func]
    fn get_layer_radius(&self, layer: i32) -> f32 {
        if layer < 0 || layer as usize >= self.shield_fills.len() {
            return SHIELD_BASE_RADIUS;
        }
        self.shield_radius(layer as usize)
    }

    /// Adds a ward: an outer layer in `color` owned by `source` (a Warden's instance
    /// id). Replaces a broken ward; does nothing while an unbroken ward is up.
    #[func]
    pub fn grant_ward(&mut self, color: Color, source: i64) -> bool {
        if self.life <= 0.0 {
            return false;
        }
        if self.ward_source.is_some() {
            if self.shield_fills.first().is_some_and(|fill| *fill > 0.0) {
                return false;
            }
            self.remove_ward_layer();
        }
        let mut colors = PackedColorArray::new();
        colors.push(color);
        colors.extend_array(&self.shield_colors);
        self.shield_colors = colors;
        self.shield_fills.insert(0, 1.0);
        self.ward_source = Some(source);
        self.base_mut().queue_redraw();
        true
    }

    /// Drops the ward granted by `source` (broken or not).
    #[func]
    pub fn revoke_ward(&mut self, source: i64) -> bool {
        if self.ward_source != Some(source) {
            return false;
        }
        self.remove_ward_layer();
        self.base_mut().queue_redraw();
        true
    }

    #[func]
    pub fn has_ward(&self) -> bool {
        self.ward_source.is_some()
    }

    /// Recolors shield layer `layer` (0 = outermost).
    #[func]
    pub fn set_layer_color(&mut self, layer: i32, color: Color) {
        if layer >= 0 && (layer as usize) < self.shield_colors.len() {
            self.shield_colors[layer as usize] = color;
            self.base_mut().queue_redraw();
        }
    }

    #[func]
    pub fn get_layer_count(&self) -> i32 {
        self.shield_fills.len() as i32
    }

    #[func]
    pub fn take_damage(
        &mut self,
        amount: f32,
        #[opt(default = Color::WHITE)] damage_color: Color,
    ) -> bool {
        if self.life <= 0.0 {
            return false;
        }

        let active = self.get_active_layer();
        if active >= 0 {
            let active_idx = active as usize;
            let active_color = self.shield_colors.get(active_idx).unwrap_or(Color::WHITE);
            if colors_match(active_color, damage_color) {
                self.shield_fills[active_idx] = (self.shield_fills[active_idx] - amount).max(0.0);
                self.damage_flash_timer = DAMAGE_FLASH_DURATION;
                self.signals().damaged().emit(amount);
                self.base_mut().queue_redraw();
                return true;
            }

            return false;
        }

        self.life = (self.life - amount).max(0.0);
        self.damage_flash_timer = DAMAGE_FLASH_DURATION;
        self.signals().damaged().emit(amount);
        self.base_mut().queue_redraw();

        if self.life == 0.0 {
            self.signals().died().emit();
        }

        true
    }
}

#[godot_api]
impl INode2D for HealthComponent {
    fn ready(&mut self) {
        self.life = self.max_life;
        self.init_shields();
        self.base_mut().queue_redraw();
    }

    fn process(&mut self, delta: f64) {
        if self.damage_flash_timer > 0.0 {
            self.damage_flash_timer -= delta as f32;
        }
        // The active ring turns and breathes every frame.
        self.base_mut().queue_redraw();
    }

    /// Shield rings as plates (`SHIELD_SEGMENTS` arcs with gaps), built into two
    /// batched line lists (glow, then track + core) so each enemy costs a couple of
    /// draw calls however many layers it has.
    fn draw(&mut self) {
        let seconds = Time::singleton().get_ticks_msec() as f32 / 1000.0;
        let pulse = (seconds * PULSE_SPEED).sin() * 0.5 + 0.5;
        let active = self.get_active_layer();
        let flash = (self.damage_flash_timer / DAMAGE_FLASH_DURATION).clamp(0.0, 1.0);

        let mut glow = RingLines::default();
        let mut core = RingLines::default();
        for i in (0..self.shield_fills.len()).rev() {
            let fill = self.shield_fills[i];
            if fill <= 0.0 {
                continue;
            }
            let radius = self.shield_radius(i);
            let color = self.shield_colors.get(i).unwrap_or(Color::WHITE);
            let is_active = i as i32 == active;
            let (alpha, glow_alpha, spin) = if is_active {
                (
                    0.85 + 0.15 * pulse,
                    0.18 + 0.12 * pulse + 0.35 * flash,
                    seconds * 0.8,
                )
            } else {
                (0.55, 0.08, 0.0)
            };
            let lift = if is_active { flash * 0.7 } else { 0.0 };
            let core_color = Color::from_rgba(
                color.r + (1.0 - color.r) * lift,
                color.g + (1.0 - color.g) * lift,
                color.b + (1.0 - color.b) * lift,
                alpha,
            );
            // Faint full track behind the remaining plates.
            core.arc(
                radius,
                0.0,
                TAU,
                Color::from_rgba(color.r, color.g, color.b, 0.12),
            );

            // A ward (borrowed from a Warden) shows as fewer, wider-spaced plates.
            let is_ward = i == 0 && self.ward_source.is_some();
            let (segments, gap) = if is_ward {
                (WARD_SEGMENTS, WARD_GAP)
            } else {
                (SHIELD_SEGMENTS, SHIELD_GAP)
            };
            let start = -std::f32::consts::FRAC_PI_2 + if is_ward { -spin } else { spin };
            let seg = TAU / segments as f32;
            let end = TAU * fill;
            for s in 0..segments {
                let a0 = s as f32 * seg;
                if a0 >= end {
                    break;
                }
                let a1 = (a0 + seg - gap).min(end);
                if a1 <= a0 {
                    continue;
                }
                glow.arc(
                    radius,
                    start + a0,
                    start + a1,
                    Color::from_rgba(color.r, color.g, color.b, glow_alpha),
                );
                core.arc(radius, start + a0, start + a1, core_color);
            }
        }
        if !glow.points.is_empty() {
            self.base_mut()
                .draw_multiline_colors_ex(&glow.points, &glow.colors)
                .width(SHIELD_WIDTH + 6.0)
                .done();
        }
        if !core.points.is_empty() {
            self.base_mut()
                .draw_multiline_colors_ex(&core.points, &core.colors)
                .width(SHIELD_WIDTH)
                .done();
        }

        let health_ratio = self.life / self.max_life.max(0.001);
        if health_ratio > 0.0 && health_ratio < 1.0 {
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    HEALTH_RADIUS,
                    -std::f32::consts::FRAC_PI_2,
                    -std::f32::consts::FRAC_PI_2 + TAU * health_ratio,
                    32,
                    Color::from_rgba(1.0, 1.0, 1.0, 0.75),
                )
                .width(2.5)
                .antialiased(true)
                .done();
        }
    }
}

/// Arcs flattened into a `draw_multiline_colors` list (one color per segment).
#[derive(Default)]
struct RingLines {
    points: PackedVector2Array,
    colors: PackedColorArray,
}

impl RingLines {
    fn arc(&mut self, radius: f32, from: f32, to: f32, color: Color) {
        let steps = ((to - from) / ARC_STEP).ceil().max(1.0) as i32;
        let mut previous = Vector2::from_angle(from) * radius;
        for k in 1..=steps {
            let angle = from + (to - from) * k as f32 / steps as f32;
            let point = Vector2::from_angle(angle) * radius;
            self.points.push(previous);
            self.points.push(point);
            self.colors.push(color);
            previous = point;
        }
    }
}

/// Radians per line piece when flattening shield arcs.
const ARC_STEP: f32 = 0.1;

impl HealthComponent {
    fn init_shields(&mut self) {
        self.shield_fills.clear();

        if self.auto_shield_layers > 0 && self.shield_colors.is_empty() {
            let players = self.base().get_tree().get_nodes_in_group("players");
            if !players.is_empty() {
                let mut colors = PackedColorArray::new();
                for _ in 0..self.auto_shield_layers {
                    let idx = randi_range(0, players.len() as i64 - 1) as usize;
                    let color = players
                        .get(idx)
                        .and_then(|player| player.get("team_color").try_to::<Color>().ok())
                        .unwrap_or(Color::WHITE);
                    colors.push(color);
                }
                self.shield_colors = colors;
            }
        }

        self.shield_fills.resize(self.shield_colors.len(), 1.0);
    }

    fn remove_ward_layer(&mut self) {
        if self.ward_source.take().is_some() && !self.shield_fills.is_empty() {
            self.shield_fills.remove(0);
            self.shield_colors.remove(0);
        }
    }

    fn shield_radius(&self, layer_idx: usize) -> f32 {
        let layer_count = self.shield_fills.len();
        SHIELD_BASE_RADIUS + (layer_count - 1 - layer_idx) as f32 * SHIELD_LAYER_SPACING
    }
}

const DAMAGE_FLASH_DURATION: f32 = 0.4;
const PULSE_SPEED: f32 = 4.0;
const SHIELD_BASE_RADIUS: f32 = 34.0;
const SHIELD_LAYER_SPACING: f32 = 8.0;
const SHIELD_WIDTH: f32 = 4.0;
/// Plates per shield ring and the gap between them (radians).
const SHIELD_SEGMENTS: i32 = 12;
const SHIELD_GAP: f32 = 0.09;
const WARD_SEGMENTS: i32 = 6;
const WARD_GAP: f32 = 0.45;
/// Life arc, shown once the enemy has taken body damage.
const HEALTH_RADIUS: f32 = 28.0;
