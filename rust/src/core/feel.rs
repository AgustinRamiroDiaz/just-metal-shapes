//! Game-feel math: screen-shake trauma, hit-stop bookkeeping, squash and stretch, and
//! beat envelopes. Godot-free so the curves are unit tested.

/// Trauma lost per second; a full-strength shake settles in under a second.
pub const TRAUMA_DECAY: f32 = 1.6;
/// Camera offset in pixels at trauma 1.
pub const SHAKE_MAX_OFFSET: (f32, f32) = (14.0, 10.0);
/// Camera roll in radians at trauma 1.
pub const SHAKE_MAX_ROLL: f32 = 0.025;

/// Hit-stop never lasts longer than this, however many requests stack up.
pub const HITSTOP_MAX_SECONDS: f32 = 0.12;
/// After a hit-stop ends, new requests are ignored for this long, so a burst of hits
/// cannot keep the game frozen.
pub const HITSTOP_COOLDOWN_SECONDS: f32 = 0.15;
/// Time scale applied during a hit-stop, relative to the scale it interrupted.
pub const HITSTOP_SCALE: f32 = 0.05;

/// Screen-shake state driven by decaying trauma (0..1). Shake magnitude is
/// `trauma^2`, so small hits barely move the camera and big ones punch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Trauma {
    pub value: f32,
    /// Noise phase in seconds of shake time.
    pub phase: f32,
}

impl Trauma {
    /// Adds trauma, clamped to 1. Hits accumulate instead of resetting.
    pub fn add(&mut self, amount: f32) {
        self.value = (self.value + amount.max(0.0)).min(1.0);
    }

    /// Decays trauma by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.value = (self.value - TRAUMA_DECAY * dt.max(0.0)).max(0.0);
        self.phase += dt.max(0.0);
    }

    pub fn is_idle(&self) -> bool {
        self.value <= 0.0
    }

    /// Camera `(offset_x, offset_y, roll)` for the current trauma and phase, scaled by
    /// `strength` (the screen-shake setting, 0 disables).
    pub fn offset(&self, strength: f32) -> (f32, f32, f32) {
        let shake = self.value * self.value * strength.max(0.0);
        if shake <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        // Sums of incommensurate sines: smooth, non-repeating, no per-frame randomness.
        let t = self.phase * 32.0;
        let nx = 0.6 * (t * 1.7).sin() + 0.4 * (t * 3.1 + 1.3).sin();
        let ny = 0.6 * (t * 2.3 + 0.7).sin() + 0.4 * (t * 2.9 + 2.1).sin();
        let nr = (t * 1.1 + 0.4).sin();
        (
            SHAKE_MAX_OFFSET.0 * shake * nx,
            SHAKE_MAX_OFFSET.1 * shake * ny,
            SHAKE_MAX_ROLL * shake * nr,
        )
    }
}

/// Hit-stop timer in real (unscaled) seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitStop {
    /// Real seconds left in the current stop (0 = not stopped).
    pub remaining: f32,
    /// Real seconds left before another stop may start.
    pub cooldown: f32,
}

impl HitStop {
    /// Requests a stop of `seconds`. Returns true if the game should enter (or stay in)
    /// a stop. An active stop is extended to the longer request, never past
    /// `HITSTOP_MAX_SECONDS`; requests during the cooldown are dropped.
    pub fn request(&mut self, seconds: f32) -> bool {
        if seconds <= 0.0 {
            return self.is_active();
        }
        if !self.is_active() && self.cooldown > 0.0 {
            return false;
        }
        self.remaining = self.remaining.max(seconds.min(HITSTOP_MAX_SECONDS));
        true
    }

    /// Advances by `real_dt`. Returns true on the step the stop ends.
    pub fn step(&mut self, real_dt: f32) -> bool {
        let dt = real_dt.max(0.0);
        if self.remaining > 0.0 {
            self.remaining -= dt;
            if self.remaining <= 0.0 {
                self.remaining = 0.0;
                self.cooldown = HITSTOP_COOLDOWN_SECONDS;
                return true;
            }
        } else {
            self.cooldown = (self.cooldown - dt).max(0.0);
        }
        false
    }

    /// Ends the stop now (pause, scene change, external time-scale change).
    pub fn cancel(&mut self) {
        if self.remaining > 0.0 {
            self.remaining = 0.0;
            self.cooldown = HITSTOP_COOLDOWN_SECONDS;
        }
    }

    pub fn is_active(&self) -> bool {
        self.remaining > 0.0
    }
}

/// Area-preserving squash and stretch for a body moving at `speed` out of `max_speed`:
/// returns `(along, across)` scale factors with `along * across == 1`.
/// `amount` is the stretch at full speed (0.2 = 20% longer).
pub fn squash_stretch(speed: f32, max_speed: f32, amount: f32) -> (f32, f32) {
    if max_speed <= 0.0 {
        return (1.0, 1.0);
    }
    let t = (speed / max_speed).clamp(0.0, 1.0);
    // Ease-out so the stretch shows early in a move and saturates at top speed.
    let eased = 1.0 - (1.0 - t) * (1.0 - t);
    let along = 1.0 + amount.max(0.0) * eased;
    (along, 1.0 / along)
}

/// 2x2 basis (columns `x`, `y`) that scales by `along` on the axis at `angle` and by
/// `across` perpendicular to it, then rotates by `tilt`. Column-major, matching Godot's
/// `Transform2D` basis vectors: `((x.x, x.y), (y.x, y.y))`.
pub fn stretch_basis(angle: f32, along: f32, across: f32, tilt: f32) -> ((f32, f32), (f32, f32)) {
    let (s, c) = angle.sin_cos();
    // R(angle) * diag(along, across) * R(-angle)
    let a = c * c * along + s * s * across;
    let b = c * s * (along - across);
    let d = s * s * along + c * c * across;
    // M = [[a, b], [b, d]], then T(tilt) * M.
    let (ts, tc) = tilt.sin_cos();
    let x = (tc * a - ts * b, ts * a + tc * b);
    let y = (tc * b - ts * d, ts * b + tc * d);
    (x, y)
}

/// Beat envelope: 1 on the beat, decaying exponentially through it. `phase` is the
/// fractional position inside the beat (0..1); `sharpness` sets how quickly it falls
/// (about 5 leaves ~1% by the next beat).
pub fn beat_envelope(phase: f64, sharpness: f64) -> f32 {
    let p = phase.rem_euclid(1.0);
    (-p * sharpness.max(0.0)).exp() as f32
}

/// Exponential decay of `value` toward 0 with half-life `half_life` seconds.
pub fn decay(value: f32, dt: f32, half_life: f32) -> f32 {
    if half_life <= 0.0 {
        return 0.0;
    }
    value * 0.5f32.powf(dt.max(0.0) / half_life)
}

/// Arena energy (0..1) for a section type: brightness, grid speed and pulse size scale
/// with it. Intro and breakdown sit low, main sections high.
pub fn section_energy(section_type: &str, intensity: f32) -> f32 {
    let base = match section_type {
        "intro" => 0.25,
        "build" => 0.5,
        "main" => 0.8,
        "breakdown" => 0.35,
        "outro" => 0.3,
        _ => 0.5,
    };
    (base * 0.7 + intensity.clamp(0.0, 1.0) * 0.3).clamp(0.0, 1.0)
}

/// Seconds the life pips and name tag stay fully visible after a change.
pub const INDICATOR_HOLD: f32 = 2.0;
/// Seconds they take to fade to their resting opacity afterwards.
pub const INDICATOR_FADE: f32 = 0.6;
/// Resting opacity of the life pips while a player is missing lives.
pub const DAMAGED_PIP_ALPHA: f32 = 0.45;

/// Fades from 1 (during `INDICATOR_HOLD`) down to `rest` over `INDICATOR_FADE`.
fn held_then_rest(seconds_since: f32, rest: f32) -> f32 {
    let t = ((seconds_since - INDICATOR_HOLD) / INDICATOR_FADE).clamp(0.0, 1.0);
    1.0 + (rest - 1.0) * t
}

/// Opacity of a player's life pips: solid right after their lives change, then faint
/// while hurt and gone at full health.
pub fn life_pip_alpha(lives: i32, max_lives: i32, seconds_since_change: f32) -> f32 {
    let rest = if lives >= max_lives {
        0.0
    } else {
        DAMAGED_PIP_ALPHA
    };
    held_then_rest(seconds_since_change, rest)
}

/// Opacity of a player's name tag: shown at the level start and while downed.
pub fn name_tag_alpha(seconds_since_start: f32, downed: bool) -> f32 {
    if downed {
        1.0
    } else {
        held_then_rest(seconds_since_start, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn trauma_clamps_and_accumulates() {
        let mut t = Trauma::default();
        t.add(0.4);
        t.add(0.4);
        assert!(approx(t.value, 0.8));
        t.add(5.0);
        assert!(approx(t.value, 1.0));
        t.add(-1.0);
        assert!(approx(t.value, 1.0));
    }

    #[test]
    fn trauma_decays_to_zero_and_stays() {
        let mut t = Trauma::default();
        t.add(1.0);
        let mut steps = 0;
        while !t.is_idle() {
            t.step(1.0 / 60.0);
            steps += 1;
            assert!(steps < 120, "trauma should settle within two seconds");
        }
        assert_eq!(t.offset(1.0), (0.0, 0.0, 0.0));
        t.step(1.0);
        assert!(t.is_idle());
    }

    #[test]
    fn shake_is_quadratic_and_bounded() {
        let mut small = Trauma::default();
        small.add(0.3);
        let mut big = Trauma::default();
        big.add(1.0);
        let mut max_small = 0.0f32;
        let mut max_big = 0.0f32;
        for i in 0..400 {
            small.phase = i as f32 * 0.01;
            big.phase = small.phase;
            let (sx, sy, sr) = small.offset(1.0);
            let (bx, by, br) = big.offset(1.0);
            assert!(bx.abs() <= SHAKE_MAX_OFFSET.0 && by.abs() <= SHAKE_MAX_OFFSET.1);
            assert!(br.abs() <= SHAKE_MAX_ROLL);
            max_small = max_small.max(sx.hypot(sy));
            max_big = max_big.max(bx.hypot(by));
            let _ = sr;
        }
        assert!(max_small < max_big * 0.15, "0.3 trauma shakes ~9% of full");
    }

    #[test]
    fn shake_setting_off_disables_offset() {
        let mut t = Trauma::default();
        t.add(1.0);
        t.phase = 0.37;
        assert_eq!(t.offset(0.0), (0.0, 0.0, 0.0));
    }

    #[test]
    fn hitstop_caps_duration_and_ends() {
        let mut h = HitStop::default();
        assert!(h.request(1.0));
        assert!(approx(h.remaining, HITSTOP_MAX_SECONDS));
        assert!(h.request(0.01), "stacked request keeps the stop");
        assert!(approx(h.remaining, HITSTOP_MAX_SECONDS));
        let mut ended = false;
        for _ in 0..20 {
            ended |= h.step(0.01);
        }
        assert!(ended);
        assert!(!h.is_active());
    }

    #[test]
    fn hitstop_cooldown_prevents_chaining() {
        let mut h = HitStop::default();
        h.request(0.05);
        while !h.step(0.01) {}
        assert!(!h.request(0.05), "dropped during cooldown");
        for _ in 0..20 {
            h.step(0.01);
        }
        assert!(h.request(0.05), "allowed after cooldown");
    }

    #[test]
    fn hitstop_cancel() {
        let mut h = HitStop::default();
        h.request(0.1);
        h.cancel();
        assert!(!h.is_active());
        assert!(h.cooldown > 0.0);
    }

    #[test]
    fn squash_preserves_area() {
        for speed in [0.0, 50.0, 110.0, 220.0, 500.0] {
            let (along, across) = squash_stretch(speed, 220.0, 0.25);
            assert!(approx(along * across, 1.0));
            assert!((1.0..=1.25 + 1e-4).contains(&along));
        }
        assert_eq!(squash_stretch(0.0, 220.0, 0.25), (1.0, 1.0));
        assert_eq!(squash_stretch(10.0, 0.0, 0.25), (1.0, 1.0));
    }

    #[test]
    fn stretch_basis_axis_aligned() {
        let ((xx, xy), (yx, yy)) = stretch_basis(0.0, 1.2, 0.8, 0.0);
        assert!(approx(xx, 1.2) && approx(xy, 0.0) && approx(yx, 0.0) && approx(yy, 0.8));
        // Moving vertically stretches y.
        let ((xx, _), (_, yy)) = stretch_basis(std::f32::consts::FRAC_PI_2, 1.2, 0.8, 0.0);
        assert!(approx(xx, 0.8) && approx(yy, 1.2));
    }

    #[test]
    fn stretch_basis_preserves_determinant() {
        let ((xx, xy), (yx, yy)) = stretch_basis(0.7, 1.25, 0.8, 0.2);
        assert!(approx(xx * yy - xy * yx, 1.0));
    }

    #[test]
    fn beat_envelope_peaks_on_beat() {
        assert!(approx(beat_envelope(0.0, 5.0), 1.0));
        assert!(beat_envelope(0.5, 5.0) < 0.1);
        assert!(approx(beat_envelope(3.0, 5.0), 1.0));
        assert!(beat_envelope(0.25, 5.0) > beat_envelope(0.5, 5.0));
    }

    #[test]
    fn decay_halves() {
        assert!(approx(decay(1.0, 0.1, 0.1), 0.5));
        assert_eq!(decay(1.0, 0.1, 0.0), 0.0);
    }

    #[test]
    fn section_energy_orders_sections() {
        assert!(section_energy("intro", 0.5) < section_energy("build", 0.5));
        assert!(section_energy("build", 0.5) < section_energy("main", 0.5));
        assert!(section_energy("breakdown", 0.5) < section_energy("main", 0.5));
        for kind in ["intro", "build", "main", "breakdown", "outro", "x"] {
            let e = section_energy(kind, 2.0);
            assert!((0.0..=1.0).contains(&e));
        }
    }

    #[test]
    fn life_pips_fade_to_nothing_at_full_health() {
        assert_eq!(life_pip_alpha(3, 3, 0.0), 1.0);
        assert_eq!(life_pip_alpha(3, 3, INDICATOR_HOLD), 1.0);
        let mid = life_pip_alpha(3, 3, INDICATOR_HOLD + INDICATOR_FADE * 0.5);
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(life_pip_alpha(3, 3, 10.0), 0.0);
    }

    #[test]
    fn life_pips_stay_faint_while_hurt() {
        assert_eq!(life_pip_alpha(2, 3, 0.5), 1.0);
        assert_eq!(life_pip_alpha(1, 3, 10.0), DAMAGED_PIP_ALPHA);
    }

    #[test]
    fn name_tags_show_at_start_and_while_down() {
        assert_eq!(name_tag_alpha(0.0, false), 1.0);
        assert_eq!(name_tag_alpha(10.0, false), 0.0);
        assert_eq!(name_tag_alpha(10.0, true), 1.0);
    }
}
