// Better F1 Reborn ported to Horizon (slice 1: HUD state machine core).
// Upstream: github.com/MovTery/Better-F1-Reborn (commit 62824ad, 2026-07-21), MIT (c) 2024 MovTery.
// Ported 1:1 from common/src/main/java/com/movtery/betterf1.

use serde::{Deserialize, Serialize};

/// com.movtery.betterf1.client.HUDState: the three-state F1 cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HudState {
    #[serde(rename = "ALL_VISIBLE")]
    AllVisible,
    #[serde(rename = "NO_HUD")]
    NoHud,
    #[serde(rename = "ALL_HIDDEN")]
    AllHidden,
}

impl HudState {
    /// HUDState.next(): ALL_VISIBLE -> NO_HUD -> ALL_HIDDEN -> ALL_VISIBLE.
    pub fn next(self) -> Self {
        match self {
            HudState::AllVisible => HudState::NoHud,
            HudState::NoHud => HudState::AllHidden,
            HudState::AllHidden => HudState::AllVisible,
        }
    }

    /// KeyboardMixin sets `options.hideGui = !BetterF1.state.equals(HUDState.ALL_VISIBLE)`.
    pub fn hides_gui(self) -> bool {
        self != HudState::AllVisible
    }

    /// GameRendererMixin.onRenderHand / MixinHandRenderer.canRender redirect:
    /// `instance.hideGui && BetterF1.state.equals(HUDState.ALL_HIDDEN)`.
    pub fn hand_hidden(self, options_hide_gui: bool) -> bool {
        options_hide_gui && self == HudState::AllHidden
    }
}

/// com.movtery.betterf1.BetterF1.state: the mod's global HUD state.
#[derive(Debug)]
pub struct BetterF1 {
    pub state: HudState,
}

impl Default for BetterF1 {
    fn default() -> Self {
        // BetterF1.state starts at HUDState.ALL_VISIBLE.
        Self {
            state: HudState::AllVisible,
        }
    }
}

impl BetterF1 {
    /// KeyboardMixin.onF1Key: advance the state, then set hideGui to follow it and
    /// cancel vanilla's own toggle. Horizon seam: native_input_key (API 121) is a
    /// post-handling, non-cancellable notice, so the port tracks the desired hideGui
    /// value and returns it for game glue to apply instead of intercepting the key.
    pub fn on_f1_key(&mut self) -> bool {
        self.state = self.state.next();
        self.state.hides_gui()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_starts_all_visible() {
        assert_eq!(BetterF1::default().state, HudState::AllVisible);
    }

    #[test]
    fn next_cycles_through_three_states() {
        let mut s = HudState::AllVisible;
        let seen = [s.next(), s.next().next(), s.next().next().next()];
        assert_eq!(
            seen,
            [HudState::NoHud, HudState::AllHidden, HudState::AllVisible]
        );
        s = s.next().next().next();
        assert_eq!(s, HudState::AllVisible, "full cycle returns to the start");
    }

    #[test]
    fn hides_gui_matches_upstream_mapping() {
        assert!(!HudState::AllVisible.hides_gui());
        assert!(HudState::NoHud.hides_gui());
        assert!(HudState::AllHidden.hides_gui());
    }

    #[test]
    fn hand_hidden_only_in_all_hidden() {
        // upstream: instance.hideGui && state == ALL_HIDDEN
        assert!(!HudState::AllVisible.hand_hidden(false));
        assert!(!HudState::AllVisible.hand_hidden(true));
        assert!(!HudState::NoHud.hand_hidden(true));
        assert!(HudState::AllHidden.hand_hidden(true));
        assert!(!HudState::AllHidden.hand_hidden(false));
    }

    #[test]
    fn on_f1_key_advances_and_returns_hide_gui() {
        let mut f1 = BetterF1::default();
        // press 1: ALL_VISIBLE -> NO_HUD, hideGui true (HUD hidden, hands kept)
        assert!(f1.on_f1_key());
        assert_eq!(f1.state, HudState::NoHud);
        // press 2: NO_HUD -> ALL_HIDDEN, hideGui true (everything hidden)
        assert!(f1.on_f1_key());
        assert_eq!(f1.state, HudState::AllHidden);
        // press 3: ALL_HIDDEN -> ALL_VISIBLE, hideGui false
        assert!(!f1.on_f1_key());
        assert_eq!(f1.state, HudState::AllVisible);
        // full press cycle returns to vanilla-off
        assert!(f1.on_f1_key());
        assert!(f1.on_f1_key());
        assert!(!f1.on_f1_key());
    }

    #[test]
    fn serde_names_match_enum_names() {
        assert_eq!(
            serde_json::to_string(&HudState::NoHud).unwrap(),
            r#""NO_HUD""#
        );
        assert_eq!(
            serde_json::from_str::<HudState>(r#""ALL_HIDDEN""#).unwrap(),
            HudState::AllHidden
        );
    }
}
