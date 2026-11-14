//! `io.github.a5b84.statuseffectbars.mixin.MobEffectInstanceMixin`, 1:1.
//! Adds the `maxDuration` attribute used for rendering.

use crate::mob_effect_instance_duck::MobEffectInstanceDuck;

/// `onInit` (ctor RETURN): `maxDuration = duration`.
pub fn on_init(duck: &mut impl MobEffectInstanceDuck, duration: i32) {
    duck.status_effect_bars_set_max_duration(duration);
}

/// `onCopyFrom` (`setDetailsFrom` RETURN): copies `maxDuration` from `copy`.
pub fn on_copy_from(duck: &mut impl MobEffectInstanceDuck, copy: &impl MobEffectInstanceDuck) {
    duck.status_effect_bars_set_max_duration(copy.status_effect_bars_get_max_duration());
}

/// `onUpgrade` (`update` at the `duration` PUTFIELD): takes over
/// `takeOver`'s `maxDuration`.
pub fn on_upgrade(duck: &mut impl MobEffectInstanceDuck, take_over: &impl MobEffectInstanceDuck) {
    duck.status_effect_bars_set_max_duration(take_over.status_effect_bars_get_max_duration());
}
