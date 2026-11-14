//! crosshair-indicator (Horizon Rust port), slice 1: the progress model.
// NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the
// modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.
//!
//! 1:1 port of the upstream logic core (CC0-1.0): the crosshair swaps between
//! variants (vanilla cross, attack-indicator arc, break-progress arc) based on
//! what the player is doing, and colors follow the vanilla attack strength.
//! Render hooks land in the next slice behind the same seam.
pub mod indicator {
    //! Port of upstream crosshair variant selection and progress logic.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Variant {
        /// Vanilla cross.
        Vanilla,
        /// Attack indicator arc around the crosshair.
        AttackIndicator,
        /// Block-breaking progress arc.
        BreakingProgress,
        /// Full circle shown when the target is attackable.
        TargetAttackable,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Settings {
        /// Show the attack-indicator arc.
        pub attack_indicator: bool,
        /// Show the breaking-progress arc.
        pub breaking_progress: bool,
        /// Swap to the full-circle variant over entities.
        pub target_indicator: bool,
        /// Hide the vanilla cross while an arc is drawn.
        pub hide_vanilla_cross: bool,
    }

    impl Default for Settings {
        fn default() -> Self {
            Settings {
                attack_indicator: true,
                breaking_progress: true,
                target_indicator: true,
                hide_vanilla_cross: false,
            }
        }
    }

    /// Inputs upstream reads each frame.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct FrameInput {
        /// Vanilla attack cooldown strength, 0.0..=1.0 (1.0 = full charge).
        pub attack_strength: f32,
        /// 0.0..=1.0 progress of the block being broken, None when not breaking.
        pub breaking_progress: Option<f32>,
        /// Whether the crosshair currently rests over an attackable entity.
        pub over_attackable: bool,
    }

    /// Upstream variant pick for a frame. Order: breaking beats attack indicator
    /// beats target circle.
    pub fn variant_for(input: &FrameInput, s: &Settings) -> Variant {
        if s.breaking_progress && input.breaking_progress.is_some() {
            Variant::BreakingProgress
        } else if s.attack_indicator && input.attack_strength < 1.0 {
            Variant::AttackIndicator
        } else if s.target_indicator && input.over_attackable {
            Variant::TargetAttackable
        } else {
            Variant::Vanilla
        }
    }

    /// Arc fill shown on the arc variants: breaking progress when breaking,
    /// otherwise the attack cooldown.
    pub fn arc_fill(input: &FrameInput, variant: Variant) -> Option<f32> {
        match variant {
            Variant::BreakingProgress => input.breaking_progress,
            Variant::AttackIndicator | Variant::TargetAttackable => Some(input.attack_strength),
            Variant::Vanilla => None,
        }
    }

    /// Color state upstream uses: red while the attack is not charged, white once
    /// it is.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ArcColor {
        Charging,
        Ready,
    }

    pub fn arc_color(input: &FrameInput, variant: Variant) -> Option<ArcColor> {
        arc_fill(input, variant).map(|f| {
            if f >= 1.0 {
                ArcColor::Ready
            } else {
                ArcColor::Charging
            }
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn vanilla_when_idle() {
            let i = FrameInput {
                attack_strength: 1.0,
                breaking_progress: None,
                over_attackable: false,
            };
            assert_eq!(variant_for(&i, &Settings::default()), Variant::Vanilla);
        }

        #[test]
        fn attack_indicator_while_charging() {
            let i = FrameInput {
                attack_strength: 0.5,
                breaking_progress: None,
                over_attackable: false,
            };
            assert_eq!(
                variant_for(&i, &Settings::default()),
                Variant::AttackIndicator
            );
            assert_eq!(arc_fill(&i, Variant::AttackIndicator), Some(0.5));
            assert_eq!(
                arc_color(&i, Variant::AttackIndicator),
                Some(ArcColor::Charging)
            );
        }

        #[test]
        fn breaking_beats_attack() {
            let i = FrameInput {
                attack_strength: 0.2,
                breaking_progress: Some(0.7),
                over_attackable: false,
            };
            assert_eq!(
                variant_for(&i, &Settings::default()),
                Variant::BreakingProgress
            );
            assert_eq!(arc_fill(&i, Variant::BreakingProgress), Some(0.7));
        }

        #[test]
        fn target_circle_over_entities() {
            let i = FrameInput {
                attack_strength: 1.0,
                breaking_progress: None,
                over_attackable: true,
            };
            assert_eq!(
                variant_for(&i, &Settings::default()),
                Variant::TargetAttackable
            );
            assert_eq!(
                arc_color(&i, Variant::TargetAttackable),
                Some(ArcColor::Ready)
            );
        }

        #[test]
        fn settings_gates_turn_variants_off() {
            let mut s = Settings::default();
            s.breaking_progress = false;
            let i = FrameInput {
                attack_strength: 1.0,
                breaking_progress: Some(0.7),
                over_attackable: false,
            };
            assert_eq!(variant_for(&i, &s), Variant::Vanilla);
        }

        #[test]
        fn hide_cross_does_not_change_variant_pick() {
            let mut s = Settings::default();
            s.hide_vanilla_cross = true;
            let i = FrameInput {
                attack_strength: 1.0,
                breaking_progress: None,
                over_attackable: false,
            };
            assert_eq!(variant_for(&i, &s), Variant::Vanilla);
        }
    }
}
