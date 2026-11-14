//! `io.github.a5b84.statuseffectbars.mixin.LivingEntityMixin`, 1:1.
//! Prevents effects' max duration from being overwritten.

use crate::mob_effect_instance_duck::MobEffectInstanceHost;

/// `onSetStatusEffect` (`forceAddEffect` TAIL, locals `newEffect` /
/// `previousEffect`).
pub fn on_set_status_effect(
    new_effect: &mut impl MobEffectInstanceHost,
    previous_effect: Option<&impl MobEffectInstanceHost>,
) {
    if let Some(previous_effect) = previous_effect {
        if previous_effect.get_amplifier() == new_effect.get_amplifier() {
            if new_effect.status_effect_bars_get_max_duration()
                < previous_effect.status_effect_bars_get_max_duration()
            {
                new_effect
                    .status_effect_bars_set_max_duration(previous_effect.status_effect_bars_get_max_duration());
            }
        }
    }
}
