class_name MidiSynth
extends AudioStreamPlayer

const SAMPLE_RATE := 44100.0
# 100 BPM → one beat every 0.6 s
const BEAT_DURATION := 60.0 / 100.0

# quaver, crotchet, minim, semibreve
const NOTE_DURATIONS: Array[float] = [
	BEAT_DURATION * 0.5,  # quaver      (8th)
	BEAT_DURATION,  # crotchet    (quarter)
	BEAT_DURATION * 2.0,  # minim       (half)
	BEAT_DURATION * 4.0,  # semibreve   (whole)
]

# A natural minor scale: A4 B4 C5 D5 E5 F5 G5 A5
const AM_SCALE: Array[int] = [69, 71, 72, 74, 76, 77, 79, 81]

var enabled: bool = false

# note_number -> phase (0..1)
var _active_notes: Dictionary = {}
var _playback: AudioStreamGeneratorPlayback
var _hit_cooldown: float = 0.0


func _ready() -> void:
	var gen := AudioStreamGenerator.new()
	gen.mix_rate = SAMPLE_RATE
	gen.buffer_length = 0.1
	stream = gen
	play()
	_playback = get_stream_playback()


func _process(delta: float) -> void:
	if _hit_cooldown > 0.0:
		_hit_cooldown -= delta
	_fill_buffer()


func note_on(note: int) -> void:
	_active_notes[note] = 0.0


func note_off(note: int) -> void:
	_active_notes.erase(note)


func play_hit_note() -> void:
	if not enabled or _hit_cooldown > 0.0:
		return
	var note := AM_SCALE[randi() % AM_SCALE.size()]
	var duration := NOTE_DURATIONS[randi() % NOTE_DURATIONS.size()]
	note_on(note)
	_hit_cooldown = duration
	get_tree().create_timer(duration).timeout.connect(func() -> void: note_off(note))


func _fill_buffer() -> void:
	var to_fill := _playback.get_frames_available()
	if not enabled or _active_notes.is_empty():
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
