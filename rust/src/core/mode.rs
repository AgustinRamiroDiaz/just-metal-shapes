//! Player-selected difficulty mode (orthogonal to a level's 1-5 difficulty).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DifficultyMode {
    Casual,
    #[default]
    Normal,
    Hardcore,
}

impl DifficultyMode {
    /// Integer used by `GameConfig.difficulty_mode` (CASUAL=0, NORMAL=1, HARDCORE=2).
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => DifficultyMode::Casual,
            2 => DifficultyMode::Hardcore,
            _ => DifficultyMode::Normal,
        }
    }

    pub fn as_i32(self) -> i32 {
        match self {
            DifficultyMode::Casual => 0,
            DifficultyMode::Normal => 1,
            DifficultyMode::Hardcore => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DifficultyMode::Casual => "casual",
            DifficultyMode::Normal => "normal",
            DifficultyMode::Hardcore => "hardcore",
        }
    }

    /// Multiplier applied to `LevelSpec::density` before chart generation.
    pub fn density_scale(self) -> f32 {
        match self {
            DifficultyMode::Casual => 0.65,
            DifficultyMode::Normal => 1.0,
            DifficultyMode::Hardcore => 1.2,
        }
    }

    /// Whether all-players-down rewinds to the last checkpoint (otherwise game over).
    pub fn allows_rewind(self) -> bool {
        self != DifficultyMode::Hardcore
    }

    pub fn score_multiplier(self) -> f64 {
        match self {
            DifficultyMode::Casual => 0.5,
            DifficultyMode::Normal => 1.0,
            DifficultyMode::Hardcore => 1.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_round_trip_and_rules() {
        for mode in [
            DifficultyMode::Casual,
            DifficultyMode::Normal,
            DifficultyMode::Hardcore,
        ] {
            assert_eq!(DifficultyMode::from_i32(mode.as_i32()), mode);
        }
        assert_eq!(DifficultyMode::from_i32(77), DifficultyMode::Normal);
        assert!(!DifficultyMode::Hardcore.allows_rewind());
        assert!(DifficultyMode::Casual.allows_rewind());
        assert!(DifficultyMode::Casual.density_scale() < DifficultyMode::Hardcore.density_scale());
    }
}
