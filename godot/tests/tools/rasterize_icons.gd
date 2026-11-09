## Rasterizes every SVG in --in to a square PNG of --size px in --out (used by
## devtools/fetch_icons.py). Headless and silent:
##   godot --headless --audio-driver Dummy --path godot \
##       -s res://tests/tools/rasterize_icons.gd -- --in=DIR --out=DIR --size=128
extends SceneTree


func _initialize() -> void:
	var in_dir := ""
	var out_dir := ""
	var size := 128
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--in="):
			in_dir = arg.trim_prefix("--in=")
		elif arg.begins_with("--out="):
			out_dir = arg.trim_prefix("--out=")
		elif arg.begins_with("--size="):
			size = int(arg.trim_prefix("--size="))
	var code := 0
	for file in DirAccess.get_files_at(in_dir):
		if not file.ends_with(".svg"):
			continue
		var svg := FileAccess.get_file_as_string(in_dir.path_join(file))
		var probe := Image.new()
		probe.load_svg_from_string(svg, 1.0)
		var image := Image.new()
		if image.load_svg_from_string(svg, float(size) / probe.get_width()) != OK:
			push_error("could not rasterize %s" % file)
			code = 1
			continue
		var path := out_dir.path_join(file.get_basename() + ".png")
		image.save_png(path)
		print("icon: ", path, " ", image.get_size())
	quit(code)
