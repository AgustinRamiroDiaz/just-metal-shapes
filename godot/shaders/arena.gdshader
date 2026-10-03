// Arena background: level-palette gradient, scrolling grid, a slowly turning hexagon
// motif, beat/bar pulses and a vignette. One full-screen pass driven by the Arena node.
shader_type canvas_item;

uniform vec4 bg_color : source_color = vec4(0.04, 0.05, 0.09, 1.0);
uniform vec4 accent_color : source_color = vec4(0.3, 0.85, 1.0, 1.0);
// 0..1 section energy: brightness, grid strength.
uniform float energy = 0.5;
// Bar downbeat kick (0..1, decays within the beat).
uniform float bar_pulse = 0.0;
// Every-beat kick (0..1, decays faster).
uniform float beat_pulse = 0.0;
uniform vec2 grid_offset = vec2(0.0);
uniform float grid_size = 64.0;
uniform vec2 arena_origin = vec2(0.0);
uniform vec2 arena_size = vec2(1280.0, 720.0);
// Rotation of the hexagon motif in radians.
uniform float motif_angle = 0.0;

varying vec2 world;

void vertex() {
	world = VERTEX;
}

float hexagon(vec2 p, float r) {
	const vec3 k = vec3(-0.866025404, 0.5, 0.577350269);
	p = abs(p);
	p -= 2.0 * min(dot(k.xy, p), 0.0) * k.xy;
	p -= vec2(clamp(p.x, -k.z * r, k.z * r), r);
	return length(p) * sign(p.y);
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
	float kick = bar_pulse + 0.35 * beat_pulse;

	// Base: palette background lifted toward the center.
	vec3 bg = bg_color.rgb;
	vec3 accent = accent_color.rgb;
	vec3 col = bg * (0.85 + 0.5 * energy);
	col += accent * (0.025 + 0.05 * energy + 0.05 * kick) * (1.0 - smoothstep(0.0, 0.85, r));

	// Grid, zoomed slightly by the bar pulse around the center.
	float zoom = 1.0 + 0.035 * bar_pulse;
	vec2 gp = p / zoom + grid_offset;
	float minor = grid_lines(gp, grid_size, 1.0);
	float major = grid_lines(gp, grid_size * 4.0, 2.0);
	float grid_fade = 1.0 - smoothstep(0.25, 0.95, r);
	float grid_alpha = (0.035 + 0.06 * energy + 0.12 * kick) * grid_fade;
	col += accent * (minor * grid_alpha + major * grid_alpha * 1.6);

	// Hexagon motif: three nested outlines, breathing with the beat.
	vec2 hp = n;
	float ca = cos(motif_angle);
	float sa = sin(motif_angle);
	hp = vec2(ca * hp.x - sa * hp.y, sa * hp.x + ca * hp.y);
	float motif = 0.0;
	for (int i = 0; i < 3; i++) {
		float rad = (0.16 + 0.13 * float(i)) * (1.0 + 0.06 * kick);
		float d = abs(hexagon(hp, rad));
		motif += (1.0 - smoothstep(0.0015, 0.0045, d)) * (0.5 - 0.12 * float(i));
	}
	col += accent * motif * (0.05 + 0.10 * energy + 0.25 * kick);

	// Bar shockwave: a soft ring expanding from the center as the pulse decays.
	float ring_r = (1.0 - bar_pulse) * 1.1;
	float ring = exp(-abs(r - ring_r) * 28.0) * bar_pulse;
	col += accent * ring * 0.22;

	// Vignette.
	float vig = smoothstep(0.35, 1.05, r);
	col *= 1.0 - 0.8 * vig;

	COLOR = vec4(col, 1.0);
}
