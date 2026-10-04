//! `LevelUi`: child of the level's `GameManager` that owns the in-level UI. It creates
//! the `Hud`, the `GymPanel` (gym only) and `PauseMenu`, applies the audio latency
//! setting to the Conductor, stops the menu music, and adds the `ResultsScreen` (named
//! `EndScreen`) when the level ends.

use super::gym::GymPanel;
use super::hud::Hud;
use super::pause::PauseMenu;
use super::results::ResultsScreen;
use crate::conductor::Conductor;
use crate::director::LevelDirector;
use crate::save::save_data;
use godot::classes::{INode, Node};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct LevelUi {
    shown_end: bool,
    base: Base<Node>,
}

#[godot_api]
impl LevelUi {
    #[func]
    fn _on_level_cleared(&mut self, _score: i64) {
        self.base_mut()
            .call_deferred("_show_results", &[true.to_variant()]);
    }

    #[func]
    fn _on_game_over(&mut self, _score: i64) {
        self.base_mut()
            .call_deferred("_show_results", &[false.to_variant()]);
    }

    #[func]
    fn _on_level_failed(&mut self) {
        self.base_mut()
            .call_deferred("_show_results_ex", &[false.to_variant(), true.to_variant()]);
    }

    #[func]
    fn _show_results(&mut self, cleared: bool) {
        self._show_results_ex(cleared, false);
    }

    #[func]
    fn _show_results_ex(&mut self, cleared: bool, load_failed: bool) {
        if self.shown_end {
            return;
        }
        let Some(mut manager) = self.base().get_parent() else {
            return;
        };
        self.shown_end = true;
        let stats = manager
            .call("get_run_stats", &[])
            .try_to::<VarDictionary>()
            .unwrap_or_default();
        let director = manager.try_get_node_as::<LevelDirector>("LevelDirector");
        let level = director
            .as_ref()
            .map(|d| d.bind().get_level_info())
            .unwrap_or_default();
        let mode = director.as_ref().map_or(1, |d| d.bind().get_mode());
        let mut results = ResultsScreen::new_alloc();
        results.set_name("EndScreen");
        {
            let mut r = results.bind_mut();
            r.stats = stats;
            r.level = level;
            r.cleared = cleared;
            r.mode = mode;
            r.load_failed = load_failed;
        }
        manager.add_child(&results);
    }
}

#[godot_api]
impl INode for LevelUi {
    fn ready(&mut self) {
        if let Some(mut ui) = super::services() {
            ui.bind_mut().stop_music();
        }
        let Some(mut manager) = self.base().get_parent() else {
            return;
        };
        if let (Some(mut conductor), Some(save)) = (
            manager.try_get_node_as::<Conductor>("Conductor"),
            save_data(&self.to_gd().upcast()),
        ) {
            conductor.bind_mut().latency_offset = save.bind().latency_offset_seconds();
        }
        let this = self.to_gd();
        manager.connect("level_cleared", &this.callable("_on_level_cleared"));
        manager.connect("game_over", &this.callable("_on_game_over"));
        manager.connect("level_failed", &this.callable("_on_level_failed"));

        let mut hud = Hud::new_alloc();
        hud.set_name("Hud");
        self.base_mut().add_child(&hud);
        if super::gym_mode() {
            let mut gym = GymPanel::new_alloc();
            gym.set_name("GymPanel");
            self.base_mut().add_child(&gym);
        }
        let mut pause = PauseMenu::new_alloc();
        pause.set_name("PauseMenu");
        self.base_mut().add_child(&pause);
    }
}
