//! Scene-tree group names shared across systems.

/// Every chart hazard node (cleared on rewind, level end).
pub const HAZARDS: &str = "hazards";
/// Anything that implements `danger_shapes()`; gathered by `DangerField` each physics
/// frame. Hazards, enemy projectiles, mines and contact-damage enemies join it.
pub const DANGER: &str = "danger";
pub const PLAYERS: &str = "players";
pub const ENEMIES: &str = "enemies";
pub const ENEMY_PROJECTILES: &str = "enemy_projectiles";
pub const MINES: &str = "mines";
/// Pending enemy spawn effects (freeing one cancels its enemy).
pub const SPAWN_EFFECTS: &str = "spawn_effects";
/// The level's `Conductor` (enemies read song time from it).
pub const CONDUCTOR: &str = "conductor";
