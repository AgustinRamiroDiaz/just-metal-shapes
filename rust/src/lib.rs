use godot::prelude::*;

pub mod core;

pub mod arena;
mod bot_brain;
mod color_utils;
pub mod conductor;
pub mod danger_field;
pub mod director;
mod enemy;
pub mod enemy_spawn;
pub mod fx;
mod game_config;
pub mod groups;
pub mod hazards;
pub mod level_catalog;
mod manager;
mod menu;
pub mod player;
mod projectile;
mod state_machine;
mod targeting;
pub mod util;
pub mod visuals;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
