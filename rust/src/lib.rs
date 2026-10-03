use godot::prelude::*;

pub mod core;

mod color_utils;
pub mod conductor;
pub mod danger_field;
pub mod director;
mod enemy;
pub mod enemy_spawn;
mod game_config;
pub mod groups;
pub mod hazards;
pub mod level_catalog;
mod manager;
mod menu;
mod player;
mod projectile;
mod state_machine;
mod targeting;
pub mod util;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
