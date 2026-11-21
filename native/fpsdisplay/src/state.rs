//! FPS Display for Horizon: the FPS readout's state (a 1:1 port of
//! fpsdisplaymod's FpsHistory and ConfigData, MIT).
//!
//! The samples come from the frame interval the draw hook measures between
//! two of its own calls: the API exposes no client fps read, and this
//! measures the same thing. History is the original's FIFO cap at 420.

use std::collections::VecDeque;
use std::time::Instant;

/// The original's history cap.
pub const LIMIT: usize = 420;

/// One FPS history (the original's `FpsHistory`).
#[derive(Default)]
pub struct FpsHistory {
    list: VecDeque<i32>,
}

impl FpsHistory {
    pub fn add(&mut self, fps: i32) {
        self.list.push_back(fps);
        if self.list.len() > LIMIT {
            self.list.pop_front();
        }
    }

    /// The smallest sample; the original's `Collections.min` panics on an
    /// empty list the same way this does on the empty iterator.
    pub fn get_minimum(&self) -> i32 {
        *self.list.iter().min().unwrap()
    }

    /// The largest sample (`Collections.max`).
    pub fn get_maximum(&self) -> i32 {
        *self.list.iter().max().unwrap()
    }

    /// The integer mean (`Collections`-style sum then divide).
    pub fn get_average(&self) -> i32 {
        let mut sum: i64 = 0;
        for fps in &self.list {
            sum += *fps as i64;
        }
        (sum / self.list.len() as i64) as i32
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }
}

/// The original's `KeyMode` (its config's keybind behaviour). Horizon has no
/// keybinding registration yet, so the setting is on file but the key half
/// of the behaviour waits on the API (README seams).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KeyMode {
    #[default]
    Toggle,
    PushToShow,
}

impl KeyMode {
    pub fn index(self) -> usize {
        match self {
            KeyMode::Toggle => 0,
            KeyMode::PushToShow => 1,
        }
    }
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => KeyMode::PushToShow,
            _ => KeyMode::Toggle,
        }
    }
}

/// The config's defaults, the original's field initializers.
pub struct Config {
    pub enabled: bool,
    pub advanced_stats: bool,
    pub text_size: f32,
    pub text_color: u32,
    pub text_alpha: u32,
    pub text_shadows: bool,
    pub offset_top: i32,
    pub offset_left: i32,
    pub keybind_mode: KeyMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            advanced_stats: false,
            text_size: 1.0,
            text_color: 0xEEEEEE,
            text_alpha: 230,
            text_shadows: false,
            offset_top: 4,
            offset_left: 4,
            keybind_mode: KeyMode::Toggle,
        }
    }
}

/// The mod's shared state: the history and the frame clock.
pub struct State {
    pub history: FpsHistory,
    pub last_frame: Option<Instant>,
    /// The overlay the keybind toggles (the original's ShowOverlay); with
    /// no keybinding API yet it stays true while `enabled` gates the rest.
    pub show_overlay: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            history: FpsHistory::default(),
            last_frame: None,
            show_overlay: true,
        }
    }
}

pub static STATE: std::sync::Mutex<State> = std::sync::Mutex::new(State {
    history: FpsHistory {
        list: VecDeque::new(),
    },
    last_frame: None,
    show_overlay: true,
});

pub fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether the keybind's overlay gate is on (the original's ShowOverlay;
/// with no keybinding API yet it stays true while `enabled` gates the rest).
pub fn show_overlay() -> bool {
    state().show_overlay
}

/// The history's min, average and max, for the advanced line.
pub fn history_min_avg_max() -> (i32, i32, i32) {
    let st = state();
    (st.history.get_minimum(), st.history.get_average(), st.history.get_maximum())
}

/// One draw call: the interval since the previous call is one frame, so the
/// fps is its reciprocal. Returns the fps just sampled.
pub fn sample_frame() -> i32 {
    let now = Instant::now();
    let mut st = state();
    let fps = st
        .last_frame
        .map(|last| {
            let dt = now.duration_since(last).as_secs_f64();
            if dt > 0.0 {
                (1.0 / dt).round().clamp(1.0, 10_000.0) as i32
            } else {
                0
            }
        })
        .unwrap_or(0);
    st.last_frame = Some(now);
    if fps > 0 {
        st.history.add(fps);
    }
    fps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_caps_at_the_original_limit() {
        let mut h = FpsHistory::default();
        for i in 0..600 {
            h.add(i);
        }
        assert_eq!(h.len(), LIMIT);
        assert_eq!(h.get_minimum(), 600 - LIMIT as i32);
        assert_eq!(h.get_maximum(), 599);
    }

    #[test]
    fn averages_are_the_original_integer_mean() {
        let mut h = FpsHistory::default();
        for i in 1..=5 {
            h.add(i);
        }
        assert_eq!(h.get_average(), 3);
    }

    #[test]
    fn key_mode_round_trips() {
        assert_eq!(KeyMode::from_index(0), KeyMode::Toggle);
        assert_eq!(KeyMode::from_index(1), KeyMode::PushToShow);
        assert_eq!(KeyMode::PushToShow.index(), 1);
    }
}
