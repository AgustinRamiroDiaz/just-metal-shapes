use godot::prelude::*;

mod color_utils;
mod enemy;
mod game_config;
mod manager;
mod menu;
mod player;
mod projectile;
mod spawner;
mod state_machine;
mod targeting;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
