use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct StateMachine {
    #[var]
    state: i32,
    #[var]
    transitions: Dictionary<i32, Variant>, // Variant should be Array<i32>

    base: Base<RefCounted>,
}

#[godot_api]
impl StateMachine {
    #[signal]
    fn state_changed(from: i32, to: i32);

    #[func]
    pub fn init_machine(&mut self, initial_state: i32, transitions: Dictionary<i32, Variant>) {
        self.state = initial_state;
        self.transitions = transitions;
    }

    #[func]
    pub fn get_current_state(&self) -> i32 {
        self.state
    }

    #[func]
    pub fn transition(&mut self, next: i32) -> bool {
        if self.state == next {
            return false;
        }

        let allowed = if let Some(a) = self.transitions.get(self.state) {
            a.try_to::<Array<i32>>().unwrap_or_default()
        } else {
            Array::new()
        };

        let mut is_allowed = false;
        for a in allowed.iter_shared() {
            if a == next {
                is_allowed = true;
                break;
            }
        }

        if !is_allowed {
            return false;
        }

        let prev = self.state;
        self.state = next;
        self.base_mut()
            .emit_signal("state_changed", &[prev.to_variant(), next.to_variant()]);
        true
    }

    #[func]
    pub fn force(&mut self, next: i32) {
        if self.state == next {
            return;
        }
        let prev = self.state;
        self.state = next;
        self.base_mut()
            .emit_signal("state_changed", &[prev.to_variant(), next.to_variant()]);
    }
}
