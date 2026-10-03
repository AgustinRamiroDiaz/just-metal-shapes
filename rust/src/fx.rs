//! `Fx` autoload (`/root/Fx`): screen shake, hit-stop, pooled particle bursts, screen
//! flashes, shockwave rings, the checkpoint sweep, the rewind overlay and gameplay SFX.
//!
//! Gameplay code calls it through [`with_fx`] (Rust) or `Fx.<method>` (GDScript); every
//! call is a no-op-safe fire-and-forget. World-space effects (particles, rings, sweep,
//! positional SFX) live under an `FxCanvas` child in the root canvas, so the level's
//! `Camera2D` transforms them like the rest of the arena. Overlays (flash, rewind) live
//! on a `CanvasLayer` at [`OVERLAY_LAYER`], under the HUD.
//!
//! Shake applies trauma to the current viewport `Camera2D` offset and rotation, scaled
//! by `SaveData.screen_shake` when that autoload exists (bool, or a 0..1 float).
//! Hit-stop dips `Engine.time_scale` relative to whatever scale was active and restores
//! it in real time; it ends at once if the tree pauses or something else changes the
//! time scale meanwhile.

use crate::core::feel::{HITSTOP_SCALE, HitStop, Trauma};
use godot::classes::canvas_item_material::BlendMode;
use godot::classes::control::{LayoutPreset, MouseFilter};
use godot::classes::cpu_particles_2d::{Parameter, ParticleFlags};
use godot::classes::node::ProcessMode;
use godot::classes::{
    AudioServer, AudioStream, AudioStreamPlayer2D, Camera2D, CanvasItemMaterial, CanvasLayer,
    ColorRect, CpuParticles2D, Curve, Engine, Gradient, INode, INode2D, Node, Node2D,
    ResourceLoader, SceneTree, Shader, ShaderMaterial, Texture2D, Time,
};
use godot::global::{randf_range, randi};
use godot::prelude::*;
use std::collections::HashMap;

const TAU: f32 = std::f32::consts::TAU;

/// Canvas layer of the flash and rewind overlays. HUD layers should sit above it.
pub const OVERLAY_LAYER: i32 = 5;
/// Z index of world-space effects: above players, enemies and hazards.
pub const WORLD_Z: i32 = 50;
const BURST_POOL_SIZE: usize = 48;
/// Peak alpha of a full-screen flash, whatever the requested color alpha.
const FLASH_MAX_ALPHA: f32 = 0.32;
/// Flash strength kept when the reduce-flashing setting is on.
const REDUCED_FLASH_SCALE: f32 = 0.2;
/// Length in pixels of the checkpoint sweep's gradient wake.
const SWEEP_TRAIL: f32 = 260.0;
const SFX_DIR: &str = "res://assets/sfx";
/// Panning only; volume barely falls off across the arena.
const SFX_MAX_DISTANCE: f32 = 6000.0;
/// `play_sfx` position meaning "screen center".
const SCREEN_CENTER: Vector2 = Vector2::new(f32::INFINITY, f32::INFINITY);

/// Particle look for a burst.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BurstStyle {
    /// Thin streaks aligned to velocity (impacts, deflects).
    Sparks = 0,
    /// Soft glowing dots (spawns, revives, hits).
    Dots = 1,
    /// Spinning flat squares (shatter, explosions).
    Shards = 2,
}

impl BurstStyle {
    fn from_i32(value: i32) -> Self {
        match value {
            1 => BurstStyle::Dots,
            2 => BurstStyle::Shards,
            _ => BurstStyle::Sparks,
        }
    }
}

/// Per-sound playback rules: `(name, volume_db, max_voices, min_interval_ms)`.
/// Unlisted sounds get 0 dB, 4 voices, 30 ms.
const SFX_TABLE: &[(&str, f32, usize, u64)] = &[
    ("enemy_hit", -14.0, 3, 70),
    ("shield_deflect", -12.0, 2, 110),
    ("shield_break", -4.0, 4, 40),
    ("enemy_die", -3.0, 4, 40),
    ("enemy_spawn", -10.0, 3, 60),
    ("player_hit", -2.0, 3, 40),
    ("player_down", 0.0, 2, 60),
    ("player_revive", -2.0, 2, 60),
    ("explosion_small", -8.0, 4, 40),
    ("explosion_big", -4.0, 2, 80),
    ("checkpoint", -4.0, 1, 200),
    ("hazard_warn", -10.0, 3, 60),
    ("laser_charge", -10.0, 3, 60),
    ("laser_fire", -8.0, 3, 40),
];

struct SfxPool {
    stream: Option<Gd<AudioStream>>,
    players: Vec<Gd<AudioStreamPlayer2D>>,
    volume_db: f32,
    max_voices: usize,
    min_interval_ms: u64,
    last_play_ms: Option<u64>,
}

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct Fx {
    trauma: Trauma,
    hitstop: HitStop,
    /// `Engine.time_scale` the current hit-stop interrupted.
    hitstop_base_scale: f64,
    /// `Engine.time_scale` the current hit-stop set.
    hitstop_applied_scale: f64,
    last_ticks_usec: u64,
    shaken_camera: Option<Gd<Camera2D>>,

    canvas: Option<Gd<FxCanvas>>,
    bursts: Vec<Gd<CpuParticles2D>>,
    next_burst: usize,
    textures: HashMap<&'static str, Gd<Texture2D>>,
    additive: Option<Gd<CanvasItemMaterial>>,
    shrink_curve: Option<Gd<Curve>>,
    fade_ramp: Option<Gd<Gradient>>,

    flash_rect: Option<Gd<ColorRect>>,
    flash_color: Color,
    flash_left: f32,
    flash_total: f32,
    rewind_rect: Option<Gd<ColorRect>>,
    rewind_left: f32,
    rewind_total: f32,

    sfx: HashMap<String, SfxPool>,
    dummy_audio: bool,

    base: Base<Node>,
}

#[godot_api]
impl Fx {
    #[constant]
    pub const BURST_SPARKS: i32 = BurstStyle::Sparks as i32;
    #[constant]
    pub const BURST_DOTS: i32 = BurstStyle::Dots as i32;
    #[constant]
    pub const BURST_SHARDS: i32 = BurstStyle::Shards as i32;

    /// Adds screen-shake trauma (0..1; 0.15 small, 0.35 medium, 0.7 large).
    #[func]
    pub fn shake(&mut self, strength: f32) {
        if self.shake_scale() <= 0.0 {
            return;
        }
        self.trauma.add(strength);
    }

    /// Briefly drops `Engine.time_scale` (real seconds, capped). Ignored while paused
    /// and during the cooldown after a previous stop.
    #[func]
    pub fn hitstop(&mut self, seconds: f32) {
        if self.base().get_tree().is_paused() {
            return;
        }
        let was_active = self.hitstop.is_active();
        if !self.hitstop.request(seconds) || was_active {
            return;
        }
        let mut engine = Engine::singleton();
        self.hitstop_base_scale = engine.get_time_scale();
        self.hitstop_applied_scale = self.hitstop_base_scale * HITSTOP_SCALE as f64;
        engine.set_time_scale(self.hitstop_applied_scale);
    }

    /// Particle burst at a global position (sparks plus a few glow dots).
    #[func]
    pub fn burst(&mut self, position: Vector2, color: Color, amount: i32) {
        let amount = amount.max(1);
        self.emit(position, color, amount, BurstStyle::Sparks, 1.0);
        self.emit(position, color, (amount / 3).max(1), BurstStyle::Dots, 0.7);
    }

    /// Particle burst of one style (`BURST_SPARKS`, `BURST_DOTS`, `BURST_SHARDS`);
    /// `speed` scales particle velocity.
    #[func]
    pub fn burst_style(
        &mut self,
        position: Vector2,
        color: Color,
        amount: i32,
        style: i32,
        #[opt(default = 1.0)] speed: f32,
    ) {
        self.emit(
            position,
            color,
            amount.max(1),
            BurstStyle::from_i32(style),
            speed,
        );
    }

    /// Full-screen additive flash fading out over `seconds`.
    #[func]
    pub fn flash(&mut self, color: Color, seconds: f32) {
        if seconds <= 0.0 {
            return;
        }
        // A stronger request replaces a weaker one already fading.
        let current = self.flash_alpha();
        let requested = color.a.min(1.0) * FLASH_MAX_ALPHA;
        if requested >= current {
            self.flash_color = color;
            self.flash_total = seconds;
            self.flash_left = seconds;
        }
    }

    /// Expanding ring at a global position (shockwaves, knockback rings).
    #[func]
    pub fn ring(&mut self, position: Vector2, color: Color, radius: f32, seconds: f32) {
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.bind_mut().add_ring(position, color, radius, seconds);
        }
    }

    /// A bright line sweeping left to right across the screen (checkpoints).
    #[func]
    pub fn sweep(&mut self, color: Color, seconds: f32) {
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.bind_mut().start_sweep(color, seconds);
        }
    }

    /// VHS-style rewind overlay (desaturate, scanlines, tracking jitter) for `seconds`.
    #[func]
    pub fn rewind_effect(&mut self, seconds: f32) {
        self.rewind_total = seconds.max(0.05);
        self.rewind_left = self.rewind_total;
    }

    /// Plays `res://assets/sfx/<name>.ogg` on the `SFX` bus (`Master` if missing).
    /// `position` (global; omitted = screen center) pans the sound. Each
    /// sound has a voice limit and a minimum retrigger interval; extra calls are dropped
    /// or steal the oldest voice.
    #[func]
    pub fn play_sfx(
        &mut self,
        name: GString,
        #[opt(default = SCREEN_CENTER)] position: Vector2,
        #[opt(default = 0.06)] pitch_jitter: f32,
        #[opt(default = 0.0)] volume_db: f32,
    ) -> bool {
        let position = if position.is_finite() {
            position
        } else {
            self.screen_center()
        };
        self.play(&name.to_string(), position, pitch_jitter, volume_db)
    }

    #[func]
    pub fn get_trauma(&self) -> f32 {
        self.trauma.value
    }

    #[func]
    pub fn is_hitstop_active(&self) -> bool {
        self.hitstop.is_active()
    }

    /// Effective shake multiplier from `SaveData.screen_shake` (1 without SaveData).
    #[func]
    pub fn shake_scale(&self) -> f32 {
        let Some(save) = self.base().get_node_or_null("/root/SaveData") else {
            return 1.0;
        };
        let value = save.get("screen_shake");
        if let Ok(on) = value.try_to::<bool>() {
            return if on { 1.0 } else { 0.0 };
        }
        value
            .try_to::<f64>()
            .map_or(1.0, |v| v.clamp(0.0, 2.0) as f32)
    }

    /// Clears all running effects and restores the time scale (scene changes, tests).
    #[func]
    pub fn reset(&mut self) {
        self.trauma = Trauma::default();
        if self.hitstop.is_active() {
            self.end_hitstop(true);
        }
        self.hitstop = HitStop::default();
        self.flash_left = 0.0;
        self.rewind_left = 0.0;
        self.apply_camera();
        self.apply_overlays();
        for burst in self.bursts.iter_mut() {
            burst.set_emitting(false);
        }
        if let Some(canvas) = self.canvas.as_mut() {
            canvas.bind_mut().clear();
        }
    }
}

impl Fx {
    fn real_delta(&mut self) -> f32 {
        let now = Time::singleton().get_ticks_usec();
        let dt = if self.last_ticks_usec == 0 {
            0.0
        } else {
            (now.saturating_sub(self.last_ticks_usec)) as f32 / 1_000_000.0
        };
        self.last_ticks_usec = now;
        dt.min(0.1)
    }

    fn end_hitstop(&mut self, restore: bool) {
        self.hitstop.cancel();
        if restore {
            Engine::singleton().set_time_scale(self.hitstop_base_scale);
        }
    }

    fn step_hitstop(&mut self, real_dt: f32) {
        if !self.hitstop.is_active() {
            self.hitstop.step(real_dt);
            return;
        }
        let current = Engine::singleton().get_time_scale();
        if (current - self.hitstop_applied_scale).abs() > 1e-9 {
            // Someone else set the time scale: theirs wins.
            self.end_hitstop(false);
            return;
        }
        if self.base().get_tree().is_paused() {
            self.end_hitstop(true);
            return;
        }
        if self.hitstop.step(real_dt) {
            Engine::singleton().set_time_scale(self.hitstop_base_scale);
        }
    }

    fn apply_camera(&mut self) {
        let camera = self
            .base()
            .get_viewport()
            .and_then(|viewport| viewport.get_camera_2d());
        if let Some(previous) = self.shaken_camera.as_mut()
            && previous.is_instance_valid()
            && camera.as_ref() != Some(previous)
        {
            previous.set_offset(Vector2::ZERO);
            previous.set_rotation(0.0);
        }
        self.shaken_camera = camera.clone();
        let Some(mut camera) = camera else {
            return;
        };
        let (x, y, roll) = self.trauma.offset(self.shake_scale());
        camera.set_offset(Vector2::new(x, y));
        camera.set_rotation(roll);
    }

    fn flash_alpha(&self) -> f32 {
        if self.flash_left <= 0.0 || self.flash_total <= 0.0 {
            return 0.0;
        }
        let t = self.flash_left / self.flash_total;
        self.flash_color.a.min(1.0) * FLASH_MAX_ALPHA * t * t * self.flash_scale()
    }

    /// Full-screen flash multiplier: dimmed when `SaveData.reduce_flashing` is on.
    fn flash_scale(&self) -> f32 {
        let reduced = self
            .base()
            .get_node_or_null("/root/SaveData")
            .is_some_and(|save| {
                save.get("reduce_flashing")
                    .try_to::<bool>()
                    .unwrap_or(false)
            });
        if reduced { REDUCED_FLASH_SCALE } else { 1.0 }
    }

    fn apply_overlays(&mut self) {
        let alpha = self.flash_alpha();
        let color = self.flash_color;
        if let Some(rect) = self.flash_rect.as_mut() {
            rect.set_visible(alpha > 0.002);
            rect.set_color(Color::from_rgba(color.r, color.g, color.b, alpha));
        }
        let amount = if self.rewind_total > 0.0 {
            (self.rewind_left / self.rewind_total).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Some(rect) = self.rewind_rect.as_mut() {
            rect.set_visible(amount > 0.0);
            if let Some(material) = rect.get_material()
                && let Ok(mut material) = material.try_cast::<ShaderMaterial>()
            {
                // Ease out: full strength at the start, a quick settle at the end.
                let eased = 1.0 - (1.0 - amount).powi(3);
                material.set_shader_parameter("amount", &eased.to_variant());
            }
        }
    }

    fn screen_center(&self) -> Vector2 {
        let Some(viewport) = self.base().get_viewport() else {
            return Vector2::ZERO;
        };
        match viewport.get_camera_2d() {
            Some(camera) => camera.get_screen_center_position(),
            None => viewport.get_visible_rect().center(),
        }
    }

    fn texture(&mut self, path: &'static str) -> Option<Gd<Texture2D>> {
        if let Some(texture) = self.textures.get(path) {
            return Some(texture.clone());
        }
        let texture = ResourceLoader::singleton()
            .load(path)
            .and_then(|r| r.try_cast::<Texture2D>().ok())?;
        self.textures.insert(path, texture.clone());
        Some(texture)
    }

    fn emit(
        &mut self,
        position: Vector2,
        color: Color,
        amount: i32,
        style: BurstStyle,
        speed: f32,
    ) {
        if self.bursts.is_empty() {
            return;
        }
        let index = self.next_burst % self.bursts.len();
        self.next_burst = (index + 1) % self.bursts.len();
        let texture = match style {
            BurstStyle::Sparks => {
                const TRACES: [&str; 3] = [
                    "res://assets/kenney-particles/trace_01.png",
                    "res://assets/kenney-particles/trace_02.png",
                    "res://assets/kenney-particles/trace_04.png",
                ];
                self.texture(TRACES[randi() as usize % TRACES.len()])
            }
            BurstStyle::Dots => self.texture("res://assets/kenney-particles/circle_05.png"),
            BurstStyle::Shards => None,
        };
        let additive = self.additive.clone();
        let mut p = self.bursts[index].clone();
        let speed = speed.max(0.05);
        match style {
            BurstStyle::Sparks => {
                p.set_lifetime(0.32);
                p.set_param_min(Parameter::INITIAL_LINEAR_VELOCITY, 160.0 * speed);
                p.set_param_max(Parameter::INITIAL_LINEAR_VELOCITY, 380.0 * speed);
                p.set_param_min(Parameter::DAMPING, 500.0 * speed);
                p.set_param_max(Parameter::DAMPING, 800.0 * speed);
                p.set_param_min(Parameter::SCALE, 0.07);
                p.set_param_max(Parameter::SCALE, 0.12);
                p.set_param_min(Parameter::ANGULAR_VELOCITY, 0.0);
                p.set_param_max(Parameter::ANGULAR_VELOCITY, 0.0);
                p.set_particle_flag(ParticleFlags::ALIGN_Y_TO_VELOCITY, true);
                p.set_emission_sphere_radius(4.0);
            }
            BurstStyle::Dots => {
                p.set_lifetime(0.55);
                p.set_param_min(Parameter::INITIAL_LINEAR_VELOCITY, 30.0 * speed);
                p.set_param_max(Parameter::INITIAL_LINEAR_VELOCITY, 140.0 * speed);
                p.set_param_min(Parameter::DAMPING, 120.0 * speed);
                p.set_param_max(Parameter::DAMPING, 220.0 * speed);
                p.set_param_min(Parameter::SCALE, 0.05);
                p.set_param_max(Parameter::SCALE, 0.11);
                p.set_param_min(Parameter::ANGULAR_VELOCITY, 0.0);
                p.set_param_max(Parameter::ANGULAR_VELOCITY, 0.0);
                p.set_particle_flag(ParticleFlags::ALIGN_Y_TO_VELOCITY, false);
                p.set_emission_sphere_radius(8.0);
            }
            BurstStyle::Shards => {
                p.set_lifetime(0.75);
                p.set_param_min(Parameter::INITIAL_LINEAR_VELOCITY, 90.0 * speed);
                p.set_param_max(Parameter::INITIAL_LINEAR_VELOCITY, 300.0 * speed);
                p.set_param_min(Parameter::DAMPING, 220.0 * speed);
                p.set_param_max(Parameter::DAMPING, 380.0 * speed);
                p.set_param_min(Parameter::SCALE, 4.0);
                p.set_param_max(Parameter::SCALE, 9.0);
                p.set_param_min(Parameter::ANGULAR_VELOCITY, -720.0);
                p.set_param_max(Parameter::ANGULAR_VELOCITY, 720.0);
                p.set_particle_flag(ParticleFlags::ALIGN_Y_TO_VELOCITY, false);
                p.set_emission_sphere_radius(6.0);
            }
        }
        match texture {
            Some(texture) => p.set_texture(&texture),
            None => p.set_texture(Gd::<Texture2D>::null_arg()),
        }
        if style == BurstStyle::Shards {
            p.set_material(Gd::<godot::classes::Material>::null_arg());
        } else if let Some(material) = &additive {
            p.set_material(material);
        }
        p.set_amount(amount.min(64));
        p.set_color(color);
        p.set_global_position(position);
        p.restart();
        p.set_emitting(true);
    }

    fn play(&mut self, name: &str, position: Vector2, pitch_jitter: f32, volume_db: f32) -> bool {
        let now = Time::singleton().get_ticks_msec();
        let Some(canvas) = self.canvas.clone() else {
            return false;
        };
        let pool = self.sfx.entry(name.to_string()).or_insert_with(|| {
            let (volume_db, max_voices, min_interval_ms) = SFX_TABLE
                .iter()
                .find(|(n, ..)| *n == name)
                .map_or((0.0, 4, 30), |&(_, db, voices, ms)| (db, voices, ms));
            let stream = ResourceLoader::singleton()
                .load(&format!("{SFX_DIR}/{name}.ogg"))
                .and_then(|r| r.try_cast::<AudioStream>().ok());
            if stream.is_none() {
                godot_warn!("Fx: missing sound '{name}'");
            }
            SfxPool {
                stream,
                players: Vec::new(),
                volume_db,
                max_voices,
                min_interval_ms,
                last_play_ms: None,
            }
        });
        let Some(stream) = pool.stream.clone() else {
            return false;
        };
        if let Some(last) = pool.last_play_ms
            && now.saturating_sub(last) < pool.min_interval_ms
        {
            return false;
        }
        let free = pool.players.iter().position(|p| !p.is_playing());
        let index = match free {
            Some(index) => index,
            None if pool.players.len() < pool.max_voices => {
                let mut player = AudioStreamPlayer2D::new_alloc();
                player.set_stream(&stream);
                player.set_max_distance(SFX_MAX_DISTANCE);
                player.set_panning_strength(0.6);
                canvas.clone().upcast::<Node>().add_child(&player);
                pool.players.push(player);
                pool.players.len() - 1
            }
            None => {
                // Steal the voice furthest into its sound.
                pool.players
                    .iter()
                    .enumerate()
                    .max_by(|a, b| {
                        a.1.get_playback_position()
                            .total_cmp(&b.1.get_playback_position())
                    })
                    .map_or(0, |(i, _)| i)
            }
        };
        pool.last_play_ms = Some(now);
        if self.dummy_audio {
            // The dummy driver (headless) never mixes, so playbacks would never be
            // released; voice limits and throttling still apply above.
            return true;
        }
        let bus = if AudioServer::singleton().get_bus_index("SFX") >= 0 {
            "SFX"
        } else {
            "Master"
        };
        let jitter = pitch_jitter.abs().min(0.5) as f64;
        let player = &mut pool.players[index];
        player.set_bus(bus);
        player.set_volume_db(pool.volume_db + volume_db);
        player.set_pitch_scale((1.0 + randf_range(-jitter, jitter)) as f32);
        player.set_global_position(position);
        player.play();
        true
    }

    fn build(&mut self) {
        let mut canvas = FxCanvas::new_alloc();
        canvas.set_name("FxCanvas");
        canvas.set_z_index(WORLD_Z);
        // Bursts and sounds pause with the game; the Fx node itself keeps running.
        canvas.set_process_mode(ProcessMode::PAUSABLE);
        self.base_mut().add_child(&canvas);

        let mut additive = CanvasItemMaterial::new_gd();
        additive.set_blend_mode(BlendMode::ADD);
        self.additive = Some(additive);

        let mut curve = Curve::new_gd();
        curve.add_point(Vector2::new(0.0, 1.0));
        curve.add_point(Vector2::new(1.0, 0.0));
        self.shrink_curve = Some(curve.clone());

        let mut ramp = Gradient::new_gd();
        ramp.set_color(0, Color::WHITE);
        ramp.set_color(1, Color::from_rgba(1.0, 1.0, 1.0, 0.0));
        self.fade_ramp = Some(ramp.clone());

        for _ in 0..BURST_POOL_SIZE {
            let mut p = CpuParticles2D::new_alloc();
            p.set_emitting(false);
            p.set_one_shot(true);
            p.set_explosiveness_ratio(1.0);
            p.set_randomness_ratio(0.5);
            p.set_lifetime_randomness(0.4);
            p.set_spread(180.0);
            p.set_direction(Vector2::RIGHT);
            p.set_gravity(Vector2::ZERO);
            p.set_emission_shape(godot::classes::cpu_particles_2d::EmissionShape::SPHERE);
            p.set_param_curve(Parameter::SCALE, &curve);
            p.set_color_ramp(&ramp);
            p.set_use_local_coordinates(false);
            canvas.clone().upcast::<Node>().add_child(&p);
            self.bursts.push(p);
        }
        self.canvas = Some(canvas);

        let mut layer = CanvasLayer::new_alloc();
        layer.set_name("FxOverlay");
        layer.set_layer(OVERLAY_LAYER);
        self.base_mut().add_child(&layer);

        let mut flash = ColorRect::new_alloc();
        flash.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        flash.set_mouse_filter(MouseFilter::IGNORE);
        flash.set_material(self.additive.as_ref());
        flash.set_visible(false);
        layer.add_child(&flash);
        self.flash_rect = Some(flash);

        let mut rewind = ColorRect::new_alloc();
        rewind.set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        rewind.set_mouse_filter(MouseFilter::IGNORE);
        if let Some(shader) = ResourceLoader::singleton()
            .load("res://shaders/rewind.gdshader")
            .and_then(|r| r.try_cast::<Shader>().ok())
        {
            let mut material = ShaderMaterial::new_gd();
            material.set_shader(&shader);
            rewind.set_material(&material);
        }
        rewind.set_visible(false);
        layer.add_child(&rewind);
        self.rewind_rect = Some(rewind);
    }
}

#[godot_api]
impl INode for Fx {
    fn ready(&mut self) {
        // Runs while paused so hit-stop can be cancelled and overlays finish fading.
        self.base_mut().set_process_mode(ProcessMode::ALWAYS);
        self.dummy_audio = AudioServer::singleton().get_driver_name() == "Dummy";
        self.build();
    }

    fn process(&mut self, _delta: f64) {
        let dt = self.real_delta();
        self.step_hitstop(dt);
        if !self.trauma.is_idle() || self.shaken_camera.is_some() {
            self.trauma.step(dt);
            self.apply_camera();
            if self.trauma.is_idle() {
                self.shaken_camera = None;
            }
        }
        self.flash_left = (self.flash_left - dt).max(0.0);
        self.rewind_left = (self.rewind_left - dt).max(0.0);
        self.apply_overlays();
    }

    fn exit_tree(&mut self) {
        if self.hitstop.is_active() {
            self.end_hitstop(true);
        }
        // Playbacks still running at quit would outlive their streams.
        for pool in self.sfx.values_mut() {
            for player in pool.players.iter_mut() {
                player.stop();
            }
        }
        self.sfx.clear();
    }
}

struct Ring {
    position: Vector2,
    color: Color,
    radius: f32,
    age: f32,
    duration: f32,
}

/// World-space drawing for `Fx`: rings and the checkpoint sweep in one `draw()`.
#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct FxCanvas {
    rings: Vec<Ring>,
    sweep_color: Color,
    sweep_age: f32,
    sweep_duration: f32,
    base: Base<Node2D>,
}

impl FxCanvas {
    fn add_ring(&mut self, position: Vector2, color: Color, radius: f32, seconds: f32) {
        const MAX_RINGS: usize = 64;
        if self.rings.len() >= MAX_RINGS {
            self.rings.remove(0);
        }
        self.rings.push(Ring {
            position,
            color,
            radius: radius.max(4.0),
            age: 0.0,
            duration: seconds.max(0.05),
        });
        self.base_mut().queue_redraw();
    }

    fn start_sweep(&mut self, color: Color, seconds: f32) {
        self.sweep_color = color;
        self.sweep_age = 0.0;
        self.sweep_duration = seconds.max(0.1);
        self.base_mut().queue_redraw();
    }

    fn clear(&mut self) {
        self.rings.clear();
        self.sweep_duration = 0.0;
        self.base_mut().queue_redraw();
    }

    fn draw_sweep(&mut self) {
        if self.sweep_duration <= 0.0 {
            return;
        }
        let Some(viewport) = self.base().get_viewport() else {
            return;
        };
        let rect = viewport.get_visible_rect().grow(80.0);
        let t = (self.sweep_age / self.sweep_duration).clamp(0.0, 1.0);
        // Ease-in-out so the line launches and lands softly.
        let eased = t * t * (3.0 - 2.0 * t);
        let x = rect.position.x + rect.size.x * eased;
        let top = rect.position.y;
        let bottom = rect.end().y;
        let c = self.sweep_color;
        let fade = (1.0 - t).powf(0.5);
        // Gradient wake behind the line: transparent at the tail, bright at the edge.
        let tail = x - SWEEP_TRAIL;
        let clear = Color::from_rgba(c.r, c.g, c.b, 0.0);
        let lit = Color::from_rgba(c.r, c.g, c.b, 0.45 * fade);
        let points = PackedVector2Array::from(&[
            Vector2::new(tail, top),
            Vector2::new(x, top),
            Vector2::new(x, bottom),
            Vector2::new(tail, bottom),
        ]);
        let colors = PackedColorArray::from(&[clear, lit, lit, clear]);
        self.base_mut().draw_polygon(&points, &colors);
        self.base_mut().draw_rect(
            Rect2::new(
                Vector2::new(x - 10.0, top),
                Vector2::new(14.0, bottom - top),
            ),
            Color::from_rgba(c.r, c.g, c.b, 0.5 * fade),
        );
        self.base_mut()
            .draw_line_ex(
                Vector2::new(x, top),
                Vector2::new(x, bottom),
                Color::from_rgba(1.0, 1.0, 1.0, 0.9 * fade),
            )
            .width(2.0)
            .done();
    }
}

#[godot_api]
impl INode2D for FxCanvas {
    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        if self.rings.is_empty() && self.sweep_duration <= 0.0 {
            return;
        }
        for ring in self.rings.iter_mut() {
            ring.age += dt;
        }
        self.rings.retain(|r| r.age < r.duration);
        if self.sweep_duration > 0.0 {
            self.sweep_age += dt;
            if self.sweep_age >= self.sweep_duration {
                self.sweep_duration = 0.0;
            }
        }
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let rings: Vec<(Vector2, Color, f32, f32)> = self
            .rings
            .iter()
            .map(|r| {
                let t = (r.age / r.duration).clamp(0.0, 1.0);
                // Ease-out expansion, linear fade.
                let grow = 1.0 - (1.0 - t).powi(3);
                (
                    r.position,
                    r.color,
                    r.radius * (0.45 + 0.55 * grow),
                    1.0 - t,
                )
            })
            .collect();
        for (position, color, radius, life) in rings {
            let width = 1.5 + 4.5 * life;
            self.base_mut()
                .draw_arc_ex(
                    position,
                    radius,
                    0.0,
                    TAU,
                    48,
                    Color::from_rgba(color.r, color.g, color.b, color.a * 0.25 * life),
                )
                .width(width * 2.0)
                .done();
            self.base_mut()
                .draw_arc_ex(
                    position,
                    radius,
                    0.0,
                    TAU,
                    48,
                    Color::from_rgba(color.r, color.g, color.b, color.a * life),
                )
                .width(width)
                .antialiased(true)
                .done();
        }
        self.draw_sweep();
    }
}

/// The `/root/Fx` autoload, if present.
pub fn instance() -> Option<Gd<Fx>> {
    let tree = Engine::singleton()
        .get_main_loop()?
        .try_cast::<SceneTree>()
        .ok()?;
    tree.get_root()?
        .get_node_or_null("Fx")?
        .try_cast::<Fx>()
        .ok()
}

/// Runs `f` against the Fx autoload; does nothing when it is missing (tests, tools).
pub fn with_fx(f: impl FnOnce(&mut Fx)) {
    if let Some(mut fx) = instance() {
        f(&mut fx.bind_mut());
    }
}

/// Plays a sound through Fx with default jitter at `position`.
pub fn sfx_at(name: &str, position: Vector2) {
    with_fx(|fx| {
        fx.play(name, position, 0.06, 0.0);
    });
}
