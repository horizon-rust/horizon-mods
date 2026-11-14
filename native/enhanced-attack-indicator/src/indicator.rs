//! Port of upstream variant selection and cooldown progress logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Upstream OFF: hide the indicator entirely.
    Off,
    /// Upstream HORIZONTAL: a horizontal bar under the crosshair.
    Horizontal,
    /// Upstream CIRCUMFERENTIAL: an arc around the crosshair.
    Circumferential,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub variant: Variant,
    /// Hide the indicator once fully charged, after this many ticks.
    pub hide_when_charged_after_ticks: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            variant: Variant::Circumferential,
            hide_when_charged_after_ticks: 10,
        }
    }
}

/// Per-tick inputs the upstream renderer reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInput {
    /// Vanilla attack cooldown strength, 0.0..=1.0.
    pub attack_strength: f32,
    /// Whether the cooldown is currently recovering (mid-swing).
    pub cooldown_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeState {
    Charging,
    Charged,
}

/// Fill fraction shown for this frame. Off draws nothing; otherwise the
/// vanilla cooldown progress.
pub fn fill(input: &FrameInput, variant: Variant) -> Option<f32> {
    match variant {
        Variant::Off => None,
        _ => Some(input.attack_strength.clamp(0.0, 1.0)),
    }
}

/// Upstream charge color state.
pub fn charge_state(input: &FrameInput, variant: Variant) -> Option<ChargeState> {
    fill(input, variant).map(|f| {
        if f >= 1.0 {
            ChargeState::Charged
        } else {
            ChargeState::Charging
        }
    })
}

/// Whether the indicator draws this frame: Off never draws; when charged the
/// indicator lingers `hide_when_charged_after_ticks` ticks after the cooldown
/// finishes, then hides (upstream lets the bar fade out rather than vanishing
/// at the frame the cooldown completes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndicatorState {
    pub charged_ticks: i32,
}

impl IndicatorState {
    pub fn new() -> Self {
        IndicatorState { charged_ticks: 0 }
    }
}

impl Default for IndicatorState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn should_draw(
    input: &FrameInput,
    variant: Variant,
    state: &mut IndicatorState,
    s: &Settings,
) -> bool {
    if variant == Variant::Off {
        return false;
    }
    if input.cooldown_active || input.attack_strength < 1.0 {
        state.charged_ticks = 0;
        return true;
    }
    state.charged_ticks += 1;
    state.charged_ticks <= s.hide_when_charged_after_ticks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_variant_draws_nothing() {
        let i = FrameInput {
            attack_strength: 0.5,
            cooldown_active: true,
        };
        assert_eq!(fill(&i, Variant::Off), None);
        assert!(!should_draw(
            &i,
            Variant::Off,
            &mut IndicatorState::new(),
            &Settings::default()
        ));
    }

    #[test]
    fn fill_tracks_cooldown_for_both_visible_variants() {
        let i = FrameInput {
            attack_strength: 0.7,
            cooldown_active: true,
        };
        assert_eq!(fill(&i, Variant::Horizontal), Some(0.7));
        assert_eq!(fill(&i, Variant::Circumferential), Some(0.7));
        assert_eq!(
            charge_state(&i, Variant::Horizontal),
            Some(ChargeState::Charging)
        );
    }

    #[test]
    fn charged_color_at_full_strength() {
        let i = FrameInput {
            attack_strength: 1.0,
            cooldown_active: false,
        };
        assert_eq!(
            charge_state(&i, Variant::Circumferential),
            Some(ChargeState::Charged)
        );
    }

    #[test]
    fn indicator_hides_after_linger_ticks() {
        let mut s = Settings::default();
        let mut st = IndicatorState::new();
        let full = FrameInput {
            attack_strength: 1.0,
            cooldown_active: false,
        };
        for _ in 0..s.hide_when_charged_after_ticks {
            assert!(should_draw(&full, Variant::Circumferential, &mut st, &s));
        }
        assert!(
            !should_draw(&full, Variant::Circumferential, &mut st, &s),
            "hides after linger expires"
        );
    }

    #[test]
    fn mid_cooldown_resets_linger() {
        let mut s = Settings::default();
        let mut st = IndicatorState::new();
        let full = FrameInput {
            attack_strength: 1.0,
            cooldown_active: false,
        };
        for _ in 0..5 {
            should_draw(&full, Variant::Circumferential, &mut st, &s);
        }
        let charging = FrameInput {
            attack_strength: 0.5,
            cooldown_active: true,
        };
        assert!(should_draw(
            &charging,
            Variant::Circumferential,
            &mut st,
            &s
        ));
        assert_eq!(st.charged_ticks, 0, "a new swing resets the linger counter");
    }

    #[test]
    fn fill_is_clamped() {
        let i = FrameInput {
            attack_strength: 5.0,
            cooldown_active: true,
        };
        assert_eq!(fill(&i, Variant::Horizontal), Some(1.0));
    }
}
