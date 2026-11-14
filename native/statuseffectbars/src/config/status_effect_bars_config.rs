//! `io.github.a5b84.statuseffectbars.config.StatusEffectBarsConfig`, 1:1.
//! The cloth-config annotations are GUI metadata for the config screen batch;
//! the data and defaults are 1:1.

use serde::{Deserialize, Serialize};

use crate::config::color_mode::ColorMode;
use crate::config::direction::Direction;
use crate::mob_effect_instance_duck::MobEffectInstanceHost;
use crate::status_effect_bars;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusEffectBarsConfig {
    // Color
    /// `@EnumHandler(option = EnumDisplayOption.BUTTON)`
    pub color_mode: ColorMode,
    /// `@ColorPicker(allowAlpha = true)` = `0x80000000`
    pub background_color: i32,
    /// `@ColorPicker(allowAlpha = true)` = `0x80ffffff`
    pub beneficial_foreground_color: i32,
    /// `@ColorPicker(allowAlpha = true)` = `beneficialForegroundColor`
    pub harmful_foreground_color: i32,
    /// `@ColorPicker(allowAlpha = true)` = `beneficialForegroundColor`
    pub neutral_foreground_color: i32,

    // Behavior
    /// Remaining duration in ticks above which the bar is hidden.
    pub max_remaining_duration: i32,
    /// Age in ticks under which the bar of ambient (i.e. beacon) effects is
    /// hidden. @see BeaconBlockEntity#tick
    pub min_ambient_age: i32,

    // Layout
    /// `@Category("hud") @TransitiveObject` = `new LayoutConfig(3, 2)`
    pub hud_layout: LayoutConfig,
    /// `@Category("inventory") @TransitiveObject` = `new LayoutConfig(4, 3)`
    pub inventory_layout: LayoutConfig,
}

impl Default for StatusEffectBarsConfig {
    fn default() -> Self {
        let beneficial_foreground_color = 0x80ffffffu32 as i32;
        Self {
            color_mode: ColorMode::EFFECT_COLOR,
            background_color: 0x80000000u32 as i32,
            beneficial_foreground_color,
            harmful_foreground_color: beneficial_foreground_color,
            neutral_foreground_color: beneficial_foreground_color,
            max_remaining_duration: i32::MAX,
            min_ambient_age: 80 + 10,
            hud_layout: LayoutConfig::new(3, 2),
            inventory_layout: LayoutConfig::new(4, 3),
        }
    }
}

impl StatusEffectBarsConfig {
    /// `getColor(MobEffectInstance)`.
    pub fn get_color(&self, effect: &impl MobEffectInstanceHost) -> i32 {
        self.color_mode.get_color(self, effect)
    }

    /// `AutoConfig.register(..., GsonConfigSerializer::new)` +
    /// `getConfigHolder(...).getConfig()`: read
    /// `config/status-effect-bars.json`, fall back to defaults.
    pub fn load_or_default() -> StatusEffectBarsConfig {
        let path = std::path::Path::new("config").join(format!("{}.json", status_effect_bars::MOD_ID));
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => StatusEffectBarsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    /// enabled = true
    pub enabled: bool,
    // Position
    /// direction = LEFT_TO_RIGHT
    pub direction: Direction,
    /// Whether the bar is placed relative to the end (bottom/right) or the
    /// start (top/left) of the effect rectangle. relativeToEnd = true
    pub relative_to_end: bool,
    // Shape
    /// thickness = 1
    pub thickness: i32,
    /// collinearPadding (ctor arg)
    pub collinear_padding: i32,
    // Fine-tuning
    /// Offset along the bar's main axis. collinearOffset = 0
    pub collinear_offset: i32,
    /// Offset perpendicular to the bar (ctor arg)
    pub orthogonal_offset: i32,
}

impl LayoutConfig {
    /// `LayoutConfig(int collinearPadding, int orthogonalOffset)`.
    pub fn new(collinear_padding: i32, orthogonal_offset: i32) -> Self {
        Self {
            enabled: true,
            direction: Direction::LEFT_TO_RIGHT,
            relative_to_end: true,
            thickness: 1,
            collinear_padding,
            collinear_offset: 0,
            orthogonal_offset,
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        // Java has no no-arg ctor; zero padding/offset stand in for serde's.
        Self::new(0, 0)
    }
}
