//! `io.github.a5b84.statuseffectbars.config.ColorMode`, 1:1.

use std::fmt;

use crate::config::status_effect_bars_config::StatusEffectBarsConfig;
use crate::i18n;
use crate::mob_effect_instance_duck::{MobEffectCategory, MobEffectInstanceHost};

/// `0xff000000` (alpha-opaque) as a Java int.
const ALPHA_OPAQUE: i32 = 0xff000000u32 as i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ColorMode {
    #[default]
    EFFECT_COLOR,
    CATEGORY_COLOR,
    CUSTOM,
}

impl ColorMode {
    /// The `ToIntBiFunction<StatusEffectBarsConfig, MobEffectInstance>` bodies, 1:1.
    pub fn get_color(&self, config: &StatusEffectBarsConfig, effect: &impl MobEffectInstanceHost) -> i32 {
        match self {
            // (_, effect) -> effect.getEffect().value().getColor() | 0xff000000
            ColorMode::EFFECT_COLOR => effect.effect_color() | ALPHA_OPAQUE,
            // (config, effect) -> TextColor.fromLegacyFormat(
            //     effect.getEffect().value().getCategory().getTooltipFormatting())
            //     .getValue() | 0xff000000
            ColorMode::CATEGORY_COLOR => effect.category_format_color() | ALPHA_OPAQUE,
            // (config, effect) -> switch (category) { ... }
            ColorMode::CUSTOM => match effect.category() {
                MobEffectCategory::BENEFICIAL => config.beneficial_foreground_color,
                MobEffectCategory::HARMFUL => config.harmful_foreground_color,
                MobEffectCategory::NEUTRAL => config.neutral_foreground_color,
            },
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ColorMode::EFFECT_COLOR => "EFFECT_COLOR",
            ColorMode::CATEGORY_COLOR => "CATEGORY_COLOR",
            ColorMode::CUSTOM => "CUSTOM",
        }
    }
}

impl fmt::Display for ColorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            i18n::get(&format!(
                "text.autoconfig.status-effect-bars.option.colorMode.{}",
                self.name()
            ))
        )
    }
}
