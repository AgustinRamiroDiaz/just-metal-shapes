// VHS-style rewind overlay drawn over the arena by Fx.rewind_effect().
// `amount` 1 = full effect, 0 = untouched screen.
shader_type canvas_item;

uniform float amount : hint_range(0.0, 1.0) = 0.0;
uniform sampler2D screen_tex : hint_screen_texture, filter_linear;

float hash(vec2 p) {
	return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

void fragment() {
	vec2 uv = SCREEN_UV;
	float t = TIME;

	// Tracking bands rolling upward, plus a fine horizontal wobble.
	float band_pos = fract(uv.y * 2.0 + t * 1.3);
	float band = smoothstep(0.0, 0.04, band_pos) * (1.0 - smoothstep(0.04, 0.09, band_pos));
	float wobble = sin(uv.y * 160.0 + t * 50.0) * 0.0015;
	uv.x += (wobble + band * 0.03) * amount;

	float ca = 0.007 * amount;
	vec3 c;
	c.r = texture(screen_tex, uv + vec2(ca, 0.0)).r;
	c.g = texture(screen_tex, uv).g;
	c.b = texture(screen_tex, uv - vec2(ca, 0.0)).b;

	// Desaturate toward a cold tape tint.
	float lum = dot(c, vec3(0.299, 0.587, 0.114));
	c = mix(c, vec3(lum) * vec3(0.8, 0.92, 1.15), amount * 0.85);

	float scan = 0.5 + 0.5 * sin(SCREEN_UV.y * 1080.0);
	c *= 1.0 - amount * 0.28 * scan;
	c += band * amount * 0.12;

	float n = hash(floor(SCREEN_UV * vec2(320.0, 180.0)) + floor(t * 30.0));
	c += (n - 0.5) * 0.07 * amount;

	COLOR = vec4(c, 1.0);
}
