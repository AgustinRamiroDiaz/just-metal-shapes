//! Beat/bar/seconds conversions and audio latency math.

use std::sync::Arc;

pub const BEATS_PER_BAR: u32 = 4;

/// Beat grid: beat 0 is at `offset_seconds`. Fixed-tempo unless `beat_times` holds a
/// tempo map (the time of every whole beat, for songs whose tempo drifts); then times
/// between beats are interpolated and times past either end use the edge tempo.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    /// Average tempo; the exact tempo when there is no map.
    pub bpm: f64,
    pub offset_seconds: f64,
    pub beats_per_bar: u32,
    pub beat_times: Option<Arc<[f64]>>,
}

impl Timing {
    pub const fn new(bpm: f64, offset_seconds: f64) -> Self {
        Self {
            bpm,
            offset_seconds,
            beats_per_bar: BEATS_PER_BAR,
            beat_times: None,
        }
    }

    /// Grid following `beat_times` (seconds of beats 0, 1, 2...). Falls back to a fixed
    /// grid at `bpm` when the map has fewer than two beats.
    pub fn with_beat_times(bpm: f64, beat_times: &[f64]) -> Self {
        match beat_times {
            [first, .., last] => Self {
                bpm: 60.0 * (beat_times.len() - 1) as f64 / (last - first),
                offset_seconds: *first,
                beats_per_bar: BEATS_PER_BAR,
                beat_times: Some(beat_times.into()),
            },
            [first] => Self::new(bpm, *first),
            [] => Self::new(bpm, 0.0),
        }
    }

    /// Average seconds per beat.
    pub fn seconds_per_beat(&self) -> f64 {
        60.0 / self.bpm
    }

    /// Length of the beat containing `beat` (the average without a tempo map).
    pub fn seconds_per_beat_at(&self, beat: f64) -> f64 {
        match self.map() {
            Some(times) => {
                let last = times.len() - 2;
                let index = (beat.floor().max(0.0) as usize).min(last);
                times[index + 1] - times[index]
            }
            None => self.seconds_per_beat(),
        }
    }

    /// Song time (seconds) of a fractional beat.
    pub fn beat_to_seconds(&self, beat: f64) -> f64 {
        let Some(times) = self.map() else {
            return self.offset_seconds + beat * self.seconds_per_beat();
        };
        let last = times.len() - 2;
        let index = (beat.floor().max(0.0) as usize).min(last);
        times[index] + (beat - index as f64) * (times[index + 1] - times[index])
    }

    /// Fractional beat at a song time. Negative before the first beat.
    pub fn seconds_to_beat(&self, seconds: f64) -> f64 {
        let Some(times) = self.map() else {
            return (seconds - self.offset_seconds) / self.seconds_per_beat();
        };
        let last = times.len() - 2;
        let index = times
            .partition_point(|t| *t <= seconds)
            .saturating_sub(1)
            .min(last);
        index as f64 + (seconds - times[index]) / (times[index + 1] - times[index])
    }

    /// Length in seconds of a span of beats, at the average tempo.
    pub fn beats_to_duration(&self, beats: f64) -> f64 {
        beats * self.seconds_per_beat()
    }

    fn map(&self) -> Option<&[f64]> {
        self.beat_times.as_deref().filter(|times| times.len() >= 2)
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
    fn follows_a_tempo_map() {
        // 120 BPM for two beats, then 60 BPM.
        let timing = Timing::with_beat_times(0.0, &[1.0, 1.5, 2.0, 3.0, 4.0]);
        assert!(close(timing.offset_seconds, 1.0));
        assert!(close(timing.bpm, 80.0));
        assert!(close(timing.beat_to_seconds(0.5), 1.25));
        assert!(close(timing.beat_to_seconds(2.5), 2.5));
        assert!(close(timing.seconds_to_beat(2.5), 2.5));
        assert!(close(timing.seconds_to_beat(1.75), 1.5));
        // Past either end: the edge tempo.
        assert!(close(timing.beat_to_seconds(-1.0), 0.5));
        assert!(close(timing.seconds_to_beat(0.5), -1.0));
        assert!(close(timing.beat_to_seconds(6.0), 6.0));
        assert!(close(timing.seconds_to_beat(6.0), 6.0));
        assert!(close(timing.seconds_per_beat_at(0.5), 0.5));
        assert!(close(timing.seconds_per_beat_at(3.2), 1.0));
        assert!(close(timing.seconds_per_beat_at(99.0), 1.0));
        for beat in [-3.5, 0.0, 0.3, 1.0, 2.99, 3.0, 4.0, 9.25] {
            assert!(close(
                timing.seconds_to_beat(timing.beat_to_seconds(beat)),
                beat
            ));
        }
        let short = Timing::with_beat_times(120.0, &[0.25]);
        assert_eq!(short, Timing::new(120.0, 0.25));
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
