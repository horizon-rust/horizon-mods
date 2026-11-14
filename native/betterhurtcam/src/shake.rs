//! Port of upstream `BetterHurtcam` mixin logic: shake setup, per-tick decay,
//! repeat scheduling and the settings surface.
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Upstream `intensity` (0.0-10.0).
    pub intensity: f32,
    /// Upstream `duration` in ticks (0-40).
    pub duration: i32,
    /// Upstream `repeats` (0-20).
    pub repeats: i32,
    /// Upstream `onlyCrit` — shake only on critical hits.
    pub only_crit: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            intensity: 1.0,
            duration: 5,
            repeats: 0,
            only_crit: false,
        }
    }
}

impl Settings {
    pub fn validate(&mut self) {
        self.intensity = self.intensity.clamp(0.0, 10.0);
        self.duration = self.duration.clamp(0, 40);
        self.repeats = self.repeats.clamp(0, 20);
    }
}

/// Upstream shake entry: on `onHurt` with `isCrit`, unless onlyCrit and not a
/// crit. Repeats mean the shake plays again after `duration` ticks of rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShakeState {
    pub active: bool,
    pub ticks_left: i32,
    pub repeats_left: i32,
    pub intensity: f32,
    /// Phase in radians, advanced per tick; drives the sinusoidal shake.
    pub phase: f32,
    /// Duration the running shake was started with; replays reuse it, the way
    /// upstream re-reads the setting each replay.
    pub started_duration: i32,
}

impl ShakeState {
    pub fn new() -> Self {
        ShakeState {
            active: false,
            ticks_left: 0,
            repeats_left: 0,
            intensity: 0.0,
            phase: 0.0,
            started_duration: 0,
        }
    }
}

impl Default for ShakeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Should this hurt event start a shake?
pub fn should_shake(settings: &Settings, is_crit: bool) -> bool {
    !(settings.only_crit && !is_crit)
}

/// Upstream `onHurt`: start a shake for `duration` ticks; repeats schedule
/// that many replays.
pub fn start_shake(settings: &Settings, state: &mut ShakeState) {
    state.active = true;
    state.ticks_left = settings.duration;
    state.repeats_left = settings.repeats;
    state.intensity = settings.intensity;
    state.phase = 0.0;
    state.started_duration = settings.duration;
}

/// One tick. Returns Some((pitch_offset, yaw_offset)) in radians while the
/// shake is running, None otherwise. Decay: upstream eases out over the
/// duration, so offsets shrink linearly to 0 at the last tick.
pub fn tick(state: &mut ShakeState) -> Option<(f32, f32)> {
    if !state.active {
        return None;
    }
    if state.ticks_left <= 0 {
        if state.repeats_left > 0 {
            state.repeats_left -= 1;
            state.ticks_left = state.started_duration;
            state.phase = 0.0;
        } else {
            state.active = false;
            return None;
        }
    }
    let progress = state.ticks_left as f32;
    let span = (state.started_duration as f32).max(1.0);
    let decay = progress / span;
    state.phase += TAU / 8.0;
    state.ticks_left -= 1;
    let amp = state.intensity * decay.clamp(0.0, 1.0);
    let pitch = (state.phase.sin() * amp).to_radians();
    let yaw = (state.phase.cos() * amp).to_radians();
    Some((pitch, yaw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_upstream() {
        let s = Settings::default();
        assert_eq!(
            s,
            Settings {
                intensity: 1.0,
                duration: 5,
                repeats: 0,
                only_crit: false
            }
        );
    }

    #[test]
    fn validation_clamps_into_upstream_ranges() {
        let mut s = Settings {
            intensity: 99.0,
            duration: -3,
            repeats: 50,
            only_crit: true,
        };
        s.validate();
        assert_eq!(s.intensity, 10.0);
        assert_eq!(s.duration, 0);
        assert_eq!(s.repeats, 20);
    }

    #[test]
    fn only_crit_blocks_non_crit_hurts() {
        let s = Settings::default();
        assert!(should_shake(&s, false));
        let s2 = Settings {
            only_crit: true,
            ..Settings::default()
        };
        assert!(!should_shake(&s2, false));
        assert!(should_shake(&s2, true));
    }

    #[test]
    fn shake_runs_for_duration_then_stops() {
        let mut s = Settings::default();
        s.validate();
        let mut st = ShakeState::new();
        start_shake(&s, &mut st);
        let mut offsets = 0;
        for _ in 0..s.duration + 2 {
            if tick(&mut st).is_some() {
                offsets += 1;
            }
        }
        assert_eq!(
            offsets, s.duration as usize,
            "shake runs exactly duration ticks"
        );
        assert!(!st.active);
    }

    #[test]
    fn shake_with_repeats_plays_duration_x_repeats_plus_one() {
        let mut s = Settings {
            duration: 3,
            repeats: 2,
            ..Settings::default()
        };
        s.validate();
        let mut st = ShakeState::new();
        start_shake(&s, &mut st);
        let mut total = 0;
        for _ in 0..50 {
            if tick(&mut st).is_some() {
                total += 1;
            }
        }
        assert_eq!(total, 3 * 3, "replays reuse the start duration");
    }

    #[test]
    fn offsets_decay_toward_end() {
        let mut s = Settings {
            intensity: 5.0,
            duration: 6,
            repeats: 0,
            only_crit: false,
        };
        s.validate();
        let mut st = ShakeState::new();
        start_shake(&s, &mut st);
        let (p1, _) = tick(&mut st).unwrap();
        let mut last = (0.0f32, 0.0f32);
        while let Some(o) = tick(&mut st) {
            last = o;
        }
        let mag1 = p1.abs();
        let mag_last = last.0.abs().max(last.1.abs());
        assert!(mag1 > mag_last, "early shake amplitude exceeds late shake");
    }
}
