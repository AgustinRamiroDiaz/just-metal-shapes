//! Beat/bar/seconds conversions and audio latency math.

pub const BEATS_PER_BAR: u32 = 4;

/// Fixed-tempo grid: beat 0 is at `offset_seconds`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub bpm: f64,
    pub offset_seconds: f64,
    pub beats_per_bar: u32,
}

impl Timing {
    pub fn new(bpm: f64, offset_seconds: f64) -> Self {
        Self {
            bpm,
            offset_seconds,
            beats_per_bar: BEATS_PER_BAR,
        }
    }

    pub fn seconds_per_beat(&self) -> f64 {
        60.0 / self.bpm
    }

    /// Song time (seconds) of a fractional beat.
    pub fn beat_to_seconds(&self, beat: f64) -> f64 {
        self.offset_seconds + beat * self.seconds_per_beat()
    }

    /// Fractional beat at a song time. Negative before the first beat.
    pub fn seconds_to_beat(&self, seconds: f64) -> f64 {
        (seconds - self.offset_seconds) / self.seconds_per_beat()
    }

    /// Length in seconds of a span of beats.
    pub fn beats_to_duration(&self, beats: f64) -> f64 {
        beats * self.seconds_per_beat()
    }

    /// Bar index containing a beat (floor; negative before beat 0).
    pub fn bar_of_beat(&self, beat: f64) -> i64 {
        (beat / self.beats_per_bar as f64).floor() as i64
    }

    pub fn bar_start_beat(&self, bar: i64) -> f64 {
        (bar * self.beats_per_bar as i64) as f64
    }

    /// Position within the bar, `0..beats_per_bar`.
    pub fn beat_in_bar(&self, beat: f64) -> f64 {
        beat.rem_euclid(self.beats_per_bar as f64)
    }

    pub fn is_downbeat(&self, beat: i64) -> bool {
        beat.rem_euclid(self.beats_per_bar as i64) == 0
    }
}

/// Song time heard by the player, from an audio playback position.
///
/// `playback_position + time_since_last_mix` estimates where the mixer is now;
/// subtracting `output_latency` gives what is coming out of the speakers. The user
/// `latency_offset` (seconds, from settings) is subtracted too: a positive value means
/// "my audio arrives later than reported", which delays the game clock to match.
pub fn audio_song_time(
    playback_position: f64,
    time_since_last_mix: f64,
    output_latency: f64,
    latency_offset: f64,
) -> f64 {
    playback_position + time_since_last_mix - output_latency - latency_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn converts_beats_and_seconds() {
        let timing = Timing::new(120.0, 0.25);
        assert!(close(timing.seconds_per_beat(), 0.5));
        assert!(close(timing.beat_to_seconds(0.0), 0.25));
        assert!(close(timing.beat_to_seconds(4.0), 2.25));
        assert!(close(timing.seconds_to_beat(2.25), 4.0));
        assert!(close(timing.seconds_to_beat(0.0), -0.5));
        assert!(close(timing.beats_to_duration(3.0), 1.5));
        for beat in [-3.5, 0.0, 1.25, 77.0] {
            assert!(close(
                timing.seconds_to_beat(timing.beat_to_seconds(beat)),
                beat
            ));
        }
    }

    #[test]
    fn bar_math() {
        let timing = Timing::new(90.0, 0.0);
        assert_eq!(timing.bar_of_beat(0.0), 0);
        assert_eq!(timing.bar_of_beat(3.99), 0);
        assert_eq!(timing.bar_of_beat(4.0), 1);
        assert_eq!(timing.bar_of_beat(-0.5), -1);
        assert!(close(timing.bar_start_beat(3), 12.0));
        assert!(close(timing.beat_in_bar(13.5), 1.5));
        assert!(close(timing.beat_in_bar(-1.0), 3.0));
        assert!(timing.is_downbeat(8));
        assert!(timing.is_downbeat(-4));
        assert!(!timing.is_downbeat(9));
    }

    #[test]
    fn latency_compensation() {
        assert!(close(audio_song_time(10.0, 0.01, 0.03, 0.0), 9.98));
        assert!(close(audio_song_time(10.0, 0.0, 0.0, 0.05), 9.95));
        assert!(close(audio_song_time(10.0, 0.0, 0.0, -0.05), 10.05));
    }
}
