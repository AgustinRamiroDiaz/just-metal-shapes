// Player body: recolors the grayscale-lit Kenney body to the team color.
// `flash` (0..1) blends to white for hit feedback; `ghost` (0..1) desaturates the
// downed body toward a pale tint.
shader_type canvas_item;

uniform vec4 player_color : source_color = vec4(1.0);
uniform float flash : hint_range(0.0, 1.0) = 0.0;
uniform float ghost : hint_range(0.0, 1.0) = 0.0;

void fragment() {
	vec4 tex = texture(TEXTURE, UV);
	float lum = dot(tex.rgb, vec3(0.299, 0.587, 0.114));
	vec3 body = player_color.rgb * lum * 2.0;
	body = mix(body, vec3(lum * 1.4) * mix(player_color.rgb, vec3(1.0), 0.6), ghost);
	body = mix(body, vec3(1.0), flash);
	COLOR = vec4(body, tex.a * COLOR.a);
}
