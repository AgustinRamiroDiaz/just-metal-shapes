class_name GameConfig


class PlayerConfig:
	var input_type: int
	var color: Color

	func _init(t: int, c: Color) -> void:
		input_type = t
		color = c


enum InputType {
	KEYBOARD1,
	KEYBOARD2,
	GAMEPAD_LEFT_0,
	GAMEPAD_LEFT_1,
	GAMEPAD_LEFT_2,
	GAMEPAD_LEFT_3,
	GAMEPAD_LEFT_4,
	GAMEPAD_LEFT_5,
	GAMEPAD_LEFT_6,
	GAMEPAD_LEFT_7,
	GAMEPAD_RIGHT_0,
	GAMEPAD_RIGHT_1,
	GAMEPAD_RIGHT_2,
	GAMEPAD_RIGHT_3,
	GAMEPAD_RIGHT_4,
	GAMEPAD_RIGHT_5,
	GAMEPAD_RIGHT_6,
	GAMEPAD_RIGHT_7,
}

const PLAYER_COLORS := [
	Color(0.35, 0.75, 1.0),
	Color(1.0, 0.6, 0.2),
	Color(0.4, 0.9, 0.3),
	Color(0.9, 0.3, 0.9),
	Color(1.0, 0.9, 0.15),
	Color(0.9, 0.3, 0.3),
	Color(0.4, 0.9, 0.85),
	Color(1.0, 0.6, 0.75),
]

const INPUT_LABELS := {
	InputType.KEYBOARD1:       "KB: WASD/Arrows",
	InputType.KEYBOARD2:       "KB: IJKL",
	InputType.GAMEPAD_LEFT_0:  "Pad 1 Left Stick",
	InputType.GAMEPAD_LEFT_1:  "Pad 2 Left Stick",
	InputType.GAMEPAD_LEFT_2:  "Pad 3 Left Stick",
	InputType.GAMEPAD_LEFT_3:  "Pad 4 Left Stick",
	InputType.GAMEPAD_LEFT_4:  "Pad 5 Left Stick",
	InputType.GAMEPAD_LEFT_5:  "Pad 6 Left Stick",
	InputType.GAMEPAD_LEFT_6:  "Pad 7 Left Stick",
	InputType.GAMEPAD_LEFT_7:  "Pad 8 Left Stick",
	InputType.GAMEPAD_RIGHT_0: "Pad 1 Right Stick",
	InputType.GAMEPAD_RIGHT_1: "Pad 2 Right Stick",
	InputType.GAMEPAD_RIGHT_2: "Pad 3 Right Stick",
	InputType.GAMEPAD_RIGHT_3: "Pad 4 Right Stick",
	InputType.GAMEPAD_RIGHT_4: "Pad 5 Right Stick",
	InputType.GAMEPAD_RIGHT_5: "Pad 6 Right Stick",
	InputType.GAMEPAD_RIGHT_6: "Pad 7 Right Stick",
	InputType.GAMEPAD_RIGHT_7: "Pad 8 Right Stick",
}

static var players: Array = []
