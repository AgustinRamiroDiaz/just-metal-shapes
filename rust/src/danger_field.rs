//! `DangerField`: once per physics frame, gathers `danger_shapes()` records from every
//! node in the `danger` group into a `core::danger::DangerSnapshot`.
//!
//! Rust callers (bots) use `snapshot()`; Godot callers use the query funcs.

use crate::core::danger::{DangerSnapshot, encode};
use crate::groups;
use crate::hazards::to_v2;
use godot::classes::{INode, Node};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base = Node)]
pub struct DangerField {
    snapshot: DangerSnapshot,
    base: Base<Node>,
}

#[godot_api]
impl DangerField {
    /// Re-gathers the snapshot now (normally done each physics frame).
    #[func]
    pub fn refresh(&mut self) {
        self.snapshot.clear();
        let tree = self.base().get_tree();
        for mut node in tree.get_nodes_in_group(groups::DANGER).iter_shared() {
            if node.is_queued_for_deletion() || !node.has_method("danger_shapes") {
                continue;
            }
            if let Ok(records) = node
                .call("danger_shapes", &[])
                .try_to::<PackedFloat32Array>()
            {
                self.snapshot.extend_from_records(records.as_slice());
            }
        }
    }

    /// Danger `0..=1` at `pos` (global px), `t_ahead` seconds from now.
    #[func]
    pub fn danger_at(&self, pos: Vector2, t_ahead: f32) -> f32 {
        self.snapshot.danger_at(to_v2(pos), t_ahead)
    }

    /// Distance to the nearest shape active at `t_ahead` (INF if none).
    #[func]
    pub fn min_distance(&self, pos: Vector2, t_ahead: f32) -> f32 {
        self.snapshot.min_distance(to_v2(pos), t_ahead)
    }

    #[func]
    pub fn is_hit(&self, pos: Vector2, radius: f32, t_ahead: f32) -> bool {
        self.snapshot.is_hit(to_v2(pos), radius, t_ahead)
    }

    #[func]
    pub fn shape_count(&self) -> i64 {
        self.snapshot.len() as i64
    }

    /// All current records, concatenated (`core::danger` layout).
    #[func]
    pub fn get_records(&self) -> PackedFloat32Array {
        PackedFloat32Array::from(encode(&self.snapshot.shapes).as_slice())
    }
}

impl DangerField {
    pub fn snapshot(&self) -> &DangerSnapshot {
        &self.snapshot
    }
}

#[godot_api]
impl INode for DangerField {
    fn physics_process(&mut self, _delta: f64) {
        self.refresh();
    }
}
