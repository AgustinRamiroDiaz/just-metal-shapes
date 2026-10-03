//! Deterministic analysis -> chart mapping, built from authored *phrases*.
//!
//! Rules (see `docs/design/complete-game-plan.md`, "Chart", and `docs/spec/hazards.md`):
//! - The song is cut into segments (one per analysis section; intros longer than
//!   `INTRO_MAX_BARS` continue as a build) and each segment into phrases of
//!   `PHRASE_BARS` bars, aligned to the segment start. A novelty spike at least two bars
//!   into a phrase starts the next phrase early.
//! - Each phrase plays one `Phrase` (a small choreography such as "laser call and
//!   response" or "rings on every kick"), picked by section type, the level's
//!   `phrases` emphasis and which hazards its `pattern_pool` allows. Phrases mirror
//!   left/right, alternate sides and escalate through their segment.
//! - `heat` (difficulty x level density x section shape x musical intensity) sets how
//!   often a phrase hits (every bar, half bar or beat).
//! - With `LevelSpec::finale`, the last `FINALE_BARS` bars of main/build music before the
//!   outro become a set piece that layers two phrases at a time.
//! - Silent beats get no hazard, enemy, `ArenaPulse`, `CameraKick`, `Flash` or
//!   `PaletteShift`. `Checkpoint` and `ShowHint` are structural and are always placed.
//! - Every non-silent downbeat gets an `ArenaPulse`; strong accents add a `CameraKick`.
//! - Each section start gets a `Checkpoint` (variant = section index).
//! - Tutorial levels show `HINTS` during the opening bars.
//! - The first appearance of each hazard kind gets one extra beat of telegraph.
//! - Enemies arrive on phrase boundaries: in pairs during breakdowns (the co-op combat
//!   phase), occasionally in main sections whose phrase is light on hazards, never in
//!   the finale.
//! - Safe path: proposals are committed in time order; the summed `coverage` of
//!   simultaneously active hazards never exceeds `coverage_cap(difficulty)` (< 1, so no
//!   pair can cover the whole arena), and at most `max_active_hazards(difficulty)`
//!   hazards are active at once. Mirrored groups are committed all-or-nothing.

use super::analysis::{BeatInfo, SectionType, SongAnalysis};
use super::chart::{Chart, ChartEvent, EventKind, HINTS, PatternParams};
use super::rng::Rng;
use super::timing::Timing;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

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
    /// UI and arena accent.
    pub accent: Rgb,
    /// Hazard color (JSB-style neon pink by default).
    pub danger: Rgb,
}

/// One hazard kind a level may use, with its tuning. Tune hazards here (or in
/// `level_catalog.rs`) rather than in the generator.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternEntry {
    pub kind: EventKind,
    /// Relative pick weight among entries allowed in the current section (used by the
    /// `Scatter` and `Breather` phrases).
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
    /// Inclusive range for `PatternParams::count`; the upper part unlocks with difficulty
    /// and is reached as a phrase escalates.
    pub count: (u32, u32),
    /// Only used on beats whose accent is at least this (0 = any beat).
    pub min_accent: f32,
}

impl PatternEntry {
    /// Entry with per-kind defaults.
    pub fn new(kind: EventKind, weight: f32) -> Self {
        // (telegraph, duration, coverage, size, speed, count)
        let (telegraph_beats, duration_beats, coverage, size, speed, count) = match kind {
            EventKind::Laser => (2.0, 1.0, 0.08, 0.07, 0.0, (1, 1)),
            EventKind::LaserSweep => (2.0, 4.0, 0.12, 0.05, 0.25, (1, 1)),
            EventKind::BulletRing => (1.0, 6.0, 0.06, 0.028, 0.35, (10, 18)),
            EventKind::Spiral => (1.0, 8.0, 0.08, 0.026, 0.30, (2, 5)),
            EventKind::Wall => (2.0, 4.0, 0.20, 0.06, 0.0, (1, 1)),
            EventKind::Pulse => (2.0, 1.0, 0.05, 0.14, 0.0, (1, 1)),
            EventKind::Bomb => (3.0, 3.0, 0.12, 0.2, 0.45, (6, 12)),
            EventKind::Spikes => (2.0, 2.0, 0.12, 0.2, 0.0, (6, 10)),
            EventKind::Barrage => (1.5, 3.0, 0.10, 0.026, 0.7, (4, 9)),
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

/// A reusable choreography a phrase of music plays. See `docs/spec/hazards.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Phrase {
    /// Sparse: one pulse or laser on strong accents only.
    Breather,
    /// Pulses on the grid: corners then center, a march across, or mirrored pairs.
    PulseGrid,
    /// Lasers alternating sides (mirrored) and closing in toward the center.
    LaserCallResponse,
    /// A bullet ring on every kick, alternating between two mirrored origins.
    RingsOnKicks,
    /// A wall with a gap sweeps in at the phrase start; a mirrored one answers later.
    SweepingWall,
    /// A spiral over the whole phrase (build risers); a counter-rotating pair later on.
    SpiralRiser,
    /// Aimed volleys on snare hits from alternating edges.
    BarrageSnares,
    /// Bombs in point-symmetric pairs on downbeats.
    BombPairs,
    /// Edge spikes thrusting in from alternating opposite sides.
    SpikeSides,
    /// Rotating laser sweeps from the center, then a beam pushing in from an edge.
    SweepCross,
    /// Weighted random picks from the pool on the phrase's grid.
    Scatter,
    /// Boss set piece: two phrases layered at once (only via `LevelSpec::finale`).
    Finale,
}

impl Phrase {
    pub const ALL: [Phrase; 12] = [
        Phrase::Breather,
        Phrase::PulseGrid,
        Phrase::LaserCallResponse,
        Phrase::RingsOnKicks,
        Phrase::SweepingWall,
        Phrase::SpiralRiser,
        Phrase::BarrageSnares,
        Phrase::BombPairs,
        Phrase::SpikeSides,
        Phrase::SweepCross,
        Phrase::Scatter,
        Phrase::Finale,
    ];

    /// Layer pairs the finale cycles through, one pair per phrase.
    pub const FINALE_LAYERS: [(Phrase, Phrase); 4] = [
        (Phrase::SweepCross, Phrase::RingsOnKicks),
        (Phrase::SweepingWall, Phrase::SpikeSides),
        (Phrase::LaserCallResponse, Phrase::BarrageSnares),
        (Phrase::BombPairs, Phrase::SpiralRiser),
    ];

    pub fn name(self) -> &'static str {
        match self {
            Phrase::Breather => "Breather",
            Phrase::PulseGrid => "PulseGrid",
            Phrase::LaserCallResponse => "LaserCallResponse",
            Phrase::RingsOnKicks => "RingsOnKicks",
            Phrase::SweepingWall => "SweepingWall",
            Phrase::SpiralRiser => "SpiralRiser",
            Phrase::BarrageSnares => "BarrageSnares",
            Phrase::BombPairs => "BombPairs",
            Phrase::SpikeSides => "SpikeSides",
            Phrase::SweepCross => "SweepCross",
            Phrase::Scatter => "Scatter",
            Phrase::Finale => "Finale",
        }
    }

    /// Hazard kinds the phrase is built from (all must be available). `Breather`,
    /// `Scatter` and `Finale` work with whatever the pool offers.
    pub fn kinds(self) -> &'static [EventKind] {
        match self {
            Phrase::PulseGrid => &[EventKind::Pulse],
            Phrase::LaserCallResponse => &[EventKind::Laser],
            Phrase::RingsOnKicks => &[EventKind::BulletRing],
            Phrase::SweepingWall => &[EventKind::Wall],
            Phrase::SpiralRiser => &[EventKind::Spiral],
            Phrase::BarrageSnares => &[EventKind::Barrage],
            Phrase::BombPairs => &[EventKind::Bomb],
            Phrase::SpikeSides => &[EventKind::Spikes],
            Phrase::SweepCross => &[EventKind::LaserSweep],
            Phrase::Breather | Phrase::Scatter | Phrase::Finale => &[],
        }
    }

    /// Base pick weight per section type (0 = never picked there).
    pub fn default_weight(self, section: SectionType) -> f32 {
        use SectionType::{Breakdown, Build, Intro, Main, Outro};
        // (intro, build, main, breakdown, outro)
        let row: [f32; 5] = match self {
            Phrase::Breather => [1.0, 0.2, 0.0, 3.0, 1.5],
            Phrase::PulseGrid => [1.5, 0.8, 0.6, 0.6, 1.0],
            Phrase::LaserCallResponse => [0.8, 1.0, 1.0, 0.3, 0.8],
            Phrase::RingsOnKicks => [0.0, 0.6, 1.2, 0.0, 0.3],
            Phrase::SweepingWall => [0.0, 0.4, 0.8, 0.0, 0.2],
            Phrase::SpiralRiser => [0.0, 1.5, 0.4, 0.0, 0.0],
            Phrase::BarrageSnares => [0.0, 0.4, 1.0, 0.0, 0.0],
            Phrase::BombPairs => [0.0, 0.3, 0.8, 0.2, 0.0],
            Phrase::SpikeSides => [0.0, 0.5, 0.8, 0.3, 0.3],
            Phrase::SweepCross => [0.0, 0.8, 0.8, 0.0, 0.0],
            Phrase::Scatter => [0.3, 0.6, 0.8, 0.2, 0.5],
            Phrase::Finale => [0.0; 5],
        };
        let index = match section {
            Intro => 0,
            Build => 1,
            Main => 2,
            Breakdown => 3,
            Outro => 4,
        };
        row[index]
    }
}

/// A level's emphasis on one phrase: multiplies `Phrase::default_weight`.
#[derive(Clone, Debug, PartialEq)]
pub struct PhraseEntry {
    pub phrase: Phrase,
    pub weight: f32,
}

impl PhraseEntry {
    pub fn new(phrase: Phrase, weight: f32) -> Self {
        Self { phrase, weight }
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
    /// Phrases this level plays and how much it favors each. Empty: every phrase at
    /// its default weight. Non-empty: only the listed phrases.
    pub phrases: Vec<PhraseEntry>,
    /// Hazard density multiplier on top of the difficulty curve (1.0 = default).
    pub density: f32,
    /// Show `ShowHint` events during the opening bars.
    pub tutorial: bool,
    /// End the last main/build stretch with the layered `Finale` set piece.
    pub finale: bool,
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

/// Bars per phrase.
pub const PHRASE_BARS: i64 = 4;
/// Intro bars beyond this play as a build (long atmospheric intros still escalate).
pub const INTRO_MAX_BARS: i64 = 8;
/// Length of the finale set piece.
pub const FINALE_BARS: i64 = 12;
/// No hazards hit before this beat, so players can orient themselves.
pub const LEAD_IN_BEATS: f64 = 4.0;
/// Hazards must finish this long before the song ends.
pub const END_MARGIN_SECONDS: f64 = 0.5;
/// Warning time for enemy spawns (the spawn effect plays for this long).
pub const ENEMY_TELEGRAPH_BEATS: f64 = 2.0;
pub const HINT_DURATION_BEATS: f64 = 6.0;
pub const HINT_SPACING_BEATS: f64 = 8.0;
/// Extra telegraph the first hazard of each kind gets.
pub const INTRODUCTION_BEATS: f64 = 1.0;

const STRONG_ACCENT: f32 = 0.75;
const KICK_ACCENT: f32 = 0.85;
const KICK_ONSET: f32 = 0.6;
const PHRASE_NOVELTY: f32 = 0.85;
/// Weight multiplier for repeating the previous phrase.
const REPEAT_PENALTY: f32 = 0.25;
/// Weight multiplier for `SweepingWall` / `SpiralRiser` on the first phrase of a
/// main / build segment.
const OPENER_BOOST: f32 = 4.0;

/// A stretch of one section played as one section type.
#[derive(Clone, Debug, PartialEq)]
struct Segment {
    section_index: usize,
    section_type: SectionType,
    start: i64,
    end: i64,
    intensity: f32,
    finale: bool,
}

/// One phrase's window and how hard it plays.
#[derive(Clone, Debug, PartialEq)]
struct PhraseCtx {
    start: i64,
    end: i64,
    section_type: SectionType,
    /// Position of the phrase in its segment, `0..=1`.
    level: f32,
    heat: f32,
    /// Running phrase number across the song (alternates directions between phrases).
    serial: u32,
    /// Index within the segment.
    index: usize,
    finale: bool,
}

impl PhraseCtx {
    fn bars(&self) -> i64 {
        ((self.end - self.start) / 4).max(1)
    }
}

#[derive(Clone, Debug)]
struct Proposal {
    beat: f64,
    telegraph: f64,
    kind: EventKind,
    params: PatternParams,
    /// Proposals sharing a group (same beat) are committed all-or-nothing.
    group: u32,
}

struct ActiveHazard {
    end: f64,
    coverage: f32,
}

/// A planned phrase, kept for enemy placement after hazards are committed.
struct PlannedPhrase {
    ctx: PhraseCtx,
    phrase: Phrase,
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
    proposals: Vec<Proposal>,
    next_group: u32,
}

/// Builds the chart for a level. Same inputs and seed always give the same chart.
pub fn generate_chart(analysis: &SongAnalysis, spec: &LevelSpec, seed: u64) -> Chart {
    let mut generator = Generator::new(analysis, spec, seed);
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

/// The phrase plan a chart is built from: `(start_beat, end_beat, phrase)`.
pub fn phrase_plan(
    analysis: &SongAnalysis,
    spec: &LevelSpec,
    seed: u64,
) -> Vec<(i64, i64, Phrase)> {
    let generator = Generator::new(analysis, spec, seed);
    let mut rng = generator.rng.fork(3);
    generator
        .plan_phrases(&mut rng)
        .into_iter()
        .map(|planned| (planned.ctx.start, planned.ctx.end, planned.phrase))
        .collect()
}

impl<'a> Generator<'a> {
    fn new(analysis: &'a SongAnalysis, spec: &'a LevelSpec, seed: u64) -> Self {
        let difficulty = spec.difficulty.clamp(1, 5);
        let difficulty_scale = 0.55 + 0.225 * (difficulty - 1) as f32;
        Self {
            analysis,
            spec,
            timing: analysis.timing(),
            rng: Rng::new(seed),
            difficulty,
            density: difficulty_scale * spec.density.max(0.0),
            cap: coverage_cap(difficulty),
            max_active: max_active_hazards(difficulty),
            events: Vec::new(),
            proposals: Vec::new(),
            next_group: 0,
        }
    }

    fn run(&mut self) {
        self.place_sections();
        if self.spec.tutorial {
            self.place_hints();
        }
        for beat_index in 0..self.analysis.beats.len() {
            let beat = &self.analysis.beats[beat_index];
            if !beat.silent {
                self.place_presentation(beat_index);
            }
        }
        let mut plan_rng = self.rng.fork(3);
        let plan = self.plan_phrases(&mut plan_rng);
        let mut hazard_rng = self.rng.fork(1);
        for planned in &plan {
            self.propose_phrase(planned.phrase, &planned.ctx, &mut hazard_rng);
        }
        self.place_finale_presentation(&plan);
        self.commit_proposals();
        let mut enemy_rng = self.rng.fork(2);
        self.place_enemies(&plan, &mut enemy_rng);
    }

    fn push(&mut self, beat: f64, telegraph_beats: f64, kind: EventKind, params: PatternParams) {
        self.events.push(ChartEvent {
            beat,
            telegraph_beats,
            kind,
            params,
        });
    }

    fn beat_info(&self, beat: i64) -> Option<&BeatInfo> {
        let beats = &self.analysis.beats;
        if let Some(info) = usize::try_from(beat).ok().and_then(|i| beats.get(i))
            && info.beat == beat
        {
            return Some(info);
        }
        beats
            .binary_search_by_key(&beat, |info| info.beat)
            .ok()
            .map(|i| &beats[i])
    }

    fn beat_is_silent(&self, beat: i64) -> bool {
        self.beat_info(beat).is_none_or(|info| info.silent)
    }

    fn accent(&self, beat: i64) -> f32 {
        self.beat_info(beat).map_or(0.0, |info| info.accent)
    }

    fn is_kick(&self, beat: i64) -> bool {
        self.beat_info(beat)
            .is_some_and(|b| !b.silent && b.low_energy >= 0.55 && b.onset_strength >= 0.45)
    }

    fn is_snare(&self, beat: i64) -> bool {
        beat.rem_euclid(2) == 1
            && self.beat_info(beat).is_some_and(|b| {
                !b.silent && b.mid_energy.max(b.high_energy) >= 0.55 && b.onset_strength >= 0.45
            })
    }

    fn bar_intensity(&self, beat: i64) -> f32 {
        self.analysis
            .bar_for_beat(beat)
            .map_or(0.5, |bar| bar.intensity)
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

    fn place_presentation(&mut self, beat_index: usize) {
        let beat = self.analysis.beats[beat_index].clone();
        if self.timing.is_downbeat(beat.beat) {
            let intensity = self.bar_intensity(beat.beat);
            self.push(
                beat.beat as f64,
                0.0,
                EventKind::ArenaPulse,
                PatternParams {
                    intensity,
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

    /// Finale start: a full-strength flash, palette shift and camera kick.
    fn place_finale_presentation(&mut self, plan: &[PlannedPhrase]) {
        let Some(first) = plan.iter().find(|p| p.ctx.finale) else {
            return;
        };
        let beat = first.ctx.start;
        if self.beat_is_silent(beat) {
            return;
        }
        let full = PatternParams {
            intensity: 1.0,
            duration_beats: 2.0,
            variant: section_type_index(SectionType::Main),
            ..PatternParams::default()
        };
        for kind in [
            EventKind::Flash,
            EventKind::PaletteShift,
            EventKind::CameraKick,
        ] {
            self.push(beat as f64, 0.0, kind, full.clone());
        }
    }

    // ----------------------------------------------------------------------------
    // Planning
    // ----------------------------------------------------------------------------

    /// Beat where the finale starts, if the level has one.
    fn finale_start(&self) -> Option<i64> {
        if !self.spec.finale {
            return None;
        }
        let sections = &self.analysis.sections;
        let outro = sections
            .iter()
            .rposition(|s| s.section_type == SectionType::Outro)
            .unwrap_or(sections.len());
        let mut run_start = None;
        for section in sections[..outro].iter().rev() {
            if matches!(section.section_type, SectionType::Main | SectionType::Build) {
                run_start = Some(section.start_beat);
            } else {
                break;
            }
        }
        let end = sections.get(outro).map_or_else(
            || sections.last().map_or(0, |s| s.end_beat),
            |s| s.start_beat,
        );
        let start = run_start?;
        Some(start.max(end - FINALE_BARS * 4))
    }

    fn segments(&self) -> Vec<Segment> {
        let finale_start = self.finale_start();
        let mut segments = Vec::new();
        for (index, section) in self.analysis.sections.iter().enumerate() {
            let mut cuts = vec![section.start_beat];
            if section.section_type == SectionType::Intro {
                let cut = section.start_beat + INTRO_MAX_BARS * 4;
                if cut < section.end_beat {
                    cuts.push(cut);
                }
            }
            if let Some(f) = finale_start
                && f > section.start_beat
                && f < section.end_beat
                && !cuts.contains(&f)
            {
                cuts.push(f);
            }
            cuts.sort_unstable();
            for (i, &start) in cuts.iter().enumerate() {
                let end = cuts.get(i + 1).copied().unwrap_or(section.end_beat);
                if end <= start {
                    continue;
                }
                let section_type = if section.section_type == SectionType::Intro
                    && start >= section.start_beat + INTRO_MAX_BARS * 4
                {
                    SectionType::Build
                } else {
                    section.section_type
                };
                let finale = finale_start.is_some_and(|f| start >= f)
                    && matches!(section_type, SectionType::Main | SectionType::Build);
                segments.push(Segment {
                    section_index: index,
                    section_type,
                    start,
                    end,
                    intensity: section.intensity,
                    finale,
                });
            }
        }
        segments
    }

    /// Phrase windows inside a segment: `PHRASE_BARS` long, cut early at novelty spikes.
    fn phrase_windows(&self, segment: &Segment) -> Vec<(i64, i64)> {
        let mut windows: Vec<(i64, i64)> = Vec::new();
        let mut start = segment.start;
        while start < segment.end {
            let mut end = (start + PHRASE_BARS * 4).min(segment.end);
            let mut bar = start + 8;
            while bar < end {
                let spike = self
                    .analysis
                    .bar_for_beat(bar)
                    .is_some_and(|b| b.novelty >= PHRASE_NOVELTY);
                if spike {
                    end = bar;
                    break;
                }
                bar += 4;
            }
            windows.push((start, end));
            start = end;
        }
        // A trailing scrap shorter than a bar joins the previous phrase.
        if windows.len() >= 2 && windows.last().is_some_and(|(s, e)| e - s < 4) {
            let (_, end) = windows.pop().unwrap_or_default();
            if let Some(last) = windows.last_mut() {
                last.1 = end;
            }
        }
        windows
    }

    fn heat(&self, segment: &Segment, level: f32, start: i64, end: i64) -> f32 {
        let shape = if segment.finale {
            1.25 + 0.25 * level
        } else {
            match segment.section_type {
                SectionType::Intro => 0.55,
                SectionType::Build => 0.45 + 0.75 * level,
                SectionType::Main => 0.8 + 0.35 * level,
                SectionType::Breakdown => 0.5,
                SectionType::Outro => 1.0 - 0.6 * level,
            }
        };
        let bars: Vec<f32> = (start..end)
            .step_by(4)
            .map(|beat| self.bar_intensity(beat))
            .collect();
        let intensity = if bars.is_empty() {
            segment.intensity
        } else {
            bars.iter().sum::<f32>() / bars.len() as f32
        };
        self.density * shape * (0.7 + 0.6 * intensity)
    }

    fn plan_phrases(&self, rng: &mut Rng) -> Vec<PlannedPhrase> {
        let mut plan = Vec::new();
        let mut previous: Option<Phrase> = None;
        let mut serial = 0u32;
        for segment in self.segments() {
            let windows = self.phrase_windows(&segment);
            let count = windows.len();
            for (index, (start, end)) in windows.into_iter().enumerate() {
                let level = if count > 1 {
                    index as f32 / (count - 1) as f32
                } else {
                    0.5
                };
                let ctx = PhraseCtx {
                    start,
                    end,
                    section_type: segment.section_type,
                    level,
                    heat: self.heat(&segment, level, start, end),
                    serial,
                    index,
                    finale: segment.finale,
                };
                serial += 1;
                let phrase = if segment.finale {
                    Phrase::Finale
                } else {
                    match self.pick_phrase(&ctx, previous, rng) {
                        Some(phrase) => phrase,
                        None => continue,
                    }
                };
                previous = Some(phrase);
                plan.push(PlannedPhrase { ctx, phrase });
            }
        }
        plan
    }

    fn phrase_weight(&self, phrase: Phrase, section: SectionType) -> f32 {
        let base = phrase.default_weight(section);
        if self.spec.phrases.is_empty() {
            return base;
        }
        self.spec
            .phrases
            .iter()
            .find(|entry| entry.phrase == phrase)
            .map_or(0.0, |entry| base * entry.weight.max(0.0))
    }

    fn phrase_available(&self, phrase: Phrase, section: SectionType) -> bool {
        match phrase {
            Phrase::Breather => [EventKind::Pulse, EventKind::Laser]
                .iter()
                .any(|kind| self.entry(*kind, section).is_some()),
            Phrase::Scatter => self
                .spec
                .pattern_pool
                .iter()
                .any(|e| e.kind.is_hazard() && e.allowed_in(section)),
            Phrase::Finale => false,
            _ => phrase
                .kinds()
                .iter()
                .all(|kind| self.entry(*kind, section).is_some()),
        }
    }

    fn pick_phrase(
        &self,
        ctx: &PhraseCtx,
        previous: Option<Phrase>,
        rng: &mut Rng,
    ) -> Option<Phrase> {
        let candidates: Vec<(Phrase, f32)> = Phrase::ALL
            .iter()
            .filter(|phrase| self.phrase_available(**phrase, ctx.section_type))
            .map(|phrase| {
                let mut weight = self.phrase_weight(*phrase, ctx.section_type);
                if Some(*phrase) == previous {
                    weight *= REPEAT_PENALTY;
                }
                let opener = match ctx.section_type {
                    SectionType::Main => Phrase::SweepingWall,
                    SectionType::Build => Phrase::SpiralRiser,
                    _ => Phrase::Finale,
                };
                if ctx.index == 0 && *phrase == opener {
                    weight *= OPENER_BOOST;
                }
                (*phrase, weight)
            })
            .filter(|(_, weight)| *weight > 0.0)
            .collect();
        let weights: Vec<f32> = candidates.iter().map(|(_, w)| *w).collect();
        rng.weighted_index(&weights).map(|i| candidates[i].0)
    }

    // ----------------------------------------------------------------------------
    // Phrase choreography
    // ----------------------------------------------------------------------------

    /// The pool entry for `kind` if this level may use it in `section`.
    fn entry(&self, kind: EventKind, section: SectionType) -> Option<&'a PatternEntry> {
        let spec: &'a LevelSpec = self.spec;
        spec.pattern_pool
            .iter()
            .find(|entry| entry.kind == kind && entry.allowed_in(section))
    }

    /// Beats between hits for a phrase's heat: a bar, half a bar, or every beat.
    fn stride(heat: f32) -> i64 {
        if heat < 0.5 {
            4
        } else if heat < 0.85 {
            2
        } else {
            1
        }
    }

    fn slots(ctx: &PhraseCtx, stride: i64) -> Vec<i64> {
        (ctx.start..ctx.end)
            .step_by(stride.max(1) as usize)
            .collect()
    }

    fn params(&self, entry: &PatternEntry, beat: i64, level: f32) -> PatternParams {
        let difficulty_t = (self.difficulty - 1) as f32 / 4.0;
        let (low, high) = entry.count;
        let unlocked = low + ((high - low) as f32 * (0.4 + 0.6 * difficulty_t)).round() as u32;
        let count = low + ((unlocked - low) as f32 * level.clamp(0.0, 1.0)).round() as u32;
        PatternParams {
            x: 0.5,
            y: 0.5,
            angle: 0.0,
            count,
            speed: entry.speed * (0.85 + 0.3 * difficulty_t),
            size: entry.size,
            duration_beats: entry.duration_beats,
            color_index: 0,
            variant: 0,
            intensity: self.accent(beat),
        }
    }

    fn propose(&mut self, beat: f64, kind: EventKind, params: PatternParams) {
        let group = self.next_group;
        self.next_group += 1;
        self.propose_in(group, beat, kind, params);
    }

    /// Several hazards on one beat that only make sense together (mirrored pairs).
    fn propose_group(&mut self, beat: f64, items: Vec<(EventKind, PatternParams)>) {
        let group = self.next_group;
        self.next_group += 1;
        for (kind, params) in items {
            self.propose_in(group, beat, kind, params);
        }
    }

    fn propose_in(&mut self, group: u32, beat: f64, kind: EventKind, mut params: PatternParams) {
        let Some(entry) = self.spec.pattern(kind) else {
            return;
        };
        params.x = params.x.clamp(0.0, 1.0);
        params.y = params.y.clamp(0.0, 1.0);
        self.proposals.push(Proposal {
            beat,
            telegraph: entry.telegraph_beats,
            kind,
            params,
            group,
        });
    }

    fn propose_phrase(&mut self, phrase: Phrase, ctx: &PhraseCtx, rng: &mut Rng) {
        match phrase {
            Phrase::Breather => self.breather(ctx, rng),
            Phrase::PulseGrid => self.pulse_grid(ctx, rng),
            Phrase::LaserCallResponse => self.laser_call_response(ctx, rng),
            Phrase::RingsOnKicks => self.rings_on_kicks(ctx, rng),
            Phrase::SweepingWall => self.sweeping_wall(ctx, rng),
            Phrase::SpiralRiser => self.spiral_riser(ctx, rng),
            Phrase::BarrageSnares => self.barrage_snares(ctx, rng),
            Phrase::BombPairs => self.bomb_pairs(ctx, rng),
            Phrase::SpikeSides => self.spike_sides(ctx, rng),
            Phrase::SweepCross => self.sweep_cross(ctx, rng),
            Phrase::Scatter => self.scatter(ctx, rng),
            Phrase::Finale => self.finale(ctx, rng),
        }
    }

    fn breather(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let kinds: Vec<&PatternEntry> = [EventKind::Pulse, EventKind::Laser]
            .iter()
            .filter_map(|kind| self.entry(*kind, ctx.section_type))
            .collect();
        if kinds.is_empty() {
            return;
        }
        let mut last: Option<i64> = None;
        for beat in ctx.start..ctx.end {
            if self.accent(beat) < STRONG_ACCENT || last.is_some_and(|l| beat - l < 4) {
                continue;
            }
            last = Some(beat);
            let weights: Vec<f32> = kinds.iter().map(|e| e.weight).collect();
            let Some(index) = rng.weighted_index(&weights) else {
                return;
            };
            let entry = kinds[index];
            let mut params = self.params(entry, beat, ctx.level);
            if entry.kind == EventKind::Laser {
                let vertical = rng.chance(0.5);
                let side = rng.range_f32(0.2, 0.4);
                let pos = if rng.chance(0.5) { side } else { 1.0 - side };
                (params.x, params.y, params.angle) = if vertical {
                    (pos, 0.5, FRAC_PI_2)
                } else {
                    (0.5, pos, 0.0)
                };
            } else {
                params.x = rng.range_f32(0.2, 0.8);
                params.y = rng.range_f32(0.25, 0.75);
            }
            self.propose(beat as f64, entry.kind, params);
        }
    }

    fn pulse_grid(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::Pulse, ctx.section_type) else {
            return;
        };
        const CORNERS: [(f32, f32); 5] = [
            (0.25, 0.3),
            (0.75, 0.7),
            (0.75, 0.3),
            (0.25, 0.7),
            (0.5, 0.5),
        ];
        let slots = Self::slots(ctx, Self::stride(ctx.heat));
        let pattern = rng.range_u32(0, 2);
        let row = rng.range_f32(0.28, 0.38);
        for (i, &beat) in slots.iter().enumerate() {
            let mut params = self.params(entry, beat, ctx.level);
            params.size *= 1.0 + 0.25 * ctx.level;
            match pattern {
                0 => {
                    (params.x, params.y) = CORNERS[i % CORNERS.len()];
                    self.propose(beat as f64, EventKind::Pulse, params);
                }
                1 => {
                    let t = i as f32 / (slots.len().max(2) - 1) as f32;
                    params.x = 0.12 + 0.76 * t.min(1.0);
                    params.y = if i % 2 == 0 { row } else { 1.0 - row };
                    self.propose(beat as f64, EventKind::Pulse, params);
                }
                _ => {
                    params.x = rng.range_f32(0.15, 0.38);
                    params.y = rng.range_f32(0.2, 0.8);
                    let mut mirror = params.clone();
                    mirror.x = 1.0 - params.x;
                    mirror.y = 1.0 - params.y;
                    mirror.color_index = 1;
                    self.propose_group(
                        beat as f64,
                        vec![(EventKind::Pulse, params), (EventKind::Pulse, mirror)],
                    );
                }
            }
        }
    }

    fn laser_call_response(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::Laser, ctx.section_type) else {
            return;
        };
        let vertical = rng.chance(0.5);
        let slots = Self::slots(ctx, Self::stride(ctx.heat));
        let n = slots.len().max(1) as f32;
        for (i, &beat) in slots.iter().enumerate() {
            let mut params = self.params(entry, beat, ctx.level);
            // Call on one side, response mirrored on the other, closing in each pair.
            let pair = (i / 2) as f32 * 2.0 / n;
            let inset = 0.12 + 0.28 * pair * (0.5 + 0.5 * ctx.level);
            let pos = if i % 2 == 0 { inset } else { 1.0 - inset };
            (params.x, params.y, params.angle) = if vertical {
                (pos, 0.5, FRAC_PI_2)
            } else {
                (0.5, pos, 0.0)
            };
            params.color_index = (i % 2) as u32;
            params.variant = (i % 2) as u32;
            self.propose(beat as f64, EventKind::Laser, params);
        }
    }

    fn rings_on_kicks(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::BulletRing, ctx.section_type) else {
            return;
        };
        let origins = match rng.range_u32(0, 2) {
            0 => [(0.2, 0.5), (0.8, 0.5)],
            1 => [(0.15, 0.2), (0.85, 0.8)],
            _ => [(0.5, 0.2), (0.5, 0.8)],
        };
        let spacing = Self::stride(ctx.heat).max(1);
        let gap = self.difficulty <= 2 || ctx.level < 0.4;
        let mut last: Option<i64> = None;
        let mut index = 0usize;
        for beat in ctx.start..ctx.end {
            let downbeat = self.timing.is_downbeat(beat);
            if !(self.is_kick(beat) || downbeat) || last.is_some_and(|l| beat - l < spacing) {
                continue;
            }
            last = Some(beat);
            let mut params = self.params(entry, beat, ctx.level);
            let count = params.count.max(1);
            (params.x, params.y) = origins[index % 2];
            // Successive rings interleave so their bullets fill each other's gaps.
            params.angle = (index as f32) * PI / count as f32;
            params.variant = u32::from(gap);
            params.color_index = (index % 2) as u32;
            self.propose(beat as f64, EventKind::BulletRing, params);
            index += 1;
        }
    }

    fn sweeping_wall(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::Wall, ctx.section_type) else {
            return;
        };
        // Wall travel direction: 0 right, 1 down, 2 left, 3 up (angle = dir * 90 deg).
        let dir = if rng.chance(0.7) {
            2 * rng.range_u32(0, 1)
        } else {
            1 + 2 * rng.range_u32(0, 1)
        };
        let gap = rng.range_f32(0.3, 0.7);
        let mut params = self.params(entry, ctx.start, ctx.level);
        params.angle = dir as f32 * FRAC_PI_2;
        (params.x, params.y) = (gap, gap);
        self.propose(ctx.start as f64, EventKind::Wall, params.clone());
        let answer = ctx.start + 8;
        if ctx.bars() >= 4 && (ctx.level >= 0.5 || self.difficulty >= 3) {
            let mut mirror = params;
            mirror.angle = ((dir + 2) % 4) as f32 * FRAC_PI_2;
            (mirror.x, mirror.y) = (1.0 - gap, 1.0 - gap);
            mirror.intensity = self.accent(answer);
            mirror.color_index = 1;
            self.propose(answer as f64, EventKind::Wall, mirror);
        }
        self.accent_fill(ctx, ctx.start + 2, rng);
    }

    fn spiral_riser(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::Spiral, ctx.section_type) else {
            return;
        };
        // A spiral every two bars, each turning the other way with one more arm.
        let length = ((ctx.end - ctx.start - 1) as f64).clamp(4.0, 8.0);
        let base_angle = rng.range_u32(0, 7) as f32 * FRAC_PI_4;
        for (i, start) in (ctx.start..ctx.end).step_by(8).enumerate() {
            if start as f64 + length > ctx.end as f64 {
                break;
            }
            let mut params = self.params(entry, start, ctx.level);
            params.duration_beats = length;
            params.angle = base_angle + i as f32 * FRAC_PI_4;
            params.count += i as u32;
            let clockwise = (ctx.serial + i as u32) % 2;
            if self.difficulty >= 4 && ctx.level > 0.5 {
                let mut left = params.clone();
                (left.x, left.y, left.variant) = (0.3, 0.5, clockwise);
                let mut right = params;
                (right.x, right.y, right.variant) = (0.7, 0.5, 1 - clockwise);
                right.angle = PI - left.angle;
                right.color_index = 1;
                self.propose_group(
                    start as f64,
                    vec![(EventKind::Spiral, left), (EventKind::Spiral, right)],
                );
            } else {
                params.variant = clockwise;
                self.propose(start as f64, EventKind::Spiral, params);
            }
        }
        self.accent_fill(ctx, ctx.start + 4, rng);
    }

    fn barrage_snares(&mut self, ctx: &PhraseCtx, _rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::Barrage, ctx.section_type) else {
            return;
        };
        const EMITTERS: [(f32, f32); 4] = [(0.03, 0.25), (0.97, 0.75), (0.03, 0.75), (0.97, 0.25)];
        let spacing = Self::stride(ctx.heat).max(2);
        let mut beats: Vec<i64> = Vec::new();
        for beat in ctx.start..ctx.end {
            if self.is_snare(beat) && beats.last().is_none_or(|last| beat - last >= spacing) {
                beats.push(beat);
            }
        }
        if beats.is_empty() {
            beats = (ctx.start + 1..ctx.end)
                .step_by(spacing as usize * 2)
                .collect();
        }
        for (i, beat) in beats.into_iter().enumerate() {
            let mut params = self.params(entry, beat, ctx.level);
            (params.x, params.y) = EMITTERS[i % EMITTERS.len()];
            params.color_index = (i % 2) as u32;
            self.propose(beat as f64, EventKind::Barrage, params);
        }
    }

    fn bomb_pairs(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::Bomb, ctx.section_type) else {
            return;
        };
        let spacing = if ctx.heat >= 0.85 { 4 } else { 8 };
        for beat in Self::slots(ctx, spacing) {
            if beat + 2 > ctx.end {
                break;
            }
            let mut params = self.params(entry, beat, ctx.level);
            params.x = rng.range_f32(0.18, 0.32);
            params.y = rng.range_f32(0.3, 0.7);
            let mut mirror = params.clone();
            mirror.x = 1.0 - params.x;
            mirror.y = 1.0 - params.y;
            mirror.color_index = 1;
            self.propose_group(
                beat as f64,
                vec![(EventKind::Bomb, params), (EventKind::Bomb, mirror)],
            );
        }
    }

    fn spike_sides(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::Spikes, ctx.section_type) else {
            return;
        };
        // Sides: 0 left, 1 top, 2 right, 3 bottom. Opposite sides alternate.
        let first = if rng.chance(0.6) { 0 } else { 1 } + 2 * rng.range_u32(0, 1);
        let comb = ctx.level < 0.5;
        let stride = if Self::stride(ctx.heat) <= 2 { 2 } else { 4 };
        for (i, beat) in Self::slots(ctx, stride).into_iter().enumerate() {
            let mut params = self.params(entry, beat, ctx.level);
            let side = (first + 2 * (i as u32 % 2)) % 4;
            params.variant = side + if comb { 4 } else { 0 };
            params.color_index = (i % 2) as u32;
            self.propose(beat as f64, EventKind::Spikes, params);
        }
    }

    fn sweep_cross(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let Some(entry) = self.entry(EventKind::LaserSweep, ctx.section_type) else {
            return;
        };
        let mut params = self.params(entry, ctx.start, ctx.level);
        params.angle = if rng.chance(0.5) { 0.0 } else { FRAC_PI_4 };
        // Rotation mode (variant bit 0 clear); bit 1 picks the direction.
        let direction = 2 * (ctx.serial % 2);
        params.variant = direction;
        if self.difficulty >= 4 && ctx.level > 0.5 {
            let mut cross = params.clone();
            cross.angle = params.angle + FRAC_PI_2;
            cross.variant = 2 - direction;
            cross.color_index = 1;
            self.propose_group(
                ctx.start as f64,
                vec![
                    (EventKind::LaserSweep, params),
                    (EventKind::LaserSweep, cross),
                ],
            );
        } else {
            self.propose(ctx.start as f64, EventKind::LaserSweep, params);
        }
        // Then a beam pushing in from an edge (translation mode).
        let push = ctx.start + 8;
        if push + 2 <= ctx.end {
            let mut beam = self.params(entry, push, ctx.level);
            let side = rng.range_u32(0, 3);
            (beam.x, beam.y, beam.angle) = match side {
                0 => (0.02, 0.5, FRAC_PI_2),
                1 => (0.98, 0.5, FRAC_PI_2),
                2 => (0.5, 0.03, 0.0),
                _ => (0.5, 0.97, 0.0),
            };
            beam.variant = 1;
            self.propose(push as f64, EventKind::LaserSweep, beam);
        }
        self.accent_fill(ctx, ctx.start + 4, rng);
    }

    fn scatter(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let entries: Vec<&PatternEntry> = self
            .spec
            .pattern_pool
            .iter()
            .filter(|e| e.kind.is_hazard() && e.allowed_in(ctx.section_type))
            .collect();
        let weights: Vec<f32> = entries.iter().map(|e| e.weight).collect();
        for beat in Self::slots(ctx, Self::stride(ctx.heat)) {
            let Some(index) = rng.weighted_index(&weights) else {
                return;
            };
            let entry = entries[index];
            let params = self.random_params(entry, beat, ctx.level, rng);
            self.propose(beat as f64, entry.kind, params);
        }
    }

    /// Free-form placement for one hazard kind (used by `Scatter` and fills).
    fn random_params(
        &self,
        entry: &PatternEntry,
        beat: i64,
        level: f32,
        rng: &mut Rng,
    ) -> PatternParams {
        let mut params = self.params(entry, beat, level);
        params.x = rng.range_f32(0.15, 0.85);
        params.y = rng.range_f32(0.15, 0.85);
        params.angle = rng.range_u32(0, 7) as f32 * FRAC_PI_4;
        match entry.kind {
            EventKind::Wall => params.angle = rng.range_u32(0, 3) as f32 * FRAC_PI_2,
            EventKind::Barrage => {
                params.x = if rng.chance(0.5) { 0.03 } else { 0.97 };
            }
            EventKind::Spikes => params.variant = rng.range_u32(0, 7),
            EventKind::LaserSweep => params.variant = 2 * rng.range_u32(0, 1),
            EventKind::BulletRing | EventKind::Spiral => params.variant = rng.range_u32(0, 1),
            _ => {}
        }
        params
    }

    /// Small hits inside a phrase built around one long hazard, from the cheap kinds
    /// (pulse, laser) the level allows: on strong accents, and on downbeats too once
    /// the phrase runs hot.
    fn accent_fill(&mut self, ctx: &PhraseCtx, from: i64, rng: &mut Rng) {
        let fills: Vec<&PatternEntry> = [EventKind::Pulse, EventKind::Laser]
            .iter()
            .filter_map(|kind| self.entry(*kind, ctx.section_type))
            .collect();
        if fills.is_empty() {
            return;
        }
        let spacing = (Self::stride(ctx.heat) * 2).max(2);
        let downbeats = ctx.heat >= 0.6;
        let mut last: Option<i64> = None;
        let mut side = 0;
        for beat in from.max(ctx.start)..ctx.end {
            let strong =
                self.accent(beat) >= STRONG_ACCENT || (downbeats && self.timing.is_downbeat(beat));
            if !strong || last.is_some_and(|l| beat - l < spacing) {
                continue;
            }
            last = Some(beat);
            let entry = fills[rng.range_u32(0, fills.len() as u32 - 1) as usize];
            let mut params = self.params(entry, beat, ctx.level);
            let x = if side % 2 == 0 { 0.15 } else { 0.85 };
            side += 1;
            if entry.kind == EventKind::Laser {
                (params.x, params.y, params.angle) = (x, 0.5, FRAC_PI_2);
            } else {
                (params.x, params.y) = (x, rng.range_f32(0.2, 0.8));
            }
            self.propose(beat as f64, entry.kind, params);
        }
    }

    fn finale(&mut self, ctx: &PhraseCtx, rng: &mut Rng) {
        let available: Vec<(Phrase, Phrase)> = Phrase::FINALE_LAYERS
            .iter()
            .copied()
            .filter(|(a, b)| {
                self.phrase_available(*a, ctx.section_type)
                    || self.phrase_available(*b, ctx.section_type)
            })
            .collect();
        if available.is_empty() {
            self.scatter(ctx, rng);
            return;
        }
        let (a, b) = available[ctx.index % available.len()];
        let mut layer = ctx.clone();
        layer.level = (0.6 + 0.4 * ctx.level).min(1.0);
        for phrase in [a, b] {
            if self.phrase_available(phrase, ctx.section_type) {
                self.propose_phrase(phrase, &layer, rng);
            }
        }
    }

    // ----------------------------------------------------------------------------
    // Commit (safe path) and enemies
    // ----------------------------------------------------------------------------

    fn commit_proposals(&mut self) {
        let mut proposals = std::mem::take(&mut self.proposals);
        proposals.sort_by(|a, b| a.beat.total_cmp(&b.beat).then(a.group.cmp(&b.group)));
        let mut active: Vec<ActiveHazard> = Vec::new();
        let mut introduced: Vec<EventKind> = Vec::new();
        let mut i = 0;
        while i < proposals.len() {
            let group = proposals[i].group;
            let mut j = i;
            while j < proposals.len() && proposals[j].group == group {
                j += 1;
            }
            self.commit_group(&proposals[i..j], &mut active, &mut introduced);
            i = j;
        }
    }

    fn proposal_fits(&self, proposal: &Proposal, entry: &PatternEntry) -> bool {
        let beat = proposal.beat;
        let end = beat + proposal.params.duration_beats;
        beat >= LEAD_IN_BEATS
            && beat - proposal.telegraph >= 0.0
            && !self.beat_is_silent(beat.floor() as i64)
            && self.accent(beat.floor() as i64) >= entry.min_accent
            && self.timing.beat_to_seconds(end)
                <= self.analysis.duration_seconds - END_MARGIN_SECONDS
    }

    fn commit_group(
        &mut self,
        group: &[Proposal],
        active: &mut Vec<ActiveHazard>,
        introduced: &mut Vec<EventKind>,
    ) {
        let Some(first) = group.first() else {
            return;
        };
        let beat = first.beat;
        let mut coverage = 0.0;
        for proposal in group {
            let Some(entry) = self.spec.pattern(proposal.kind) else {
                return;
            };
            if !self.proposal_fits(proposal, entry) {
                return;
            }
            coverage += entry.coverage;
        }
        active.retain(|hazard| hazard.end > beat);
        if active.len() + group.len() > self.max_active {
            return;
        }
        // Telegraphs are harmless, so only active spans count. Every hazard in `active`
        // already overlaps `beat`, hence the group's whole span.
        let used: f32 = active.iter().map(|hazard| hazard.coverage).sum();
        if used + coverage > self.cap + 1e-6 {
            return;
        }
        for proposal in group {
            let entry_coverage = self.spec.pattern(proposal.kind).map_or(0.0, |e| e.coverage);
            let mut telegraph = proposal.telegraph;
            if !introduced.contains(&proposal.kind) {
                introduced.push(proposal.kind);
                if beat - telegraph - INTRODUCTION_BEATS >= 0.0 {
                    telegraph += INTRODUCTION_BEATS;
                }
            }
            active.push(ActiveHazard {
                end: beat + proposal.params.duration_beats,
                coverage: entry_coverage,
            });
            self.push(beat, telegraph, proposal.kind, proposal.params.clone());
        }
    }

    fn hazards_between(&self, start: i64, end: i64) -> usize {
        self.events
            .iter()
            .filter(|e| e.kind.is_hazard() && e.beat >= start as f64 && e.beat < end as f64)
            .count()
    }

    fn place_enemies(&mut self, plan: &[PlannedPhrase], rng: &mut Rng) {
        if self.spec.enemy_pool.is_empty() {
            return;
        }
        let difficulty_scale = 0.6 + 0.2 * (self.difficulty - 1) as f32;
        let mut per_section: Vec<usize> = vec![0; self.analysis.sections.len()];
        for (index, planned) in plan.iter().enumerate() {
            let ctx = &planned.ctx;
            if ctx.finale {
                continue;
            }
            let section_index = self.analysis.section_index_for_beat(ctx.start);
            let section = &self.analysis.sections[section_index];
            let section_bars = (section.end_bar - section.start_bar).max(1) as usize;
            let (limit, wanted) = match ctx.section_type {
                SectionType::Breakdown => {
                    let pair = self.difficulty >= 3 && ctx.bars() >= 4;
                    ((section_bars / 3).max(1), if pair { 2 } else { 1 })
                }
                SectionType::Main => {
                    let per_bar =
                        self.hazards_between(ctx.start, ctx.end) as f32 / ctx.bars() as f32;
                    let chance =
                        ((0.45 - 0.15 * per_bar).clamp(0.05, 0.4) * difficulty_scale) as f64;
                    let first_in_section = per_section[section_index] == 0
                        && plan.get(index + 1).is_none_or(|next| {
                            self.analysis.section_index_for_beat(next.ctx.start) != section_index
                        });
                    let wanted = usize::from(first_in_section || rng.chance(chance));
                    ((section_bars / 6).max(1), wanted)
                }
                _ => continue,
            };
            let room = limit.saturating_sub(per_section[section_index]);
            let wanted = wanted.min(room);
            if wanted == 0 {
                continue;
            }
            let Some(beat) = (ctx.start..ctx.end)
                .step_by(4)
                .find(|beat| !self.beat_is_silent(*beat))
            else {
                continue;
            };
            if (beat as f64) < ENEMY_TELEGRAPH_BEATS {
                continue;
            }
            let x = rng.range_f32(0.18, 0.4);
            let y = rng.range_f32(0.2, 0.8);
            let angle = rng.range_f32(0.0, TAU);
            for k in 0..wanted {
                let weights: Vec<f32> = self.spec.enemy_pool.iter().map(|e| e.weight).collect();
                let Some(variant) = rng.weighted_index(&weights) else {
                    return;
                };
                // The second enemy of a pair mirrors the first.
                let (ex, ey, ea) = if k == 0 {
                    (x, y, angle)
                } else {
                    (1.0 - x, 1.0 - y, angle + PI)
                };
                let params = PatternParams {
                    x: ex,
                    y: ey,
                    angle: ea.rem_euclid(TAU),
                    variant: variant as u32,
                    intensity: section.intensity,
                    ..PatternParams::default()
                };
                self.push(
                    beat as f64,
                    ENEMY_TELEGRAPH_BEATS,
                    EventKind::SpawnEnemy,
                    params,
                );
                per_section[section_index] += 1;
            }
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

    /// Proposals one phrase makes in isolation (before the safe-path commit).
    fn phrase_proposals(
        analysis: &SongAnalysis,
        spec: &LevelSpec,
        phrase: Phrase,
        ctx: &PhraseCtx,
        seed: u64,
    ) -> Vec<Proposal> {
        let mut generator = Generator::new(analysis, spec, seed);
        let mut rng = Rng::new(seed);
        generator.propose_phrase(phrase, ctx, &mut rng);
        generator.proposals
    }

    fn ctx(start: i64, bars: i64, section_type: SectionType, level: f32, heat: f32) -> PhraseCtx {
        PhraseCtx {
            start,
            end: start + bars * 4,
            section_type,
            level,
            heat,
            serial: 0,
            index: 0,
            finale: false,
        }
    }

    fn only(proposals: &[Proposal], kind: EventKind) -> Vec<&Proposal> {
        proposals.iter().filter(|p| p.kind == kind).collect()
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
                    if event.kind.is_hazard() {
                        assert!(p.count >= 1, "{name}: {event:?} count");
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
        assert_eq!(
            phrase_plan(&analysis, &spec, 11),
            phrase_plan(&analysis, &spec, 11)
        );
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
                assert!(
                    !silent.contains(&(event.beat.floor() as i64)),
                    "{event:?} on silent beat"
                );
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

    /// The safe-path invariant: at every hazard's hit beat, the summed coverage and the
    /// number of active hazards stay under the difficulty's caps.
    pub(crate) fn assert_safe_path(name: &str, chart: &Chart, spec: &LevelSpec) {
        let difficulty = spec.difficulty;
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
            assert!(
                active.len() <= max_active_hazards(difficulty),
                "{name} d{difficulty} beat {at}: {} active",
                active.len()
            );
        }
    }

    #[test]
    fn coverage_cap_and_active_limit_hold() {
        for (name, analysis) in all_inputs() {
            for difficulty in 1..=5u8 {
                for seed in [9, 10] {
                    let mut spec = test_spec(difficulty);
                    spec.finale = true;
                    let chart = generate_chart(&analysis, &spec, seed);
                    assert_safe_path(&name, &chart, &spec);
                }
            }
        }
    }

    #[test]
    fn concurrent_hazards_bounded_per_difficulty() {
        // Sampled on every half beat (not only hit beats).
        for (name, analysis) in all_inputs() {
            for difficulty in 1..=5u8 {
                let spec = test_spec(difficulty);
                let chart = generate_chart(&analysis, &spec, 21);
                let hazards: Vec<&ChartEvent> =
                    chart.events.iter().filter(|e| e.kind.is_hazard()).collect();
                let last = hazards.iter().map(|e| e.end_beat()).fold(0.0, f64::max);
                let mut beat = 0.0;
                let mut peak = 0;
                while beat <= last {
                    let active = hazards
                        .iter()
                        .filter(|e| e.beat <= beat && e.end_beat() > beat)
                        .count();
                    peak = peak.max(active);
                    beat += 0.5;
                }
                assert!(
                    peak <= max_active_hazards(difficulty),
                    "{name} d{difficulty}: {peak} concurrent"
                );
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
    fn level_phrase_list_restricts_phrases() {
        let analysis = real("surf-rock");
        let mut spec = test_spec(4);
        spec.phrases = vec![
            PhraseEntry::new(Phrase::RingsOnKicks, 1.0),
            PhraseEntry::new(Phrase::Breather, 1.0),
            PhraseEntry::new(Phrase::PulseGrid, 1.0),
        ];
        for (_, _, phrase) in phrase_plan(&analysis, &spec, 3) {
            assert!(matches!(
                phrase,
                Phrase::RingsOnKicks | Phrase::Breather | Phrase::PulseGrid
            ));
        }
        let chart = generate_chart(&analysis, &spec, 3);
        for event in chart.events.iter().filter(|e| e.kind.is_hazard()) {
            assert!(matches!(
                event.kind,
                EventKind::BulletRing | EventKind::Pulse | EventKind::Laser
            ));
        }
    }

    #[test]
    fn phrases_tile_the_song_on_bar_lines() {
        for (name, analysis) in all_inputs() {
            let plan = phrase_plan(&analysis, &test_spec(3), 1);
            assert!(!plan.is_empty(), "{name}");
            for (start, end, _) in &plan {
                assert!(end > start, "{name}");
                assert!(end - start <= PHRASE_BARS * 4 + 3, "{name}: {start}..{end}");
                assert_eq!(start % 4, 0, "{name}: phrase off the bar line at {start}");
            }
            for pair in plan.windows(2) {
                assert!(pair[0].1 <= pair[1].0, "{name}: overlapping phrases");
            }
        }
    }

    #[test]
    fn long_intros_continue_as_builds() {
        let analysis = real("voxel-revolution");
        let spec = test_spec(2);
        let generator = Generator::new(&analysis, &spec, 1);
        let segments = generator.segments();
        let intro = &segments[0];
        assert_eq!(intro.section_type, SectionType::Intro);
        assert_eq!(intro.end - intro.start, INTRO_MAX_BARS * 4);
        assert_eq!(segments[1].section_type, SectionType::Build);
        assert_eq!(segments[1].section_index, 0);
    }

    #[test]
    fn pulse_grid_mirrors_and_stays_in_bounds() {
        let analysis = synthetic_analysis(120.0, 48);
        let spec = test_spec(3);
        for seed in 0..12 {
            let proposals = phrase_proposals(
                &analysis,
                &spec,
                Phrase::PulseGrid,
                &ctx(64, 4, SectionType::Main, 0.5, 1.0),
                seed,
            );
            assert!(!proposals.is_empty());
            for p in &proposals {
                assert_eq!(p.kind, EventKind::Pulse);
                assert!((0.05..=0.95).contains(&p.params.x) && (0.05..=0.95).contains(&p.params.y));
            }
            for pair in proposals.windows(2).filter(|w| w[0].group == w[1].group) {
                assert!((pair[0].params.x + pair[1].params.x - 1.0).abs() < 1e-5);
                assert!((pair[0].params.y + pair[1].params.y - 1.0).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn laser_call_response_alternates_and_closes_in() {
        let analysis = synthetic_analysis(120.0, 48);
        let spec = test_spec(3);
        let proposals = phrase_proposals(
            &analysis,
            &spec,
            Phrase::LaserCallResponse,
            &ctx(64, 4, SectionType::Main, 1.0, 0.7),
            5,
        );
        assert_eq!(proposals.len(), 8, "every half bar");
        let positions: Vec<f32> = proposals
            .iter()
            .map(|p| {
                if (p.params.angle - FRAC_PI_2).abs() < 1e-5 {
                    p.params.x
                } else {
                    p.params.y
                }
            })
            .collect();
        for (i, pos) in positions.iter().enumerate() {
            assert_eq!(*pos < 0.5, i % 2 == 0, "side alternates: {positions:?}");
        }
        for pair in positions.chunks(2) {
            assert!(
                (pair[0] + pair[1] - 1.0).abs() < 1e-5,
                "response mirrors call"
            );
        }
        let insets: Vec<f32> = positions.iter().map(|p| p.min(1.0 - p)).collect();
        assert!(insets.windows(2).all(|w| w[1] >= w[0] - 1e-6), "{insets:?}");
        assert!(insets.last().unwrap() > insets.first().unwrap());
    }

    #[test]
    fn rings_alternate_between_mirrored_origins() {
        let analysis = real("surf-rock");
        let spec = test_spec(4);
        for seed in 0..6 {
            let proposals = phrase_proposals(
                &analysis,
                &spec,
                Phrase::RingsOnKicks,
                &ctx(64 * 2, 4, SectionType::Main, 0.8, 1.2),
                seed,
            );
            assert!(proposals.len() >= 4, "a ring at least every bar");
            for pair in proposals.windows(2) {
                let (a, b) = (&pair[0].params, &pair[1].params);
                assert!((a.x + b.x - 1.0).abs() < 1e-5 && (a.y + b.y - 1.0).abs() < 1e-5);
                assert!(pair[1].beat > pair[0].beat);
            }
            for p in &proposals {
                let beat = p.beat as i64;
                let generator = Generator::new(&analysis, &spec, 0);
                assert!(generator.is_kick(beat) || beat % 4 == 0);
            }
        }
    }

    #[test]
    fn sweeping_wall_opens_the_phrase_and_answers_mirrored() {
        let analysis = synthetic_analysis(120.0, 48);
        let spec = test_spec(3);
        for seed in 0..8 {
            let proposals = phrase_proposals(
                &analysis,
                &spec,
                Phrase::SweepingWall,
                &ctx(64, 4, SectionType::Main, 0.6, 0.7),
                seed,
            );
            let walls = only(&proposals, EventKind::Wall);
            assert_eq!(walls.len(), 2);
            assert_eq!(walls[0].beat, 64.0);
            assert_eq!(walls[1].beat, 72.0);
            let (a, b) = (&walls[0].params, &walls[1].params);
            assert!((0.25..=0.75).contains(&a.x));
            assert!((a.x + b.x - 1.0).abs() < 1e-5, "gap mirrored");
            let turn = (b.angle - a.angle).rem_euclid(TAU);
            assert!((turn - PI).abs() < 1e-4, "answer comes from the other side");
        }
    }

    #[test]
    fn spiral_riser_escalates_and_alternates_direction() {
        let analysis = synthetic_analysis(120.0, 48);
        let spec = test_spec(3);
        let mut first_directions = Vec::new();
        for serial in 0..4 {
            let mut c = ctx(32, 4, SectionType::Build, 0.3, 0.6);
            c.serial = serial;
            let proposals = phrase_proposals(&analysis, &spec, Phrase::SpiralRiser, &c, 2);
            let spirals = only(&proposals, EventKind::Spiral);
            assert_eq!(spirals.len(), 2, "one spiral every two bars");
            assert_eq!(spirals[0].beat, 32.0);
            assert_eq!(spirals[1].beat, 40.0);
            for spiral in &spirals {
                let end = spiral.beat + spiral.params.duration_beats;
                assert!(end <= c.end as f64, "the riser ends with its phrase");
            }
            assert_ne!(spirals[0].params.variant, spirals[1].params.variant);
            assert_eq!(spirals[1].params.count, spirals[0].params.count + 1);
            first_directions.push(spirals[0].params.variant);
        }
        assert_eq!(first_directions, vec![0, 1, 0, 1], "phrases alternate too");

        let pair = phrase_proposals(
            &analysis,
            &test_spec(5),
            Phrase::SpiralRiser,
            &ctx(32, 4, SectionType::Build, 0.9, 1.2),
            2,
        );
        let spirals = only(&pair, EventKind::Spiral);
        assert_eq!(
            spirals.len(),
            4,
            "counter-rotating pairs late in a hard build"
        );
        assert_eq!(spirals[0].group, spirals[1].group);
        assert_ne!(spirals[0].params.variant, spirals[1].params.variant);
        assert!((spirals[0].params.x + spirals[1].params.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn barrage_fires_from_alternating_edges_on_snares() {
        let analysis = real("surf-rock");
        let spec = test_spec(4);
        let c = ctx(128, 4, SectionType::Main, 0.5, 1.0);
        let proposals = phrase_proposals(&analysis, &spec, Phrase::BarrageSnares, &c, 1);
        assert!(!proposals.is_empty());
        let generator = Generator::new(&analysis, &spec, 0);
        let any_snare = (c.start..c.end).any(|b| generator.is_snare(b));
        for (i, p) in proposals.iter().enumerate() {
            assert!(p.params.x < 0.05 || p.params.x > 0.95, "emitter on an edge");
            assert_eq!(p.params.x < 0.5, i % 2 == 0, "edges alternate");
            if any_snare {
                assert!(
                    generator.is_snare(p.beat as i64),
                    "beat {} is a snare",
                    p.beat
                );
            }
        }
    }

    #[test]
    fn bombs_come_in_point_symmetric_pairs() {
        let analysis = synthetic_analysis(120.0, 48);
        let proposals = phrase_proposals(
            &analysis,
            &test_spec(4),
            Phrase::BombPairs,
            &ctx(64, 4, SectionType::Main, 0.5, 1.0),
            3,
        );
        assert_eq!(proposals.len() % 2, 0);
        assert!(!proposals.is_empty());
        for pair in proposals.chunks(2) {
            assert_eq!(pair[0].group, pair[1].group);
            assert_eq!(pair[0].beat, pair[1].beat);
            assert!((pair[0].params.x + pair[1].params.x - 1.0).abs() < 1e-5);
            assert!((pair[0].params.y + pair[1].params.y - 1.0).abs() < 1e-5);
            assert!(pair[0].params.x < 0.35, "bombs keep the middle lane open");
        }
    }

    #[test]
    fn spikes_alternate_opposite_sides() {
        let analysis = synthetic_analysis(120.0, 48);
        for seed in 0..6 {
            let proposals = phrase_proposals(
                &analysis,
                &test_spec(3),
                Phrase::SpikeSides,
                &ctx(64, 4, SectionType::Main, 0.2, 0.7),
                seed,
            );
            assert_eq!(proposals.len(), 8);
            for pair in proposals.windows(2) {
                let (a, b) = (pair[0].params.variant % 4, pair[1].params.variant % 4);
                assert_eq!((a + 2) % 4, b, "opposite side answers");
            }
            assert!(
                proposals.iter().all(|p| p.params.variant >= 4),
                "early phrases comb"
            );
        }
    }

    #[test]
    fn sweep_cross_rotates_from_center_then_pushes_from_an_edge() {
        let analysis = synthetic_analysis(120.0, 48);
        let proposals = phrase_proposals(
            &analysis,
            &test_spec(5),
            Phrase::SweepCross,
            &ctx(64, 4, SectionType::Main, 0.9, 1.2),
            4,
        );
        let sweeps = only(&proposals, EventKind::LaserSweep);
        assert_eq!(sweeps.len(), 3);
        let (a, b, push) = (&sweeps[0].params, &sweeps[1].params, &sweeps[2].params);
        assert_eq!((a.x, a.y), (0.5, 0.5));
        assert_eq!(a.variant % 2, 0, "rotation mode");
        assert_ne!(a.variant, b.variant, "counter-rotating");
        assert!(((b.angle - a.angle).abs() - FRAC_PI_2).abs() < 1e-5);
        assert_eq!(push.variant, 1, "translation mode");
        assert!(push.x < 0.05 || push.x > 0.95 || push.y < 0.05 || push.y > 0.95);
    }

    #[test]
    fn finale_layers_phrases_in_the_last_main_stretch() {
        let analysis = real("surf-rock");
        let mut spec = test_spec(5);
        spec.finale = true;
        let plan = phrase_plan(&analysis, &spec, 7);
        let finale: Vec<&(i64, i64, Phrase)> = plan
            .iter()
            .filter(|(_, _, p)| *p == Phrase::Finale)
            .collect();
        assert!(finale.len() >= 2, "{plan:?}");
        let outro = analysis.sections.last().unwrap().start_beat;
        assert_eq!(
            finale.last().unwrap().1,
            outro,
            "finale runs up to the outro"
        );
        let start = finale[0].0;
        assert!(outro - start <= FINALE_BARS * 4);

        let chart = generate_chart(&analysis, &spec, 7);
        let kinds: std::collections::HashSet<EventKind> = chart
            .events
            .iter()
            .filter(|e| e.kind.is_hazard() && e.beat >= start as f64 && e.beat < outro as f64)
            .map(|e| e.kind)
            .collect();
        assert!(kinds.len() >= 4, "finale combines hazards: {kinds:?}");
        assert!(chart.events.iter().any(|e| e.kind == EventKind::Flash
            && e.beat == start as f64
            && e.params.intensity == 1.0));
        assert_eq!(
            chart
                .events
                .iter()
                .filter(|e| e.kind == EventKind::SpawnEnemy && e.beat >= start as f64)
                .count(),
            0,
            "no enemies during the finale"
        );
        spec.finale = false;
        assert!(
            phrase_plan(&analysis, &spec, 7)
                .iter()
                .all(|(_, _, p)| *p != Phrase::Finale)
        );
    }

    #[test]
    fn first_hazard_of_each_kind_gets_a_longer_telegraph() {
        let analysis = real("celtic");
        let spec = test_spec(3);
        let chart = generate_chart(&analysis, &spec, 2);
        for kind in EventKind::HAZARDS {
            let events: Vec<&ChartEvent> = chart.events.iter().filter(|e| e.kind == kind).collect();
            let Some(first) = events.first() else {
                continue;
            };
            let base = spec.pattern(kind).unwrap().telegraph_beats;
            assert_eq!(first.telegraph_beats, base + INTRODUCTION_BEATS, "{kind:?}");
            assert!(
                events[1..].iter().all(|e| e.telegraph_beats == base),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn enemies_arrive_on_phrase_boundaries_mostly_in_breakdowns() {
        for (name, analysis) in all_inputs() {
            let spec = test_spec(3);
            let plan = phrase_plan(&analysis, &spec, 6);
            let chart = generate_chart(&analysis, &spec, 6);
            let enemies: Vec<&ChartEvent> = chart
                .events
                .iter()
                .filter(|e| e.kind == EventKind::SpawnEnemy)
                .collect();
            assert!(!enemies.is_empty(), "{name}");
            for enemy in &enemies {
                assert_eq!(enemy.beat as i64 % 4, 0, "{name}: enemy off the bar line");
                let section = analysis.section_index_for_beat(enemy.beat as i64);
                assert!(matches!(
                    analysis.sections[section].section_type,
                    SectionType::Main | SectionType::Breakdown
                ));
                assert!(
                    plan.iter()
                        .any(|(s, e, _)| *s <= enemy.beat as i64 && (enemy.beat as i64) < *e),
                    "{name}"
                );
            }
            let per_bar = |section_type: SectionType| {
                let bars: i64 = analysis
                    .sections
                    .iter()
                    .filter(|s| s.section_type == section_type)
                    .map(|s| s.end_bar - s.start_bar)
                    .sum();
                let count = enemies
                    .iter()
                    .filter(|e| {
                        let s = analysis.section_index_for_beat(e.beat as i64);
                        analysis.sections[s].section_type == section_type
                    })
                    .count();
                (bars > 0).then(|| count as f32 / bars as f32)
            };
            if let (Some(breakdown), Some(main)) =
                (per_bar(SectionType::Breakdown), per_bar(SectionType::Main))
            {
                assert!(
                    breakdown > main,
                    "{name}: breakdown {breakdown} vs main {main}"
                );
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
