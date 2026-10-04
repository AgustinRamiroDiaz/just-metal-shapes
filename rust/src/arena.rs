//! `Arena`: the level background, plus the glue from `LevelDirector` presentation
//! signals to `Fx`.
//!
//! Draws one full-screen rect with `shaders/arena.gdshader` in the level palette
//! (`LevelDirector.get_level_info()` `bg_color`/`accent_color`) and reacts to:
//!
//! - `LevelDirector.arena_pulse`: a soft glow on each bar (nothing moves on single
//!   beats: the background stays calm so hazards and enemies read first);
//! - `palette_shift`: section energy (intro calm, main bright) eased over ~a second;
//! - `camera_kick`: `Fx.shake`;
//! - `flash`: `Fx.flash`;
//! - `checkpoint_reached`: `Fx.sweep` + `checkpoint` SFX;
//! - `rewound`: `Fx.rewind_effect`.

use crate::conductor::Conductor;
use crate::core::feel::{decay, section_energy};
use crate::director::LevelDirector;
use crate::fx::{sfx_at, with_fx};
use godot::classes::{INode2D, Node2D, ResourceLoader, Shader, ShaderMaterial};
use godot::prelude::*;

/// Extra pixels drawn past the viewport so camera shake and roll never show an edge.
const BLEED: f32 = 96.0;
const DEFAULT_BG: Color = Color::from_rgb(0.04, 0.05, 0.09);
const DEFAULT_ACCENT: Color = Color::from_rgb(0.3, 0.85, 1.0);

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct Arena {
    #[export]
    #[init(val = NodePath::from("../LevelDirector"))]
    pub director_path: NodePath,
    #[export]
    #[init(val = NodePath::from("../Conductor"))]
    pub conductor_path: NodePath,

    #[init(val = DEFAULT_BG)]
    bg_color: Color,
    #[init(val = DEFAULT_ACCENT)]
    accent_color: Color,
    palette_loaded: bool,
    #[init(val = 0.35)]
    energy: f32,
    #[init(val = 0.35)]
    target_energy: f32,
    bar_pulse: f32,
    grid_offset: Vector2,
    rect: Rect2,

    director: Option<Gd<LevelDirector>>,
    conductor: Option<Gd<Conductor>>,
    material: Option<Gd<ShaderMaterial>>,

    base: Base<Node2D>,
}

#[godot_api]
impl Arena {
    #[func]
    pub fn get_bar_pulse(&self) -> f32 {
        self.bar_pulse
    }

    #[func]
    pub fn get_energy(&self) -> f32 {
        self.energy
    }

    #[func]
    pub fn get_accent_color(&self) -> Color {
        self.accent_color
    }

    #[func]
    fn _on_arena_pulse(&mut self, _beat: f64, intensity: f64) {
        self.bar_pulse = self
            .bar_pulse
            .max(0.55 + 0.45 * intensity.clamp(0.0, 1.0) as f32);
    }

    #[func]
    fn _on_camera_kick(&mut self, strength: f64) {
        let strength = strength.clamp(0.0, 1.0) as f32;
        self.bar_pulse = self.bar_pulse.max(0.5 + 0.5 * strength);
        with_fx(|fx| fx.shake(0.12 + 0.22 * strength));
    }

    #[func]
    fn _on_flash(&mut self, color: Color, duration_seconds: f64) {
        let seconds = duration_seconds.clamp(0.05, 0.5) as f32;
        with_fx(|fx| fx.flash(Color::from_rgba(color.r, color.g, color.b, 0.6), seconds));
    }

    #[func]
    fn _on_palette_shift(&mut self, section_type: GString, intensity: f64) {
        let target = section_energy(&section_type.to_string(), intensity as f32);
        // A big rise (into a drop) gets a burst of light.
        if target - self.target_energy > 0.2 {
            let accent = self.accent_color;
            with_fx(|fx| fx.flash(Color::from_rgba(accent.r, accent.g, accent.b, 0.5), 0.35));
            self.bar_pulse = 1.0;
        }
        self.target_energy = target;
    }

    #[func]
    fn _on_checkpoint_reached(&mut self, index: i64, _beat: f64) {
        // Checkpoint 0 is the song start; the countdown already marks it.
        if index <= 0 {
            return;
        }
        let accent = self.accent_color;
        let center = self.rect.center();
        with_fx(|fx| fx.sweep(accent.lightened(0.3), 0.75));
        sfx_at("checkpoint", center);
    }

    #[func]
    fn _on_rewound(&mut self, _beat: f64) {
        with_fx(|fx| {
            fx.rewind_effect(0.9);
            fx.flash(Color::from_rgba(0.8, 0.9, 1.0, 0.8), 0.25);
        });
        self.bar_pulse = 1.0;
    }
}

impl Arena {
    fn connect_signals(&mut self) {
        let this = self.to_gd();
        if let Some(mut director) = self.director.clone() {
            for (signal, method) in [
                ("arena_pulse", "_on_arena_pulse"),
                ("camera_kick", "_on_camera_kick"),
                ("flash", "_on_flash"),
                ("palette_shift", "_on_palette_shift"),
                ("checkpoint_reached", "_on_checkpoint_reached"),
                ("rewound", "_on_rewound"),
            ] {
                director.connect(signal, &this.callable(method));
            }
        }
    }

    fn load_palette(&mut self) {
        if self.palette_loaded {
            return;
        }
        let Some(director) = self.director.as_ref() else {
            return;
        };
        let info = director.bind().get_level_info();
        if info.is_empty() {
            return;
        }
        if let Some(bg) = info.get("bg_color").and_then(|v| v.try_to::<Color>().ok()) {
            self.bg_color = bg;
        }
        if let Some(accent) = info
            .get("accent_color")
            .and_then(|v| v.try_to::<Color>().ok())
        {
            self.accent_color = accent;
        }
        self.palette_loaded = true;
    }

    fn seconds_per_beat(&self) -> f32 {
        self.conductor
            .as_ref()
            .map_or(0.5, |c| c.bind().seconds_per_beat() as f32)
            .clamp(0.2, 1.5)
    }

    fn push_uniforms(&mut self) {
        let Some(material) = self.material.as_mut() else {
            return;
        };
        let rect = self.rect;
        let params: [(&str, Variant); 8] = [
            ("bg_color", self.bg_color.to_variant()),
            ("accent_color", self.accent_color.to_variant()),
            ("energy", self.energy.to_variant()),
            ("bar_pulse", self.bar_pulse.to_variant()),
            ("grid_offset", self.grid_offset.to_variant()),
            ("arena_origin", rect.position.to_variant()),
            ("arena_size", rect.size.to_variant()),
            ("grid_size", 64.0f32.to_variant()),
        ];
        for (name, value) in params {
            material.set_shader_parameter(name, &value);
        }
    }
}

#[godot_api]
impl INode2D for Arena {
    fn ready(&mut self) {
        self.base_mut().set_z_index(-1000);
        self.rect = self
            .base()
            .get_viewport()
            .map(|v| v.get_visible_rect())
            .unwrap_or(Rect2::new(Vector2::ZERO, Vector2::new(1280.0, 720.0)));

        if let Some(shader) = ResourceLoader::singleton()
            .load("res://shaders/arena.gdshader")
            .and_then(|r| r.try_cast::<Shader>().ok())
        {
            let mut material = ShaderMaterial::new_gd();
            material.set_shader(&shader);
            self.base_mut().set_material(&material);
            self.material = Some(material);
        }

        self.director = self
            .base()
            .try_get_node_as::<LevelDirector>(&self.director_path);
        self.conductor = self
            .base()
            .try_get_node_as::<Conductor>(&self.conductor_path);
        self.connect_signals();
        self.push_uniforms();
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        self.load_palette();
        let spb = self.seconds_per_beat();
        self.bar_pulse = decay(self.bar_pulse, dt, spb * 0.3);
        let ease = 1.0 - (-dt * 1.5).exp();
        self.energy += (self.target_energy - self.energy) * ease;

        // Grid drifts slowly and steadily; a little faster in energetic sections.
        let speed = 4.0 + 10.0 * self.energy;
        self.grid_offset += Vector2::new(0.8, 0.6) * speed * dt;
        let wrap = 64.0 * 4.0;
        self.grid_offset.x = self.grid_offset.x.rem_euclid(wrap);
        self.grid_offset.y = self.grid_offset.y.rem_euclid(wrap);

        self.push_uniforms();
    }

    fn draw(&mut self) {
        let rect = self.rect.grow(BLEED);
        self.base_mut().draw_rect(rect, Color::WHITE);
    }
}
