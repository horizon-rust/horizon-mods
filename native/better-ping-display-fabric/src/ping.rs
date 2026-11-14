//! Port of upstream `PingText`/display logic: sample smoothing, threshold
//! coloring and the text form.
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Upstream "accuratePingDisplay" — show exact ping instead of bars only.
    pub accurate: bool,
    /// Text format, upstream template with {PING}.
    pub text_format: String,
    /// Color thresholds in ms (green below green_ms, yellow below yellow_ms,
    /// red at/above red_ms).
    pub green_ms: u32,
    pub yellow_ms: u32,
    pub red_ms: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            accurate: true,
            text_format: "+{PING} ms".to_string(),
            green_ms: 80,
            yellow_ms: 160,
            red_ms: 250,
        }
    }
}

/// One player's ping tracker: keeps the last N latency samples and exposes the
/// smoothed value upstream renders.
#[derive(Clone, Debug)]
pub struct PingTracker {
    pub player_name: String,
    samples: VecDeque<u32>,
    capacity: usize,
}

impl PingTracker {
    pub fn new(player_name: &str, capacity: usize) -> Self {
        PingTracker {
            player_name: player_name.to_string(),
            samples: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
        }
    }

    pub fn record(&mut self, latency_ms: u32) {
        if self.samples.len() == self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(latency_ms);
    }

    /// Mean of the samples, like upstream's smoothing over recent responses.
    pub fn smoothed_ms(&self) -> u32 {
        if self.samples.is_empty() {
            return 0;
        }
        (self.samples.iter().map(|&s| s as u64).sum::<u64>() / self.samples.len() as u64) as u32
    }
}

/// Upstream color choice for a latency in ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingColor {
    Green,
    Yellow,
    Red,
}

pub fn color_for(ms: u32, s: &Settings) -> PingColor {
    if ms < s.green_ms {
        PingColor::Green
    } else if ms < s.yellow_ms {
        PingColor::Yellow
    } else {
        PingColor::Red
    }
}

/// Upstream text: template with {PING} replaced by the smoothed value.
pub fn text_for(tracker: &PingTracker, s: &Settings) -> String {
    if !s.accurate {
        return String::new();
    }
    s.text_format.replace("{PING}", &tracker.smoothed_ms().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_upstream() {
        assert_eq!(Settings::default().text_format, "+{PING} ms");
    }

    #[test]
    fn smoothing_is_mean_of_recent_samples() {
        let mut t = PingTracker::new("Danil", 5);
        for ms in [100u32, 200, 300, 400, 500] {
            t.record(ms);
        }
        assert_eq!(t.smoothed_ms(), 300);
        t.record(0); // evicts the oldest (100)
        assert_eq!(t.smoothed_ms(), 280);
    }

    #[test]
    fn capacity_keeps_at_least_one_sample() {
        let mut t = PingTracker::new("p", 0);
        t.record(42);
        assert_eq!(t.smoothed_ms(), 42);
    }

    #[test]
    fn thresholds_match_upstream_bands() {
        let s = Settings::default();
        assert_eq!(color_for(0, &s), PingColor::Green);
        assert_eq!(color_for(79, &s), PingColor::Green);
        assert_eq!(color_for(80, &s), PingColor::Yellow);
        assert_eq!(color_for(160, &s), PingColor::Red);
        assert_eq!(color_for(999, &s), PingColor::Red);
    }

    #[test]
    fn text_replaces_ping_token() {
        let mut t = PingTracker::new("p", 3);
        t.record(120);
        assert_eq!(text_for(&t, &Settings::default()), "+120 ms");
    }

    #[test]
    fn accurate_off_suppresses_text_like_upstream_toggle() {
        let mut t = PingTracker::new("p", 3);
        t.record(120);
        let mut s = Settings::default();
        s.accurate = false;
        assert_eq!(text_for(&t, &s), "");
    }
}
