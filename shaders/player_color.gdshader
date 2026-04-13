shader_type canvas_item;

uniform vec4 player_color : source_color = vec4(1.0);

void fragment() {
	vec4 tex = texture(TEXTURE, UV);
	float lum = dot(tex.rgb, vec3(0.299, 0.587, 0.114));
	COLOR = vec4(player_color.rgb * lum * 2.0, tex.a);
}
