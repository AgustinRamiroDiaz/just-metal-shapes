//! `PlayerVisual`: presentation for a `Player` (first child in `player.tscn`, so it
//! draws under the body sprites).
//!
//! Reads the player's state each frame and owns everything cosmetic: squash/stretch
//! and tilt along the velocity, a beat-synced idle bounce, a team-color afterimage
//! trail at speed, a steady range ring, the hit flash/knockback ring,
//! the downed ghost with its revive zone and progress ring, and the revive pop. It
//! never changes gameplay state.

use crate::conductor::Conductor;
use crate::core::feel::{
    beat_envelope, decay, life_pip_alpha, name_tag_alpha, squash_stretch, stretch_basis,
};
use crate::fx::{BurstStyle, with_fx};
use crate::player::Player;
use crate::ui::{FontKind, font};
use godot::classes::{Font, INode2D, Node2D, ShaderMaterial, Sprite2D};
use godot::prelude::*;

const TAU: f32 = std::f32::consts::TAU;
/// Revive zone radius; matches `RevivalComponent`'s reach.
const REVIVE_RADIUS: f32 = 60.0;
const TRAIL_INTERVAL: f32 = 0.03;
const TRAIL_LIFETIME: f32 = 0.22;
/// Fraction of top speed above which the afterimage trail appears.
const TRAIL_SPEED_FRACTION: f32 = 0.6;
const RANGE_TIERS: i32 = 3;
/// Life pips sit in a row this far below the body's center.
const PIP_Y: f32 = 25.0;
const PIP_SPACING: f32 = 11.0;
const PIP_RADIUS: f32 = 4.0;
const NAME_Y: f32 = -30.0;
const NAME_SIZE: i32 = 15;

struct Ghost {
    position: Vector2,
    age: f32,
}

#[derive(GodotClass)]
#[class(init, base = Node2D)]
pub struct PlayerVisual {
    player: Option<Gd<Player>>,
    body: Option<Gd<Sprite2D>>,
    face: Option<Gd<Sprite2D>>,
    body_material: Option<Gd<ShaderMaterial>>,
    conductor: Option<Gd<Conductor>>,
    #[init(val = Vector2::new(0.325, 0.325))]
    sprite_scale: Vector2,

    /// `damaged` events not yet turned into effects (`lives_left` per hit).
    pending_hits: Vec<i32>,
    pending_revive: bool,
    was_dead: bool,

    #[init(val = 1.0)]
    along: f32,
    #[init(val = 1.0)]
    across: f32,
    move_angle: f32,
    tilt: f32,
    hit_flash: f32,
    /// Revive/respawn scale pop (decays to 0).
    pop: f32,
    beat: f32,
    time: f32,

    ghosts: Vec<Ghost>,
    ghost_timer: f32,

    /// Lives the pips last showed (-1 before the first frame).
    #[init(val = -1)]
    shown_lives: i32,
    since_lives_changed: f32,
    since_start: f32,
    /// Per-pip flash after losing that life, 1 -> 0.
    pip_flash: [f32; 3],
    display_name: String,
    name_font: Option<Gd<Font>>,

    base: Base<Node2D>,
}

#[godot_api]
impl PlayerVisual {
    #[func]
    fn _on_damaged(&mut self, lives_left: i32) {
        self.pending_hits.push(lives_left);
    }

    #[func]
    fn _on_revived(&mut self) {
        self.pending_revive = true;
    }

    /// 1 right after a hit, fading out (tests and debugging).
    #[func]
    pub fn get_hit_flash(&self) -> f32 {
        self.hit_flash
    }

    /// Current opacity of the life pips (tests).
    #[func]
    pub fn get_life_pip_alpha(&self) -> f32 {
        self.snapshot().map_or(0.0, |s| {
            life_pip_alpha(s.lives, Player::MAX_LIVES, self.since_lives_changed)
        })
    }

    /// Current opacity of the name tag (tests).
    #[func]
    pub fn get_name_alpha(&self) -> f32 {
        self.snapshot()
            .map_or(0.0, |s| name_tag_alpha(self.since_start, s.is_dead))
    }

    #[func]
    pub fn get_display_name(&self) -> GString {
        GString::from(&self.display_name)
    }
}

struct Snapshot {
    is_dead: bool,
    lives: i32,
    revival_progress: f32,
    color: Color,
    range_radius: f32,
    speed: f32,
    velocity: Vector2,
    position: Vector2,
}

impl PlayerVisual {
    fn snapshot(&self) -> Option<Snapshot> {
        let player = self.player.as_ref()?;
        let p = player.bind();
        Some(Snapshot {
            is_dead: p.is_dead,
            lives: p.lives,
            revival_progress: p.revival_progress,
            color: p.team_color,
            range_radius: p.range_radius,
            speed: p.speed.max(1.0),
            velocity: player.get_velocity(),
            position: player.get_global_position(),
        })
    }

    fn find_conductor(&mut self) {
        if self.conductor.is_some() {
            return;
        }
        self.conductor = self
            .base()
            .get_tree()
            .get_current_scene()
            .and_then(|scene| scene.try_get_node_as::<Conductor>("Conductor"));
    }

    fn beat_phase(&self) -> Option<f64> {
        let conductor = self.conductor.as_ref()?;
        let c = conductor.bind();
        c.is_playing().then(|| c.song_beat())
    }

    fn play_events(&mut self, s: &Snapshot) {
        let hits = std::mem::take(&mut self.pending_hits);
        for lives_left in hits {
            self.hit_flash = 1.0;
            let color = s.color;
            let pos = s.position;
            if lives_left <= 0 {
                with_fx(|fx| {
                    fx.burst_style(pos, color, 28, BurstStyle::Shards as i32, 1.2);
                    fx.burst_style(pos, Color::WHITE, 14, BurstStyle::Sparks as i32, 1.3);
                    fx.ring(pos, color, 110.0, 0.5);
                    fx.shake(0.55);
                    fx.hitstop(0.1);
                    fx.play_sfx("player_down".into(), pos, 0.04, 0.0);
                });
            } else {
                with_fx(|fx| {
                    fx.burst_style(pos, Color::WHITE, 12, BurstStyle::Sparks as i32, 1.0);
                    fx.ring(pos, Color::WHITE, 72.0, 0.32);
                    fx.ring(pos, color, 52.0, 0.4);
                    fx.shake(0.38);
                    fx.hitstop(0.06);
                    fx.play_sfx("player_hit".into(), pos, 0.06, 0.0);
                });
            }
        }

        let revived_now = self.was_dead && !s.is_dead;
        if self.pending_revive || revived_now {
            let color = s.color;
            let pos = s.position;
            let teammate = std::mem::take(&mut self.pending_revive);
            self.pop = 1.0;
            with_fx(|fx| {
                fx.burst_style(pos, color, 24, BurstStyle::Dots as i32, 1.4);
                fx.burst_style(pos, Color::WHITE, 10, BurstStyle::Sparks as i32, 0.8);
                fx.ring(pos, color, 90.0, 0.5);
                if teammate {
                    fx.play_sfx("player_revive".into(), pos, 0.03, 0.0);
                }
            });
        }
        self.was_dead = s.is_dead;
    }

    fn animate_sprites(&mut self, s: &Snapshot, dt: f32) {
        let speed = s.velocity.length();
        let (along, across) = if s.is_dead {
            (1.0, 1.0)
        } else {
            squash_stretch(speed, s.speed, 0.22)
        };
        let k = 1.0 - (-dt * 18.0).exp();
        self.along += (along - self.along) * k;
        self.across += (across - self.across) * k;
        if speed > 5.0 {
            self.move_angle = s.velocity.angle();
        }
        let target_tilt = if s.is_dead {
            0.0
        } else {
            (s.velocity.x / s.speed).clamp(-1.0, 1.0) * 0.16
        };
        self.tilt += (target_tilt - self.tilt) * (1.0 - (-dt * 12.0).exp());

        // Idle: bounce on the beat (vertical stretch + lift). Moving: velocity stretch.
        let idle = (1.0 - speed / (s.speed * 0.3)).clamp(0.0, 1.0);
        let bounce = self.beat * idle * if s.is_dead { 0.0 } else { 1.0 };
        let bounce_stretch = 1.0 + 0.08 * bounce;
        let lift = -3.0 * bounce;
        let float = if s.is_dead {
            (self.time * 2.2).sin() * 2.5
        } else {
            0.0
        };
        // Elastic settle: overshoots, then rings down as `pop` decays.
        let pop = 1.0 + 0.35 * self.pop * ((1.0 - self.pop) * 14.0).cos();
        let shrink = if s.is_dead { 0.85 } else { 1.0 };

        let ((xx, xy), (yx, yy)) =
            stretch_basis(self.move_angle, self.along, self.across, self.tilt);
        let ((bxx, bxy), (byx, byy)) = stretch_basis(
            std::f32::consts::FRAC_PI_2,
            bounce_stretch,
            1.0 / bounce_stretch,
            0.0,
        );
        // Combined basis = motion * bounce, then uniform scale.
        let scale = self.sprite_scale * pop * shrink;
        let a = Vector2::new(xx * bxx + yx * bxy, xy * bxx + yy * bxy) * scale.x;
        let b = Vector2::new(xx * byx + yx * byy, xy * byx + yy * byy) * scale.y;
        let transform = Transform2D::from_cols(a, b, Vector2::new(0.0, lift + float));

        let flash = self.hit_flash;
        let alpha = if s.is_dead { 0.4 } else { 1.0 };
        if let Some(body) = self.body.as_mut() {
            body.set_transform(transform);
            body.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, alpha));
        }
        if let Some(face) = self.face.as_mut() {
            face.set_transform(transform);
            face.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, alpha));
        }
        if let Some(material) = self.body_material.as_mut() {
            material.set_shader_parameter("flash", &flash.to_variant());
            let ghost = if s.is_dead { 1.0f32 } else { 0.0 };
            material.set_shader_parameter("ghost", &ghost.to_variant());
        }
    }

    fn update_trail(&mut self, s: &Snapshot, dt: f32) {
        for ghost in self.ghosts.iter_mut() {
            ghost.age += dt;
        }
        self.ghosts.retain(|g| g.age < TRAIL_LIFETIME);
        self.ghost_timer -= dt;
        let fast = !s.is_dead && s.velocity.length() > s.speed * TRAIL_SPEED_FRACTION;
        if fast && self.ghost_timer <= 0.0 {
            self.ghost_timer = TRAIL_INTERVAL;
            self.ghosts.push(Ghost {
                position: s.position,
                age: 0.0,
            });
        }
    }

    // Steady on purpose: with several players, rings flashing on every beat were noise.
    fn draw_range(&mut self, s: &Snapshot) {
        let mut fill = s.color;
        fill.a = 0.035;
        for tier in (0..RANGE_TIERS).rev() {
            let radius = s.range_radius * (tier + 1) as f32 / RANGE_TIERS as f32;
            self.base_mut().draw_circle(Vector2::ZERO, radius, fill);
        }
        let mut edge = s.color;
        edge.a = 0.24;
        let radius = s.range_radius;
        self.base_mut()
            .draw_arc_ex(Vector2::ZERO, radius, 0.0, TAU, 64, edge)
            .width(1.5)
            .antialiased(true)
            .done();
    }

    fn draw_trail(&mut self, s: &Snapshot) {
        let ghosts: Vec<(Vector2, f32)> = self
            .ghosts
            .iter()
            .map(|g| (g.position - s.position, g.age / TRAIL_LIFETIME))
            .collect();
        for (local, t) in ghosts {
            let mut c = s.color;
            c.a = 0.3 * (1.0 - t);
            self.base_mut()
                .draw_circle(local, 12.0 * (1.0 - 0.6 * t), c);
        }
    }

    fn track_lives(&mut self, s: &Snapshot, dt: f32) {
        self.since_start += dt;
        self.since_lives_changed += dt;
        for flash in &mut self.pip_flash {
            *flash = decay(*flash, dt, 0.15);
        }
        if s.lives == self.shown_lives {
            return;
        }
        if self.shown_lives >= 0 {
            self.since_lives_changed = 0.0;
            for i in s.lives.max(0)..self.shown_lives.min(Player::MAX_LIVES) {
                self.pip_flash[i as usize] = 1.0;
            }
        }
        self.shown_lives = s.lives;
    }

    /// One pip per life under the body; spent lives are hollow.
    fn draw_life_pips(&mut self, s: &Snapshot) {
        let alpha = life_pip_alpha(s.lives, Player::MAX_LIVES, self.since_lives_changed);
        let flashing = self.pip_flash.iter().any(|f| *f > 0.01);
        if alpha <= 0.01 && !flashing {
            return;
        }
        let count = Player::MAX_LIVES;
        for i in 0..count {
            let x = (i as f32 - (count - 1) as f32 * 0.5) * PIP_SPACING;
            let at = Vector2::new(x, PIP_Y);
            let flash = self.pip_flash[i as usize];
            if i < s.lives {
                self.base_mut().draw_circle(
                    at,
                    PIP_RADIUS + 1.2,
                    Color::from_rgba(0.02, 0.02, 0.06, 0.7 * alpha),
                );
                self.base_mut()
                    .draw_circle(at, PIP_RADIUS, s.color.with_alpha(alpha));
            } else {
                let mut ring = s.color;
                ring.a = 0.6 * alpha;
                self.base_mut()
                    .draw_arc_ex(at, PIP_RADIUS, 0.0, TAU, 12, ring)
                    .width(1.2)
                    .antialiased(true)
                    .done();
            }
            if flash > 0.01 {
                // The life just lost bursts outward.
                self.base_mut()
                    .draw_arc_ex(
                        at,
                        PIP_RADIUS + 8.0 * (1.0 - flash),
                        0.0,
                        TAU,
                        16,
                        Color::from_rgba(1.0, 1.0, 1.0, flash),
                    )
                    .width(2.0)
                    .antialiased(true)
                    .done();
            }
        }
    }

    fn draw_name(&mut self, s: &Snapshot) {
        let alpha = name_tag_alpha(self.since_start, s.is_dead);
        if alpha <= 0.01 || self.display_name.is_empty() {
            return;
        }
        let Some(name_font) = self.name_font.clone() else {
            return;
        };
        let text = self.display_name.clone();
        let width = name_font
            .get_string_size_ex(&text)
            .font_size(NAME_SIZE)
            .done()
            .x;
        let at = Vector2::new(-width * 0.5, NAME_Y);
        self.base_mut()
            .draw_string_outline_ex(&name_font, at, &text)
            .font_size(NAME_SIZE)
            .size(5)
            .modulate(Color::from_rgba(0.02, 0.02, 0.06, 0.85 * alpha))
            .done();
        self.base_mut()
            .draw_string_ex(&name_font, at, &text)
            .font_size(NAME_SIZE)
            .modulate(s.color.lightened(0.25).with_alpha(alpha))
            .done();
    }

    fn draw_downed(&mut self, s: &Snapshot) {
        let pulse = 0.5 + 0.5 * (self.time * 4.0).sin();
        let mut zone = s.color;
        zone.a = 0.06 + 0.06 * pulse;
        self.base_mut()
            .draw_circle(Vector2::ZERO, REVIVE_RADIUS, zone);

        // Dashed boundary, slowly turning: "come here".
        const DASHES: i32 = 16;
        let spin = self.time * 0.6;
        let mut dash = s.color;
        dash.a = 0.45 + 0.4 * pulse;
        for i in 0..DASHES {
            let start = spin + i as f32 * TAU / DASHES as f32;
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    REVIVE_RADIUS,
                    start,
                    start + TAU / DASHES as f32 * 0.55,
                    6,
                    dash,
                )
                .width(2.0)
                .antialiased(true)
                .done();
        }

        let progress = s.revival_progress.clamp(0.0, 1.0);
        if progress > 0.0 {
            let start = -std::f32::consts::FRAC_PI_2;
            let mut fill = s.color;
            fill.a = 0.18 * progress;
            self.base_mut()
                .draw_circle(Vector2::ZERO, REVIVE_RADIUS * progress, fill);
            self.base_mut()
                .draw_arc_ex(
                    Vector2::ZERO,
                    REVIVE_RADIUS - 6.0,
                    start,
                    start + TAU * progress,
                    48,
                    Color::WHITE,
                )
                .width(4.0)
                .antialiased(true)
                .done();
        }
    }
}

#[godot_api]
impl INode2D for PlayerVisual {
    fn ready(&mut self) {
        let Some(parent) = self.base().get_parent() else {
            return;
        };
        let Ok(player) = parent.try_cast::<Player>() else {
            godot_warn!("PlayerVisual: parent is not a Player");
            return;
        };
        self.body = player.try_get_node_as::<Sprite2D>("Sprite2D");
        self.face = player.try_get_node_as::<Sprite2D>("FaceSprite");
        if let Some(body) = &self.body {
            self.sprite_scale = body.get_scale();
            self.body_material = body
                .get_material()
                .and_then(|m| m.try_cast::<ShaderMaterial>().ok());
        }
        let this = self.to_gd();
        let mut player_node = player.clone();
        player_node.connect("damaged", &this.callable("_on_damaged"));
        player_node.connect("revived", &this.callable("_on_revived"));
        self.display_name = player
            .get_meta_ex("display_name")
            .default(&GString::new().to_variant())
            .done()
            .to_string();
        self.name_font = font(FontKind::Narrow);
        self.player = Some(player);
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        self.time += dt;
        self.find_conductor();
        let Some(s) = self.snapshot() else {
            return;
        };
        self.beat = self.beat_phase().map_or(0.0, |b| beat_envelope(b, 5.0));
        self.hit_flash = decay(self.hit_flash, dt, 0.06);
        self.pop = decay(self.pop, dt, 0.12);
        self.play_events(&s);
        self.track_lives(&s, dt);
        self.animate_sprites(&s, dt);
        self.update_trail(&s, dt);
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let Some(s) = self.snapshot() else {
            return;
        };
        if s.is_dead {
            self.draw_downed(&s);
        } else {
            self.draw_range(&s);
            self.draw_trail(&s);
            self.draw_life_pips(&s);
        }
        self.draw_name(&s);
    }
}
