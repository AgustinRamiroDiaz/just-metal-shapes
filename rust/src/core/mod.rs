//! Pure game logic with no Godot types, covered by `cargo test`.
//!
//! Nothing in this module tree may import `godot`, spawn threads, or read the system
//! clock (`std::time::Instant`), so it builds unchanged for the no-thread web target.

pub mod analysis;
pub mod bot;
pub mod chart;
pub mod chart_gen;
pub mod credits;
pub mod danger;
pub mod feel;
pub mod mode;
pub mod rng;
pub mod save_model;
pub mod scoring;
pub mod timing;

#[cfg(test)]
pub(crate) mod test_support;
