use godot::prelude::*;

mod projectile;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
