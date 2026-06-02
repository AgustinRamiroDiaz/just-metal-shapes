use godot::prelude::*;

#[derive(GodotClass)]
#[class(init, base = Object)]
struct ColorUtils;

#[godot_api]
impl ColorUtils {
    #[func]
    pub fn colors_match(c1: Color, c2: Color) -> bool {
        c1.r == c2.r && c1.g == c2.g && c1.b == c2.b
    }
}
