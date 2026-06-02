use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base = Object)]
struct Targeting;

#[godot_api]
impl Targeting {
    #[func]
    pub fn get_nearest_alive(
        tree: Gd<SceneTree>,
        origin: Vector2,
        group: StringName,
    ) -> Option<Gd<Node2D>> {
        let mut nearest: Option<Gd<Node2D>> = None;
        let mut min_dist = f32::INFINITY;

        for node in tree.get_nodes_in_group(&group).iter_shared() {
            if let Ok(t) = node.try_cast::<Node2D>() {
                if !t.is_instance_valid() {
                    continue;
                }

                // Check if t has "is_dead" and it's true
                let is_dead = t.get("is_dead");
                if is_dead.try_to::<bool>().unwrap_or(false) {
                    continue;
                }

                let d = origin.distance_to(t.get_global_position());
                if d < min_dist {
                    min_dist = d;
                    nearest = Some(t);
                }
            }
        }
        nearest
    }
}
