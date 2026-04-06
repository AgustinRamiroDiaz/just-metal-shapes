class_name Targeting


static func get_nearest_alive(tree: SceneTree, origin: Vector2, group: StringName) -> Node2D:
	var nearest: Node2D = null
	var min_dist: float = INF
	for t in tree.get_nodes_in_group(group):
		if not is_instance_valid(t) or not t is Node2D:
			continue
		if t.get("is_dead") == true:
			continue
		var d := origin.distance_to(t.global_position)
		if d < min_dist:
			min_dist = d
			nearest = t as Node2D
	return nearest
