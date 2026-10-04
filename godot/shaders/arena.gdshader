// Arena background: level-palette gradient, a slowly drifting grid, a soft bar glow and
// a vignette. Kept calm so hazards and enemies are what moves. One full-screen pass
// driven by the Arena node.
shader_type canvas_item;

uniform vec4 bg_color : source_color = vec4(0.04, 0.05, 0.09, 1.0);
uniform vec4 accent_color : source_color = vec4(0.3, 0.85, 1.0, 1.0);
// 0..1 section energy: brightness, grid strength.
uniform float energy = 0.5;
// Bar downbeat glow (0..1, decays within the beat).
uniform float bar_pulse = 0.0;
uniform vec2 grid_offset = vec2(0.0);
uniform float grid_size = 64.0;
uniform vec2 arena_origin = vec2(0.0);
uniform vec2 arena_size = vec2(1280.0, 720.0);

varying vec2 world;

void vertex() {
	world = VERTEX;
}

float grid_lines(vec2 p, float cell, float width) {
	vec2 dist = (0.5 - abs(fract(p / cell) - 0.5)) * cell;
	return 1.0 - smoothstep(width * 0.5, width * 0.5 + 1.0, min(dist.x, dist.y));
}

void fragment() {
	vec2 center = arena_origin + arena_size * 0.5;
	vec2 p = world - center;
	vec2 n = p / arena_size.y; // height-normalized, aspect-correct
	float r = length(n);

	// Base: palette background lifted toward the center, a little more on the bar.
	vec3 bg = bg_color.rgb;
	vec3 accent = accent_color.rgb;
	vec3 col = bg * (0.85 + 0.5 * energy);
	col += accent * (0.025 + 0.05 * energy + 0.02 * bar_pulse) * (1.0 - smoothstep(0.0, 0.85, r));

	vec2 gp = p + grid_offset;
	float minor = grid_lines(gp, grid_size, 1.0);
	float major = grid_lines(gp, grid_size * 4.0, 2.0);
	float grid_fade = 1.0 - smoothstep(0.25, 0.95, r);
	float grid_alpha = (0.03 + 0.04 * energy + 0.02 * bar_pulse) * grid_fade;
	col += accent * (minor * grid_alpha + major * grid_alpha * 1.6);

	// Vignette.
	float vig = smoothstep(0.35, 1.05, r);
	col *= 1.0 - 0.8 * vig;

	COLOR = vec4(col, 1.0);
}
