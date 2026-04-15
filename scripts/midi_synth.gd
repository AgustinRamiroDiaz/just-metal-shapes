class_name MidiSynth
extends AudioStreamPlayer

const SAMPLE_RATE := 44100.0

# note_number -> phase (0..1)
var _active_notes: Dictionary = {}
var _playback: AudioStreamGeneratorPlayback


func _ready() -> void:
	var gen := AudioStreamGenerator.new()
	gen.mix_rate = SAMPLE_RATE
	gen.buffer_length = 0.1
	stream = gen
	play()
	_playback = get_stream_playback()


func _process(_delta: float) -> void:
	_fill_buffer()


func note_on(note: int) -> void:
	_active_notes[note] = 0.0


func note_off(note: int) -> void:
	_active_notes.erase(note)


func _fill_buffer() -> void:
	var to_fill := _playback.get_frames_available()
	if _active_notes.is_empty():
		for i in to_fill:
			_playback.push_frame(Vector2.ZERO)
		return
	var notes := _active_notes.keys()
	for i in to_fill:
		var sample := 0.0
		for note in notes:
			var freq := 440.0 * pow(2.0, (note - 69.0) / 12.0)
			var phase: float = fmod(_active_notes[note] + freq / SAMPLE_RATE, 1.0)
			_active_notes[note] = phase
			# additive: fundamental + harmonics for a warmer tone
			sample += sin(phase * TAU) * 0.6
			sample += sin(phase * TAU * 2.0) * 0.25
			sample += sin(phase * TAU * 3.0) * 0.1
			sample += sin(phase * TAU * 4.0) * 0.05
		sample = sample / notes.size() * 0.4
		_playback.push_frame(Vector2(sample, sample))
