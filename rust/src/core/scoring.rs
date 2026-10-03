//! Team score and rank from a finished (or failed) run.

use super::mode::DifficultyMode;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunStats {
    /// Song seconds survived (the full song length on a clear).
    pub seconds_survived: f64,
    pub song_duration: f64,
    pub completed: bool,
    pub players: u32,
    /// Total hits taken across all players.
    pub hits_taken: u32,
    /// Times a player went down (lives reached 0).
    pub downs: u32,
    pub revives: u32,
    pub rewinds: u32,
    pub enemies_killed: u32,
    /// Level difficulty, 1-5.
    pub difficulty: u8,
    pub mode: DifficultyMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    S,
    A,
    B,
    C,
    D,
}

impl Rank {
    pub fn as_str(self) -> &'static str {
        match self {
            Rank::S => "S",
            Rank::A => "A",
            Rank::B => "B",
            Rank::C => "C",
            Rank::D => "D",
        }
    }
}

pub const POINTS_PER_SECOND: f64 = 10.0;
pub const POINTS_PER_ENEMY: f64 = 100.0;
pub const POINTS_PER_REVIVE: f64 = 150.0;
pub const CLEAR_BONUS: f64 = 5000.0;
pub const HIT_PENALTY: f64 = 50.0;
pub const REWIND_PENALTY: f64 = 1000.0;

/// Hits per player per minute of song; the main skill measure for ranks.
pub fn hit_rate(stats: &RunStats) -> f64 {
    let minutes = (stats.seconds_survived / 60.0).max(0.25);
    stats.hits_taken as f64 / stats.players.max(1) as f64 / minutes
}

pub fn score(stats: &RunStats) -> i64 {
    let difficulty = stats.difficulty.clamp(1, 5) as f64;
    let raw = stats.seconds_survived.max(0.0) * POINTS_PER_SECOND
        + stats.enemies_killed as f64 * POINTS_PER_ENEMY
        + stats.revives as f64 * POINTS_PER_REVIVE
        + if stats.completed { CLEAR_BONUS } else { 0.0 }
        - stats.hits_taken as f64 * HIT_PENALTY
        - stats.rewinds as f64 * REWIND_PENALTY;
    let multiplier = (1.0 + 0.25 * (difficulty - 1.0)) * stats.mode.score_multiplier();
    (raw.max(0.0) * multiplier).round() as i64
}

/// S: clear, no rewinds, under 1 hit per player-minute. A: clear, no rewinds, under 3.
/// B: clear with at most one rewind. C: any other clear. D: not cleared.
pub fn rank(stats: &RunStats) -> Rank {
    if !stats.completed {
        return Rank::D;
    }
    let rate = hit_rate(stats);
    match stats.rewinds {
        0 if rate < 1.0 => Rank::S,
        0 if rate < 3.0 => Rank::A,
        0 | 1 => Rank::B,
        _ => Rank::C,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_run() -> RunStats {
        RunStats {
            seconds_survived: 120.0,
            song_duration: 120.0,
            completed: true,
            players: 2,
            difficulty: 1,
            ..RunStats::default()
        }
    }

    #[test]
    fn perfect_clear_is_s() {
        let stats = clear_run();
        assert_eq!(rank(&stats), Rank::S);
        assert_eq!(score(&stats), 1200 + 5000);
    }

    #[test]
    fn ranks_degrade_with_hits_and_rewinds() {
        let mut stats = clear_run();
        stats.hits_taken = 8; // 2 per player-minute
        assert_eq!(rank(&stats), Rank::A);
        stats.hits_taken = 20;
        assert_eq!(rank(&stats), Rank::B);
        stats.rewinds = 1;
        assert_eq!(rank(&stats), Rank::B);
        stats.rewinds = 3;
        assert_eq!(rank(&stats), Rank::C);
        stats.completed = false;
        assert_eq!(rank(&stats), Rank::D);
        assert!(Rank::S < Rank::D);
    }

    #[test]
    fn score_rewards_and_penalizes() {
        let base = score(&clear_run());
        let mut stats = clear_run();
        stats.enemies_killed = 3;
        stats.revives = 1;
        assert_eq!(score(&stats), base + 300 + 150);
        stats.rewinds = 100;
        assert_eq!(score(&stats), 0);
    }

    #[test]
    fn multipliers_apply() {
        let mut stats = clear_run();
        let normal = score(&stats);
        stats.difficulty = 5;
        assert_eq!(score(&stats), normal * 2);
        stats.difficulty = 1;
        stats.mode = DifficultyMode::Hardcore;
        assert_eq!(score(&stats), (normal as f64 * 1.5).round() as i64);
        stats.mode = DifficultyMode::Casual;
        assert!(score(&stats) < normal);
    }

    #[test]
    fn short_runs_do_not_explode_hit_rate() {
        let stats = RunStats {
            seconds_survived: 0.0,
            players: 0,
            hits_taken: 1,
            ..RunStats::default()
        };
        assert!(hit_rate(&stats).is_finite());
        assert_eq!(rank(&stats), Rank::D);
    }
}
