//! Deterministic analysis -> chart mapping.
//!
//! Rules (see `docs/design/complete-game-plan.md`, "Chart"):
//! - The section type picks the pattern family and base hazard rate: intro sparse,
//!   build rising through the section, main full, breakdown few hazards plus an enemy
//!   phase, outro winding down.
//! - Bar intensity scales the per-beat hazard probability; strong accents always try to
//!   place a hit; strong off-beat onsets add half-beat hits from difficulty 3 up.
//! - A novelty spike on a downbeat (or every `PHRASE_BARS` bars) starts a new phrase,
//!   which re-picks the phrase's preferred hazard kind.
//! - Silent beats get no hazard, enemy, `ArenaPulse`, `CameraKick`, `Flash` or
//!   `PaletteShift`. `Checkpoint` and `ShowHint` are structural and are always placed.
//! - Every non-silent downbeat gets an `ArenaPulse`; strong accents add a `CameraKick`.
//! - Each section start gets a `Checkpoint` (variant = section index).
//! - Tutorial levels show `HINTS` during the opening bars.
//! - Safe path: the summed `coverage` of simultaneously active hazards never exceeds
//!   `coverage_cap(difficulty)` (< 1, so no pair can cover the whole arena), and at
//!   most `max_active_hazards(difficulty)` hazards are active at once.
//!
//! Which hazards exist, how often each is picked and in which sections is entirely
//! data: `LevelSpec::pattern_pool`.

use super::analysis::{BeatInfo, SectionType, SongAnalysis};
use super::chart::{Chart, ChartEvent, EventKind, HINTS, PatternParams};
use super::rng::Rng;
use super::timing::Timing;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub bg: Rgb,
    pub accent: Rgb,
}

/// One hazard kind a level may use, with its tuning. Tune hazards here (or in
/// `level_catalog.rs`) rather than in the generator.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternEntry {
    pub kind: EventKind,
    /// Relative pick weight among entries allowed in the current section.
    pub weight: f32,
    /// Sections this entry may appear in; empty means all.
    pub sections: Vec<SectionType>,
    pub telegraph_beats: f64,
    pub duration_beats: f64,
    /// Fraction of the arena area that is dangerous while the hazard is active. Feeds
    /// the safe-path cap, so overestimate rather than underestimate.
    pub coverage: f32,
    /// Copied to `PatternParams::size` (fraction of arena height).
    pub size: f32,
    /// Copied to `PatternParams::speed` (arena heights per second), scaled up slightly
    /// with difficulty.
    pub speed: f32,
    /// Inclusive range for `PatternParams::count`; the upper part unlocks with difficulty.
    pub count: (u32, u32),
    /// Only used on beats whose accent is at least this (0 = any beat).
    pub min_accent: f32,
}

impl PatternEntry {
    /// Entry with per-kind defaults.
    pub fn new(kind: EventKind, weight: f32) -> Self {
        // (telegraph, duration, coverage, size, speed, count)
        let (telegraph_beats, duration_beats, coverage, size, speed, count) = match kind {
            EventKind::Laser => (2.0, 1.0, 0.08, 0.06, 0.0, (1, 1)),
            EventKind::LaserSweep => (2.0, 4.0, 0.10, 0.05, 0.25, (1, 1)),
            EventKind::BulletRing => (1.0, 6.0, 0.06, 0.02, 0.35, (10, 16)),
            EventKind::Spiral => (1.0, 8.0, 0.08, 0.02, 0.30, (3, 5)),
            EventKind::Wall => (2.0, 4.0, 0.20, 0.15, 0.25, (1, 1)),
            EventKind::Pulse => (2.0, 1.0, 0.05, 0.14, 0.0, (1, 1)),
            EventKind::Bomb => (3.0, 1.0, 0.12, 0.25, 0.0, (1, 1)),
            EventKind::Spikes => (2.0, 2.0, 0.10, 0.08, 0.0, (4, 8)),
            EventKind::Barrage => (1.0, 4.0, 0.10, 0.02, 0.50, (6, 10)),
            _ => (0.0, 0.0, 0.0, 0.0, 0.0, (1, 1)),
        };
        Self {
            kind,
            weight,
            sections: Vec::new(),
            telegraph_beats,
            duration_beats,
            coverage,
            size,
            speed,
            count,
            min_accent: 0.0,
        }
    }

    pub fn in_sections(mut self, sections: &[SectionType]) -> Self {
        self.sections = sections.to_vec();
        self
    }

    pub fn telegraph(mut self, beats: f64) -> Self {
        self.telegraph_beats = beats;
        self
    }

    pub fn duration(mut self, beats: f64) -> Self {
        self.duration_beats = beats;
        self
    }

    pub fn coverage(mut self, coverage: f32) -> Self {
        self.coverage = coverage;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn count(mut self, low: u32, high: u32) -> Self {
        self.count = (low, high.max(low));
        self
    }

    pub fn min_accent(mut self, accent: f32) -> Self {
        self.min_accent = accent;
        self
    }

    pub fn allowed_in(&self, section: SectionType) -> bool {
        self.sections.is_empty() || self.sections.contains(&section)
    }
}

/// An enemy scene the chart may spawn.
#[derive(Clone, Debug, PartialEq)]
pub struct EnemyEntry {
    pub scene: String,
    /// Spawn just outside the arena edge (chasers) instead of inside with a spawn effect.
    pub spawn_outside: bool,
    pub weight: f32,
}

impl EnemyEntry {
    pub fn new(scene: &str, spawn_outside: bool, weight: f32) -> Self {
        Self {
            scene: scene.to_string(),
            spawn_outside,
            weight,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LevelSpec {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub music_path: String,
    pub analysis_path: String,
    /// 1 (gentlest) to 5.
    pub difficulty: u8,
    pub seed: u64,
    pub palette: Palette,
    pub pattern_pool: Vec<PatternEntry>,
    pub enemy_pool: Vec<EnemyEntry>,
    /// Hazard density multiplier on top of the difficulty curve (1.0 = default).
    pub density: f32,
    /// Show `ShowHint` events during the opening bars.
    pub tutorial: bool,
}

impl LevelSpec {
    pub fn pattern(&self, kind: EventKind) -> Option<&PatternEntry> {
        self.pattern_pool.iter().find(|entry| entry.kind == kind)
    }
}

/// Max summed `coverage` of simultaneously active hazards.
pub fn coverage_cap(difficulty: u8) -> f32 {
    0.30 + 0.05 * (difficulty.clamp(1, 5) - 1) as f32
}

/// Max number of simultaneously active hazards.
pub fn max_active_hazards(difficulty: u8) -> usize {
    2 + difficulty.clamp(1, 5) as usize
}

/// Bars per phrase before the preferred hazard kind is re-picked.
pub const PHRASE_BARS: i64 = 4;
/// No hazards hit before this beat, so players can orient themselves.
pub const LEAD_IN_BEATS: f64 = 4.0;
/// Hazards must finish this long before the song ends.
pub const END_MARGIN_SECONDS: f64 = 0.5;
/// Warning time for enemy spawns (the spawn effect plays for this long).
pub const ENEMY_TELEGRAPH_BEATS: f64 = 2.0;
pub const HINT_DURATION_BEATS: f64 = 6.0;
pub const HINT_SPACING_BEATS: f64 = 8.0;

const STRONG_ACCENT: f32 = 0.75;
const KICK_ACCENT: f32 = 0.85;
const KICK_ONSET: f32 = 0.6;
const PHRASE_NOVELTY: f32 = 0.85;
const OFFBEAT_ONSET: f32 = 0.8;
const PHRASE_KIND_BIAS: f64 = 0.6;

struct ActiveHazard {
    start: f64,
    end: f64,
    coverage: f32,
}

struct Generator<'a> {
    analysis: &'a SongAnalysis,
    spec: &'a LevelSpec,
    timing: Timing,
    rng: Rng,
    difficulty: u8,
    density: f32,
    cap: f32,
    max_active: usize,
    events: Vec<ChartEvent>,
    active: Vec<ActiveHazard>,
    phrase_kind: Option<EventKind>,
    phrase_start_bar: i64,
}

/// Builds the chart for a level. Same inputs and seed always give the same chart.
pub fn generate_chart(analysis: &SongAnalysis, spec: &LevelSpec, seed: u64) -> Chart {
    let difficulty = spec.difficulty.clamp(1, 5);
    let difficulty_scale = 0.55 + 0.225 * (difficulty - 1) as f32;
    let mut generator = Generator {
        analysis,
        spec,
        timing: analysis.timing(),
        rng: Rng::new(seed),
        difficulty,
        density: difficulty_scale * spec.density.max(0.0),
        cap: coverage_cap(difficulty),
        max_active: max_active_hazards(difficulty),
        events: Vec::new(),
        active: Vec::new(),
        phrase_kind: None,
        phrase_start_bar: i64::MIN,
    };
    generator.run();
    let mut events = generator.events;
    events.sort_by(|a, b| a.beat.total_cmp(&b.beat).then(a.kind.cmp(&b.kind)));
    Chart {
        bpm: analysis.bpm,
        offset_seconds: analysis.beat_offset_seconds,
        duration_seconds: analysis.duration_seconds,
        events,
    }
}

impl Generator<'_> {
    fn run(&mut self) {
        self.place_sections();
        if self.spec.tutorial {
            self.place_hints();
        }
        let mut hazard_rng = self.rng.fork(1);
        let mut enemy_rng = self.rng.fork(2);
        for beat_index in 0..self.analysis.beats.len() {
            let beat = self.analysis.beats[beat_index].clone();
            if beat.silent {
                continue;
            }
            self.place_presentation(&beat);
            self.place_enemy(&beat, &mut enemy_rng);
            self.place_hazards(&beat, &mut hazard_rng);
        }
    }

    fn push(&mut self, beat: f64, telegraph_beats: f64, kind: EventKind, params: PatternParams) {
        self.events.push(ChartEvent {
            beat,
            telegraph_beats,
            kind,
            params,
        });
    }

    fn beat_is_silent(&self, beat: i64) -> bool {
        self.analysis
            .beats
            .iter()
            .find(|info| info.beat == beat)
            .is_none_or(|info| info.silent)
    }

    fn place_sections(&mut self) {
        let sections = self.analysis.sections.clone();
        for (index, section) in sections.iter().enumerate() {
            let beat = section.start_beat as f64;
            self.push(
                beat,
                0.0,
                EventKind::Checkpoint,
                PatternParams {
                    variant: index as u32,
                    intensity: section.intensity,
                    ..PatternParams::default()
                },
            );
            if self.beat_is_silent(section.start_beat) {
                continue;
            }
            if index > 0 {
                self.push(
                    beat,
                    0.0,
                    EventKind::PaletteShift,
                    PatternParams {
                        variant: section_type_index(section.section_type),
                        intensity: section.intensity,
                        ..PatternParams::default()
                    },
                );
            }
            if matches!(section.section_type, SectionType::Main | SectionType::Build) {
                self.push(
                    beat,
                    0.0,
                    EventKind::Flash,
                    PatternParams {
                        duration_beats: 1.0,
                        intensity: section.intensity,
                        ..PatternParams::default()
                    },
                );
            }
        }
    }

    fn place_hints(&mut self) {
        let start = self.analysis.sections[0].start_beat as f64;
        for (index, _) in HINTS.iter().enumerate() {
            let beat = start + 1.0 + index as f64 * HINT_SPACING_BEATS;
            if self.timing.beat_to_seconds(beat + HINT_DURATION_BEATS)
                > self.analysis.duration_seconds
            {
                break;
            }
            self.push(
                beat,
                0.0,
                EventKind::ShowHint,
                PatternParams {
                    variant: index as u32,
                    duration_beats: HINT_DURATION_BEATS,
                    ..PatternParams::default()
                },
            );
        }
    }

    fn place_presentation(&mut self, beat: &BeatInfo) {
        let bar_intensity = self
            .analysis
            .bar_for_beat(beat.beat)
            .map_or(beat.loudness, |bar| bar.intensity);
        if self.timing.is_downbeat(beat.beat) {
            self.push(
                beat.beat as f64,
                0.0,
                EventKind::ArenaPulse,
                PatternParams {
                    intensity: bar_intensity,
                    ..PatternParams::default()
                },
            );
        }
        if beat.accent >= KICK_ACCENT && beat.onset_strength >= KICK_ONSET {
            self.push(
                beat.beat as f64,
                0.0,
                EventKind::CameraKick,
                PatternParams {
                    intensity: beat.accent,
                    ..PatternParams::default()
                },
            );
        }
    }

    fn place_enemy(&mut self, beat: &BeatInfo, rng: &mut Rng) {
        if self.spec.enemy_pool.is_empty() || !self.timing.is_downbeat(beat.beat) {
            return;
        }
        let section_index = self.analysis.section_index_for_beat(beat.beat);
        let section = &self.analysis.sections[section_index];
        let (rate, forced_bar) = match section.section_type {
            SectionType::Main => (0.12, 1),
            SectionType::Breakdown => (0.3, 1),
            _ => return,
        };
        let bar_in_section = (beat.beat - section.start_beat) / self.timing.beats_per_bar as i64;
        let section_bars = (section.end_bar - section.start_bar).max(1);
        let limit = match section.section_type {
            SectionType::Breakdown => (section_bars / 3).max(1),
            _ => (section_bars / 6).max(1),
        } as usize;
        let section_start = section.start_beat as f64;
        let section_end = section.end_beat as f64;
        let spawned = self
            .events
            .iter()
            .filter(|event| {
                event.kind == EventKind::SpawnEnemy
                    && event.beat >= section_start
                    && event.beat < section_end
            })
            .count();
        if spawned >= limit {
            return;
        }
        let difficulty_scale = 0.6 + 0.2 * (self.difficulty - 1) as f64;
        let forced = bar_in_section >= forced_bar && spawned == 0;
        if !forced && !rng.chance(rate * difficulty_scale) {
            return;
        }
        if beat.beat as f64 - ENEMY_TELEGRAPH_BEATS < 0.0 {
            return;
        }
        let weights: Vec<f32> = self.spec.enemy_pool.iter().map(|e| e.weight).collect();
        let Some(variant) = rng.weighted_index(&weights) else {
            return;
        };
        let params = PatternParams {
            x: rng.range_f32(0.15, 0.85),
            y: rng.range_f32(0.15, 0.85),
            angle: rng.range_f32(0.0, std::f32::consts::TAU),
            variant: variant as u32,
            intensity: section.intensity,
            ..PatternParams::default()
        };
        self.push(
            beat.beat as f64,
            ENEMY_TELEGRAPH_BEATS,
            EventKind::SpawnEnemy,
            params,
        );
    }

    fn base_rate(&self, section_type: SectionType, progress: f32) -> f32 {
        match section_type {
            SectionType::Intro if self.spec.tutorial => 0.05,
            SectionType::Intro => 0.10,
            SectionType::Build => 0.12 + 0.28 * progress,
            SectionType::Main => 0.38,
            SectionType::Breakdown => 0.07,
            SectionType::Outro => 0.22 * (1.0 - progress),
        }
    }

    fn place_hazards(&mut self, beat: &BeatInfo, rng: &mut Rng) {
        let section_index = self.analysis.section_index_for_beat(beat.beat);
        let section = self.analysis.sections[section_index].clone();
        let section_len = (section.end_beat - section.start_beat).max(1) as f32;
        let progress = ((beat.beat - section.start_beat) as f32 / section_len).clamp(0.0, 1.0);
        let bar_intensity = self
            .analysis
            .bar_for_beat(beat.beat)
            .map_or(beat.loudness, |bar| bar.intensity);

        self.update_phrase(beat, section.section_type, rng);

        let probability = (self.base_rate(section.section_type, progress)
            * (0.4 + 0.6 * bar_intensity)
            * self.density) as f64;
        let strong = beat.accent >= STRONG_ACCENT;
        let mut hits = 0;
        if strong || rng.chance(probability) {
            hits = 1;
            if strong && self.difficulty >= 4 && beat.accent >= KICK_ACCENT {
                hits = 2;
            }
        }
        for _ in 0..hits {
            self.try_place_hazard(beat.beat as f64, beat, section.section_type, rng);
        }

        let offbeat = beat.beat as f64 + 0.5;
        if self.difficulty >= 3
            && matches!(section.section_type, SectionType::Main | SectionType::Build)
            && self.analysis.onsets.iter().any(|onset| {
                (onset.quantized_beat - offbeat).abs() < 1e-6 && onset.strength >= OFFBEAT_ONSET
            })
            && rng.chance((0.5 * self.density) as f64)
        {
            self.try_place_hazard(offbeat, beat, section.section_type, rng);
        }
    }

    fn update_phrase(&mut self, beat: &BeatInfo, section_type: SectionType, rng: &mut Rng) {
        if !self.timing.is_downbeat(beat.beat) && self.phrase_kind.is_some() {
            return;
        }
        let bar = self.timing.bar_of_beat(beat.beat as f64);
        let section_start = self
            .analysis
            .sections
            .iter()
            .any(|section| section.start_beat == beat.beat);
        let novelty_spike = self
            .analysis
            .bar_for_beat(beat.beat)
            .is_some_and(|bar| bar.novelty >= PHRASE_NOVELTY)
            || beat.novelty >= PHRASE_NOVELTY;
        if self.phrase_kind.is_none()
            || section_start
            || novelty_spike
            || bar - self.phrase_start_bar >= PHRASE_BARS
        {
            self.phrase_kind = self.pick_kind(section_type, 1.0, f32::INFINITY, rng);
            self.phrase_start_bar = bar;
        }
    }

    /// Weighted pick among pool entries allowed in this section, by accent, and whose
    /// coverage fits in `coverage_room`.
    fn pick_kind(
        &self,
        section_type: SectionType,
        accent: f32,
        coverage_room: f32,
        rng: &mut Rng,
    ) -> Option<EventKind> {
        let candidates: Vec<&PatternEntry> = self
            .spec
            .pattern_pool
            .iter()
            .filter(|entry| {
                entry.kind.is_hazard()
                    && entry.allowed_in(section_type)
                    && accent >= entry.min_accent
                    && entry.coverage <= coverage_room
                    && entry.coverage <= self.cap
            })
            .collect();
        let weights: Vec<f32> = candidates.iter().map(|entry| entry.weight).collect();
        rng.weighted_index(&weights)
            .map(|index| candidates[index].kind)
    }

    fn try_place_hazard(
        &mut self,
        hit_beat: f64,
        beat: &BeatInfo,
        section_type: SectionType,
        rng: &mut Rng,
    ) {
        if hit_beat < LEAD_IN_BEATS {
            return;
        }
        self.active.retain(|hazard| hazard.end > hit_beat);
        let used: f32 = self
            .active
            .iter()
            .filter(|hazard| hazard.start <= hit_beat)
            .map(|hazard| hazard.coverage)
            .sum();
        if self.active.len() >= self.max_active {
            return;
        }
        let room = self.cap - used;

        let phrase_entry = self
            .phrase_kind
            .and_then(|kind| self.spec.pattern(kind))
            .filter(|entry| {
                entry.allowed_in(section_type)
                    && beat.accent >= entry.min_accent
                    && entry.coverage <= room
            });
        let kind = match phrase_entry {
            Some(entry) if rng.chance(PHRASE_KIND_BIAS) => Some(entry.kind),
            _ => self.pick_kind(section_type, beat.accent, room, rng),
        };
        let Some(entry) = kind.and_then(|kind| self.spec.pattern(kind)).cloned() else {
            return;
        };
        if hit_beat < entry.telegraph_beats {
            return;
        }
        let end_beat = hit_beat + entry.duration_beats;
        if self.timing.beat_to_seconds(end_beat)
            > self.analysis.duration_seconds - END_MARGIN_SECONDS
        {
            return;
        }
        // Coverage only counts while the hazard is active (telegraphs are harmless).
        let overlapping: f32 = self
            .active
            .iter()
            .filter(|hazard| hazard.start < end_beat && hazard.end > hit_beat)
            .map(|hazard| hazard.coverage)
            .sum();
        if overlapping + entry.coverage > self.cap + 1e-6 {
            return;
        }

        let params = self.hazard_params(&entry, beat, rng);
        self.active.push(ActiveHazard {
            start: hit_beat,
            end: end_beat,
            coverage: entry.coverage,
        });
        self.push(hit_beat, entry.telegraph_beats, entry.kind, params);
    }

    fn hazard_params(&self, entry: &PatternEntry, beat: &BeatInfo, rng: &mut Rng) -> PatternParams {
        let difficulty_t = (self.difficulty - 1) as f32 / 4.0;
        let (low, high) = entry.count;
        let unlocked_high = low + ((high - low) as f32 * (0.4 + 0.6 * difficulty_t)).round() as u32;
        // Snap angles to 8 directions half the time so patterns read as deliberate.
        let angle = if rng.chance(0.5) {
            rng.range_u32(0, 7) as f32 * std::f32::consts::FRAC_PI_4
        } else {
            rng.range_f32(0.0, std::f32::consts::TAU)
        };
        PatternParams {
            x: rng.range_f32(0.1, 0.9),
            y: rng.range_f32(0.1, 0.9),
            angle,
            count: rng.range_u32(low, unlocked_high),
            speed: entry.speed * (0.85 + 0.3 * difficulty_t),
            size: entry.size,
            duration_beats: entry.duration_beats,
            color_index: rng.range_u32(0, 3),
            variant: rng.range_u32(0, 3),
            intensity: beat.accent,
        }
    }
}

/// Index of a section type, used as `PaletteShift` variant (intro=0 .. outro=4).
pub fn section_type_index(section_type: SectionType) -> u32 {
    SectionType::ALL
        .iter()
        .position(|t| *t == section_type)
        .unwrap_or(0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::test_support::{REAL_ANALYSES, synthetic_analysis, test_spec};

    fn real(name: &str) -> SongAnalysis {
        let (_, json) = REAL_ANALYSES
            .iter()
            .find(|(n, _)| *n == name)
            .expect("analysis");
        SongAnalysis::from_json(json).unwrap()
    }

    fn all_inputs() -> Vec<(String, SongAnalysis)> {
        let mut inputs: Vec<(String, SongAnalysis)> = REAL_ANALYSES
            .iter()
            .map(|(name, json)| (name.to_string(), SongAnalysis::from_json(json).unwrap()))
            .collect();
        inputs.push(("synthetic".into(), synthetic_analysis(120.0, 48)));
        inputs
    }

    fn hazard_count(chart: &Chart) -> usize {
        chart.events.iter().filter(|e| e.kind.is_hazard()).count()
    }

    #[test]
    fn events_sorted_and_in_bounds() {
        for (name, analysis) in all_inputs() {
            for difficulty in 1..=5 {
                let spec = test_spec(difficulty);
                let chart = generate_chart(&analysis, &spec, 7);
                assert!(!chart.events.is_empty(), "{name}");
                assert!(
                    chart.events.windows(2).all(|w| w[0].beat <= w[1].beat),
                    "{name}: not sorted"
                );
                let timing = chart.timing();
                for event in &chart.events {
                    assert!(event.spawn_beat() >= 0.0, "{name}: {event:?} spawns early");
                    let end = timing.beat_to_seconds(event.end_beat());
                    assert!(end <= chart.duration_seconds, "{name}: {event:?} ends late");
                    let p = &event.params;
                    assert!((0.0..=1.0).contains(&p.x) && (0.0..=1.0).contains(&p.y));
                    if event.kind == EventKind::SpawnEnemy {
                        assert!((p.variant as usize) < spec.enemy_pool.len());
                    }
                }
            }
        }
    }

    #[test]
    fn deterministic_per_seed_and_seeds_differ() {
        let analysis = real("celtic");
        let spec = test_spec(3);
        let a = generate_chart(&analysis, &spec, 11);
        let b = generate_chart(&analysis, &spec, 11);
        let c = generate_chart(&analysis, &spec, 12);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn silent_beats_get_nothing_audible() {
        let analysis = synthetic_analysis(120.0, 48);
        let silent: Vec<i64> = analysis
            .beats
            .iter()
            .filter(|b| b.silent)
            .map(|b| b.beat)
            .collect();
        assert!(!silent.is_empty());
        for difficulty in 1..=5 {
            let chart = generate_chart(&analysis, &test_spec(difficulty), 3);
            for event in &chart.events {
                if matches!(event.kind, EventKind::Checkpoint | EventKind::ShowHint) {
                    continue;
                }
                let beat = event.beat.floor() as i64;
                if event.beat.fract() == 0.0 {
                    assert!(!silent.contains(&beat), "{event:?} on silent beat");
                }
            }
        }
    }

    #[test]
    fn density_scales_with_difficulty() {
        for (name, analysis) in all_inputs() {
            let counts: Vec<usize> = (1..=5)
                .map(|d| {
                    (0..4)
                        .map(|seed| hazard_count(&generate_chart(&analysis, &test_spec(d), seed)))
                        .sum()
                })
                .collect();
            assert!(counts[4] > counts[0], "{name}: {counts:?}");
            assert!(counts[2] > counts[0], "{name}: {counts:?}");
        }
    }

    #[test]
    fn density_multiplier_scales_hazards() {
        let analysis = real("surf-rock");
        let mut sparse = test_spec(3);
        sparse.density = 0.5;
        let mut dense = test_spec(3);
        dense.density = 1.5;
        assert!(
            hazard_count(&generate_chart(&analysis, &dense, 1))
                > hazard_count(&generate_chart(&analysis, &sparse, 1))
        );
    }

    #[test]
    fn every_section_has_a_checkpoint() {
        for (name, analysis) in all_inputs() {
            let chart = generate_chart(&analysis, &test_spec(2), 5);
            let checkpoints = chart.checkpoint_beats();
            assert_eq!(checkpoints.len(), analysis.sections.len(), "{name}");
            for section in &analysis.sections {
                assert!(checkpoints.contains(&(section.start_beat as f64)), "{name}");
            }
        }
    }

    #[test]
    fn coverage_cap_and_active_limit_hold() {
        for (name, analysis) in all_inputs() {
            for difficulty in 1..=5u8 {
                let spec = test_spec(difficulty);
                let chart = generate_chart(&analysis, &spec, 9);
                let hazards: Vec<&ChartEvent> =
                    chart.events.iter().filter(|e| e.kind.is_hazard()).collect();
                for probe in &hazards {
                    let at = probe.beat;
                    let active: Vec<&&ChartEvent> = hazards
                        .iter()
                        .filter(|e| e.beat <= at && e.end_beat() > at)
                        .collect();
                    let coverage: f32 = active
                        .iter()
                        .map(|e| spec.pattern(e.kind).unwrap().coverage)
                        .sum();
                    assert!(
                        coverage <= coverage_cap(difficulty) + 1e-5,
                        "{name} d{difficulty} beat {at}: coverage {coverage}"
                    );
                    assert!(coverage < 1.0);
                    assert!(active.len() <= max_active_hazards(difficulty));
                }
            }
        }
    }

    #[test]
    fn arena_pulse_on_every_audible_downbeat() {
        let analysis = synthetic_analysis(120.0, 48);
        let chart = generate_chart(&analysis, &test_spec(1), 0);
        for beat in analysis
            .beats
            .iter()
            .filter(|b| b.beat % 4 == 0 && !b.silent)
        {
            assert!(
                chart
                    .events
                    .iter()
                    .any(|e| e.kind == EventKind::ArenaPulse && e.beat == beat.beat as f64),
                "missing pulse at {}",
                beat.beat
            );
        }
    }

    #[test]
    fn pattern_pool_controls_kinds_and_sections() {
        let analysis = real("celtic");
        let mut spec = test_spec(3);
        spec.pattern_pool = vec![
            PatternEntry::new(EventKind::Pulse, 1.0),
            PatternEntry::new(EventKind::Laser, 1.0).in_sections(&[SectionType::Outro]),
        ];
        let chart = generate_chart(&analysis, &spec, 4);
        assert!(chart.count_kind(EventKind::Pulse) > 0);
        for event in chart.events.iter().filter(|e| e.kind.is_hazard()) {
            assert!(matches!(event.kind, EventKind::Pulse | EventKind::Laser));
            if event.kind == EventKind::Laser {
                let section = analysis.section_index_for_beat(event.beat.floor() as i64);
                assert_eq!(analysis.sections[section].section_type, SectionType::Outro);
            }
        }
    }

    #[test]
    fn tutorial_shows_hints_and_enemies_follow_sections() {
        let analysis = real("wonders-of-the-earth");
        let mut spec = test_spec(1);
        spec.tutorial = true;
        let chart = generate_chart(&analysis, &spec, 1);
        assert_eq!(chart.count_kind(EventKind::ShowHint), HINTS.len());
        assert!(chart.count_kind(EventKind::SpawnEnemy) > 0);
        for event in chart
            .events
            .iter()
            .filter(|e| e.kind == EventKind::SpawnEnemy)
        {
            let section = analysis.section_index_for_beat(event.beat as i64);
            assert!(matches!(
                analysis.sections[section].section_type,
                SectionType::Main | SectionType::Breakdown
            ));
        }
        spec.tutorial = false;
        let chart = generate_chart(&analysis, &spec, 1);
        assert_eq!(chart.count_kind(EventKind::ShowHint), 0);
    }

    #[test]
    fn no_hazards_during_lead_in() {
        for (_, analysis) in all_inputs() {
            let chart = generate_chart(&analysis, &test_spec(5), 2);
            assert!(
                chart
                    .events
                    .iter()
                    .filter(|e| e.kind.is_hazard())
                    .all(|e| e.beat >= LEAD_IN_BEATS)
            );
        }
    }
}
