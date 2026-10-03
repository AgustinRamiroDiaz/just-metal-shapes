// Enemy body: maps the flat-shaded Kenney simple-space sprites onto a cool metal ramp
// (the enemy family look, distinct from player colors and hazard accents).
// `flash` (0..1) blends to white for damage feedback.
shader_type canvas_item;

uniform vec4 shadow_color : source_color = vec4(0.28, 0.30, 0.42, 1.0);
uniform vec4 light_color : source_color = vec4(0.94, 0.95, 1.0, 1.0);
uniform float flash : hint_range(0.0, 1.0) = 0.0;

void fragment() {
	vec4 tex = texture(TEXTURE, UV);
	float lum = dot(tex.rgb, vec3(0.299, 0.587, 0.114));
	vec3 metal = mix(shadow_color.rgb, light_color.rgb, smoothstep(0.78, 1.0, lum));
	metal = mix(metal, vec3(1.0), flash);
	COLOR = vec4(metal, tex.a * COLOR.a);
}
