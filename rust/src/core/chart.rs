//! The schedule a level plays: timed events produced by `chart_gen` and consumed by
//! the `LevelDirector`.

use serde::{Deserialize, Serialize};

use super::timing::Timing;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventKind {
    // Hazards (JSB-like).
    Laser,
    LaserSweep,
    BulletRing,
    Spiral,
    Wall,
    Pulse,
    Bomb,
    Spikes,
    Barrage,
    // Co-op enemies (existing scenes).
    SpawnEnemy,
    // Presentation.
    ArenaPulse,
    CameraKick,
    Flash,
    PaletteShift,
    Checkpoint,
    ShowHint,
}

impl EventKind {
    pub const ALL: [EventKind; 16] = [
        EventKind::Laser,
        EventKind::LaserSweep,
        EventKind::BulletRing,
        EventKind::Spiral,
        EventKind::Wall,
        EventKind::Pulse,
        EventKind::Bomb,
        EventKind::Spikes,
        EventKind::Barrage,
        EventKind::SpawnEnemy,
        EventKind::ArenaPulse,
        EventKind::CameraKick,
        EventKind::Flash,
        EventKind::PaletteShift,
        EventKind::Checkpoint,
        EventKind::ShowHint,
    ];

    pub const HAZARDS: [EventKind; 9] = [
        EventKind::Laser,
        EventKind::LaserSweep,
        EventKind::BulletRing,
        EventKind::Spiral,
        EventKind::Wall,
        EventKind::Pulse,
        EventKind::Bomb,
        EventKind::Spikes,
        EventKind::Barrage,
    ];

    /// Stable name used across the Godot boundary (signals, dictionaries).
    pub fn name(self) -> &'static str {
        match self {
            EventKind::Laser => "Laser",
            EventKind::LaserSweep => "LaserSweep",
            EventKind::BulletRing => "BulletRing",
            EventKind::Spiral => "Spiral",
            EventKind::Wall => "Wall",
            EventKind::Pulse => "Pulse",
            EventKind::Bomb => "Bomb",
            EventKind::Spikes => "Spikes",
            EventKind::Barrage => "Barrage",
            EventKind::SpawnEnemy => "SpawnEnemy",
            EventKind::ArenaPulse => "ArenaPulse",
            EventKind::CameraKick => "CameraKick",
            EventKind::Flash => "Flash",
            EventKind::PaletteShift => "PaletteShift",
            EventKind::Checkpoint => "Checkpoint",
            EventKind::ShowHint => "ShowHint",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    pub fn is_hazard(self) -> bool {
        Self::HAZARDS.contains(&self)
    }

    /// Events that only drive visuals/UI; they never hurt anyone.
    pub fn is_presentation(self) -> bool {
        !self.is_hazard() && self != EventKind::SpawnEnemy
    }
}

/// Generic, schema-free parameters every event carries. Each hazard interprets the
/// fields it needs and ignores the rest, so new hazards need no schema change.
///
/// Units:
/// - `x`, `y`: normalized arena position, `0..=1` (0,0 = top-left).
/// - `angle`: radians, 0 = +x (right), clockwise on screen (Godot's y-down).
/// - `size`: fraction of the arena height (720 px at 1280x720).
/// - `speed`: arena heights per second.
/// - `duration_beats`: how long the hazard stays dangerous after its hit beat.
/// - `color_index`: index into the level palette / player colors (hazard decides).
/// - `variant`: hazard-specific sub-pattern; for `SpawnEnemy` the `enemy_pool` index,
///   for `Checkpoint` the section index, for `ShowHint` the `HINTS` index.
/// - `intensity`: musical strength of the hit (`0..=1`, from the beat accent or bar
///   intensity). Presentation events scale their effect by it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternParams {
    pub x: f32,
    pub y: f32,
    pub angle: f32,
    pub count: u32,
    pub speed: f32,
    pub size: f32,
    pub duration_beats: f64,
    pub color_index: u32,
    pub variant: u32,
    pub intensity: f32,
}

impl Default for PatternParams {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 0.5,
            angle: 0.0,
            count: 1,
            speed: 0.0,
            size: 0.0,
            duration_beats: 0.0,
            color_index: 0,
            variant: 0,
            intensity: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChartEvent {
    /// When the event becomes dangerous (the "hit" beat).
    pub beat: f64,
    /// Warning lead time before `beat`; the director spawns at `beat - telegraph_beats`.
    pub telegraph_beats: f64,
    pub kind: EventKind,
    pub params: PatternParams,
}

impl ChartEvent {
    pub fn spawn_beat(&self) -> f64 {
        self.beat - self.telegraph_beats
    }

    /// Beat at which the event stops being dangerous.
    pub fn end_beat(&self) -> f64 {
        self.beat + self.params.duration_beats
    }
}

/// UI text for `ShowHint` events, indexed by `PatternParams::variant`.
pub const HINTS: [&str; 4] = [
    "Dodge the shapes - outlines warn you before they hit",
    "Stay close to enemies so your lightning hits them",
    "Shields only break to the player of the same color",
    "Downed teammates revive if you stand next to them",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    pub bpm: f64,
    pub offset_seconds: f64,
    pub duration_seconds: f64,
    /// Sorted by `beat` (hit beat).
    pub events: Vec<ChartEvent>,
}

impl Chart {
    pub fn timing(&self) -> Timing {
        Timing::new(self.bpm, self.offset_seconds)
    }

    /// Event indices ordered by spawn beat (`beat - telegraph_beats`), ties broken by
    /// chart order. This is the order the director plays events in.
    pub fn spawn_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.events.len()).collect();
        order.sort_by(|&a, &b| {
            self.events[a]
                .spawn_beat()
                .total_cmp(&self.events[b].spawn_beat())
                .then(a.cmp(&b))
        });
        order
    }

    /// Beats of every `Checkpoint` event, ascending.
    pub fn checkpoint_beats(&self) -> Vec<f64> {
        self.events
            .iter()
            .filter(|event| event.kind == EventKind::Checkpoint)
            .map(|event| event.beat)
            .collect()
    }

    /// Latest checkpoint at or before `beat` (0.0 if none).
    pub fn checkpoint_at_or_before(&self, beat: f64) -> f64 {
        self.checkpoint_beats()
            .into_iter()
            .take_while(|checkpoint| *checkpoint <= beat + 1e-6)
            .last()
            .unwrap_or(0.0)
    }

    pub fn count_kind(&self, kind: EventKind) -> usize {
        self.events
            .iter()
            .filter(|event| event.kind == kind)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(beat: f64, telegraph: f64, kind: EventKind) -> ChartEvent {
        ChartEvent {
            beat,
            telegraph_beats: telegraph,
            kind,
            params: PatternParams::default(),
        }
    }

    #[test]
    fn names_round_trip() {
        for kind in EventKind::ALL {
            assert_eq!(EventKind::from_name(kind.name()), Some(kind));
        }
        assert_eq!(EventKind::from_name("Nope"), None);
        assert!(EventKind::Pulse.is_hazard());
        assert!(!EventKind::SpawnEnemy.is_hazard());
        assert!(!EventKind::SpawnEnemy.is_presentation());
        assert!(EventKind::Checkpoint.is_presentation());
    }

    #[test]
    fn spawn_order_uses_telegraph() {
        let chart = Chart {
            bpm: 120.0,
            offset_seconds: 0.0,
            duration_seconds: 10.0,
            events: vec![
                event(4.0, 0.0, EventKind::ArenaPulse),
                event(5.0, 2.0, EventKind::Pulse),
                event(6.0, 4.0, EventKind::Laser),
            ],
        };
        assert_eq!(chart.spawn_order(), vec![2, 1, 0]);
    }

    #[test]
    fn finds_checkpoints() {
        let chart = Chart {
            bpm: 120.0,
            offset_seconds: 0.0,
            duration_seconds: 60.0,
            events: vec![
                event(0.0, 0.0, EventKind::Checkpoint),
                event(16.0, 0.0, EventKind::Checkpoint),
                event(48.0, 0.0, EventKind::Checkpoint),
            ],
        };
        assert_eq!(chart.checkpoint_beats(), vec![0.0, 16.0, 48.0]);
        assert_eq!(chart.checkpoint_at_or_before(15.9), 0.0);
        assert_eq!(chart.checkpoint_at_or_before(16.0), 16.0);
        assert_eq!(chart.checkpoint_at_or_before(100.0), 48.0);
        assert_eq!(chart.count_kind(EventKind::Checkpoint), 3);
    }
}
