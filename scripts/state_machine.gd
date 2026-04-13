class_name StateMachine

## A generic graph-based state machine.
## States are plain ints (pass any enum value).
## Transitions are validated against an allowed-transitions dictionary.
## Use transition() for normal event-driven changes (returns false if blocked).
## Use force() for intentional overrides that bypass the graph (e.g. revive).

signal state_changed(from: int, to: int)

var _state: int
var _transitions: Dictionary  # int -> Array[int]


func _init(initial_state: int, transitions: Dictionary) -> void:
	_state = initial_state
	_transitions = transitions


func get_state() -> int:
	return _state


## Attempt a transition. Returns false (no-op) if not allowed by the graph.
func transition(next: int) -> bool:
	if _state == next:
		return false
	var allowed: Array = _transitions.get(_state, [])
	if next not in allowed:
		return false
	var prev := _state
	_state = next
	state_changed.emit(prev, next)
	return true


## Force a state regardless of the transition graph.
## Use only for intentional resets (e.g. revival, scene init).
func force(next: int) -> void:
	if _state == next:
		return
	var prev := _state
	_state = next
	state_changed.emit(prev, next)
