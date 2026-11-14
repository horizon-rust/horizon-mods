//! `io.github.a5b84.statuseffectbars.MobEffectInstanceDuck`, 1:1, plus the
//! `MobEffectInstance` host surface the mixins and renderer read.

/// `net.minecraft.world.effect.MobEffectCategory` (host enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobEffectCategory {
    BENEFICIAL,
    HARMFUL,
    NEUTRAL,
}

pub trait MobEffectInstanceDuck {
    fn status_effect_bars_get_max_duration(&self) -> i32;
    fn status_effect_bars_set_max_duration(&mut self, max_duration: i32);
}

/// `net.minecraft.world.effect.MobEffectInstance` host surface: the reads the
/// mixin-injected state and the renderer make on the host class.
pub trait MobEffectInstanceHost: MobEffectInstanceDuck {
    /// `MobEffectInstance.duration` (the mixin's shadowed field).
    fn get_duration(&self) -> i32;
    fn set_duration(&mut self, duration: i32);
    fn is_infinite_duration(&self) -> bool;
    fn is_ambient(&self) -> bool;
    /// `MobEffectInstance.getAmplifier()`.
    fn get_amplifier(&self) -> i32;
    /// `effect.getEffect().value().getColor()`.
    fn effect_color(&self) -> i32;
    /// `effect.getEffect().value().getCategory()`.
    fn category(&self) -> MobEffectCategory;
    /// `TextColor.fromLegacyFormat(category.getTooltipFormatting()).getValue()`
    /// (the legacy-format RGB value, alpha unset).
    fn category_format_color(&self) -> i32;
}
