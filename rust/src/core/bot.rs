//! Bot decision logic over a `DangerSnapshot`.
//!
//! Stub: `decide` returns no movement. The bots workstream owns this module and may
//! extend `BotInput` (see `docs/design/complete-game-plan.md`, "Players and bots").

use super::danger::{DangerSnapshot, V2};

/// An enemy as a bot sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotEnemy {
    pub position: V2,
    /// Contact-damage radius (stay outside it).
    pub contact_radius: f32,
    /// Whether the enemy's active shield color matches this bot (it can damage it).
    pub shield_matches: bool,
}

/// A teammate as a bot sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BotTeammate {
    pub position: V2,
    pub is_dead: bool,
}

/// Everything `decide` needs for one physics frame.
#[derive(Clone, Copy, Debug)]
pub struct BotInput<'a> {
    pub position: V2,
    /// Player body radius (px).
    pub radius: f32,
    /// Max speed (px/s).
    pub speed: f32,
    /// Lightning range ring radius (px).
    pub range_radius: f32,
    /// Arena bounds in global pixels.
    pub arena_min: V2,
    pub arena_max: V2,
    pub danger: &'a DangerSnapshot,
    pub enemies: &'a [BotEnemy],
    pub teammates: &'a [BotTeammate],
    /// Per-bot deterministic seed (e.g. player index) for tie-breaking.
    pub seed: u64,
}

/// Move direction for this frame, length `0..=1`.
pub fn decide(_input: &BotInput) -> V2 {
    V2::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_stands_still() {
        let snapshot = DangerSnapshot::default();
        let input = BotInput {
            position: V2::new(640.0, 360.0),
            radius: 11.0,
            speed: 220.0,
            range_radius: 140.0,
            arena_min: V2::ZERO,
            arena_max: V2::new(1280.0, 720.0),
            danger: &snapshot,
            enemies: &[],
            teammates: &[],
            seed: 0,
        };
        assert_eq!(decide(&input), V2::ZERO);
    }
}
