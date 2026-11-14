//! Port of upstream screen animation: eased open/close over ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Upstream "animation duration" in ticks.
    pub duration: i32,
    /// Upstream slide amount in pixels the screen travels while opening.
    pub slide_px: f32,
    /// Upstream fade amount, 0.0 (no fade) to 1.0 (full fade at start).
    pub fade: f32,
    /// Upstream "animateOnClose".
    pub animate_close: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            duration: 5,
            slide_px: 16.0,
            fade: 0.5,
            animate_close: true,
        }
    }
}

impl Settings {
    pub fn validate(&mut self) {
        self.duration = self.duration.clamp(0, 40);
        self.slide_px = self.slide_px.clamp(0.0, 256.0);
        self.fade = self.fade.clamp(0.0, 1.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimState {
    Closed,
    Opening,
    Open,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anim {
    pub state: AnimState,
    pub elapsed: i32,
}

impl Anim {
    pub fn new() -> Self {
        Anim {
            state: AnimState::Closed,
            elapsed: 0,
        }
    }
}

impl Default for Anim {
    fn default() -> Self {
        Self::new()
    }
}

/// Upstream ease: smoothstep over the duration. Returns the eased progress
/// 0.0..=1.0 for tick t out of n, or 1.0 when n == 0 (instant).
pub fn eased(t: i32, n: i32) -> f32 {
    if n <= 0 {
        return 1.0;
    }
    let p = (t as f32 / n as f32).clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

/// The offset a screen sits at for eased progress p: slide_px below its final
/// position while opening, 0 when done.
pub fn slide_offset(p: f32, s: &Settings) -> f32 {
    s.slide_px * (1.0 - p)
}

/// Alpha multiplier for eased progress p: fade bottoms out at (1 - fade).
pub fn alpha(p: f32, s: &Settings) -> f32 {
    1.0 - s.fade * (1.0 - p)
}

impl Anim {
    pub fn open(&mut self, s: &Settings) {
        self.state = AnimState::Opening;
        self.elapsed = 0;
        if s.duration == 0 {
            self.state = AnimState::Open;
        }
    }

    pub fn close(&mut self, s: &Settings) {
        if !s.animate_close {
            self.state = AnimState::Closed;
            self.elapsed = 0;
            return;
        }
        self.state = AnimState::Closing;
        self.elapsed = 0;
        if s.duration == 0 {
            self.state = AnimState::Closed;
        }
    }

    /// Upstream cancel: the screen closes instantly, no close animation.
    pub fn cancel(&mut self) {
        self.state = AnimState::Closed;
        self.elapsed = 0;
    }

    pub fn tick(&mut self, s: &Settings) {
        match self.state {
            AnimState::Opening => {
                self.elapsed += 1;
                if self.elapsed >= s.duration {
                    self.state = AnimState::Open;
                    self.elapsed = s.duration;
                }
            }
            AnimState::Closing => {
                self.elapsed += 1;
                if self.elapsed >= s.duration {
                    self.state = AnimState::Closed;
                    self.elapsed = 0;
                }
            }
            _ => {}
        }
    }

    /// Current eased progress: opening counts 0->1, closing counts 1->0.
    pub fn progress(&self, s: &Settings) -> f32 {
        let e = eased(self.elapsed, s.duration);
        match self.state {
            AnimState::Opening | AnimState::Open => e,
            AnimState::Closing => 1.0 - e,
            AnimState::Closed => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_match_upstream() {
        assert_eq!(
            Settings::default(),
            Settings {
                duration: 5,
                slide_px: 16.0,
                fade: 0.5,
                animate_close: true
            }
        );
    }

    #[test]
    fn validation_clamps_ranges() {
        let mut s = Settings {
            duration: 100,
            slide_px: 999.0,
            fade: 5.0,
            animate_close: false,
        };
        s.validate();
        assert_eq!(s.duration, 40);
        assert_eq!(s.slide_px, 256.0);
        assert_eq!(s.fade, 1.0);
    }

    #[test]
    fn zero_duration_opens_instantly() {
        let s = Settings {
            duration: 0,
            ..Settings::default()
        };
        let mut a = Anim::new();
        a.open(&s);
        assert_eq!(a.state, AnimState::Open);
        assert_eq!(a.progress(&s), 1.0);
    }

    #[test]
    fn opening_completes_after_duration_ticks() {
        let mut s = Settings::default();
        s.validate();
        let mut a = Anim::new();
        a.open(&s);
        for _ in 0..s.duration - 1 {
            a.tick(&s);
            assert_eq!(a.state, AnimState::Opening);
        }
        a.tick(&s);
        assert_eq!(a.state, AnimState::Open);
    }

    #[test]
    fn slide_and_alpha_track_progress() {
        let mut s = Settings::default();
        s.validate();
        assert_eq!(slide_offset(1.0, &s), 0.0);
        assert_eq!(slide_offset(0.0, &s), 16.0);
        assert_eq!(alpha(0.0, &s), 0.5);
        assert_eq!(alpha(1.0, &s), 1.0);
    }

    #[test]
    fn close_reverses_and_cancel_skips_animation() {
        let mut s = Settings::default();
        s.validate();
        let mut a = Anim::new();
        a.open(&s);
        for _ in 0..s.duration {
            a.tick(&s);
        }
        a.close(&s);
        a.tick(&s);
        assert_eq!(a.state, AnimState::Closing);
        assert!(a.progress(&s) < 1.0, "closing starts below full progress");
        a.cancel();
        assert_eq!(a.state, AnimState::Closed);
        let mut s2 = Settings {
            animate_close: false,
            ..s
        };
        s2.validate();
        let mut b = Anim::new();
        b.open(&s2);
        b.close(&s2);
        assert_eq!(
            b.state,
            AnimState::Closed,
            "animate_close off closes instantly"
        );
    }
}
