/*
 * Copyright (c) 2026 A5b84.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License (LGPL-3.0-or-later, per upstream fabric.mod.json), or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this program. If not, see
 * <https://www.gnu.org/licenses/>.
 */
//!
//! Exact 1:1 translation of Status Effect Bars (upstream Java,
//! LGPL-3.0-or-later per fabric.mod.json; Modrinth lists LGPL-3.0-only),
//! master @7c00964 (2026-06-18). The Minecraft/Fabric/cloth-config APIs
//! that do not exist on Horizon are host seams with the same shapes and
//! semantics. The fabric client entrypoint `StatusEffectBars` maps to the
//! carrier's client-init hook (`status_effect_bars::on_initialize_client`);
//! the ModMenu entrypoint (config screen) joins the config-screen batch
//! later, as with the other ports.


pub mod i18n {
use std::sync::RwLock;

type Provider = fn(&str) -> String;

static PROVIDER: RwLock<Option<Provider>> = RwLock::new(None);

/// `I18n.get(key)`. The carrier maps this onto the engine's I18n; the
/// standalone port falls back to the key itself.
pub fn get(key: &str) -> String {
    let provider = PROVIDER.read().unwrap();
    match *provider {
        Some(provider) => provider(key),
        None => key.to_string(),
    }
}

pub fn set_provider_for_tests(provider: Provider) {
    *PROVIDER.write().unwrap() = Some(provider);
}

pub fn clear_provider_for_tests() {
    *PROVIDER.write().unwrap() = None;
}

}


pub mod mth {
/// `Mth.lerpInt(delta, start, end)`: `start + floor(delta * (end - start))`.
pub fn lerp_int(delta: f32, start: i32, end: i32) -> i32 {
    start + (delta * (end - start) as f32).floor() as i32
}

}


pub mod mob_effect_instance_duck {
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

}


pub mod status_effect_bars {
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::sync::MutexGuard;

use super::config::status_effect_bars_config::StatusEffectBarsConfig;

pub const MOD_ID: &str = "status-effect-bars";

/// `StatusEffectBars.config` (set by `onInitializeClient`).
static CONFIG: Mutex<Option<StatusEffectBarsConfig>> = Mutex::new(None);

/// `onInitializeClient`:
/// `AutoConfig.register(StatusEffectBarsConfig.class, GsonConfigSerializer::new);
///  config = AutoConfig.getConfigHolder(StatusEffectBarsConfig.class).getConfig();`
pub fn on_initialize_client() {
    let loaded = StatusEffectBarsConfig::load_or_default();
    *CONFIG.lock().unwrap() = Some(loaded);
}

/// Live handle over the `config` static, like Java's shared reference.
pub fn config() -> ConfigRef {
    ConfigRef(CONFIG.lock().unwrap())
}

pub struct ConfigRef(MutexGuard<'static, Option<StatusEffectBarsConfig>>);

impl Deref for ConfigRef {
    type Target = StatusEffectBarsConfig;

    fn deref(&self) -> &StatusEffectBarsConfig {
        self.0.as_ref().expect("StatusEffectBars.config not initialized")
    }
}

impl DerefMut for ConfigRef {
    fn deref_mut(&mut self) -> &mut StatusEffectBarsConfig {
        self.0.as_mut().expect("StatusEffectBars.config not initialized")
    }
}

/// Resets the static for tests.
pub fn reset_vars_for_tests() {
    *CONFIG.lock().unwrap() = None;
}

}


pub mod status_effect_bar_renderer {
use super::config::status_effect_bars_config::LayoutConfig;
use super::mth;
use super::mob_effect_instance_duck::MobEffectInstanceHost;
use super::status_effect_bars;

/// `net.minecraft.client.gui.GuiGraphicsExtractor` host surface (the one call
/// the renderer makes).
pub trait GraphicsHost {
    fn fill(&self, min_x: i32, min_y: i32, max_x: i32, max_y: i32, color: i32);
}

/// `net.minecraft.client.DeltaTracker` host surface.
pub trait DeltaTrackerHost {
    /// `getGameTimeDeltaPartialTick(false)`.
    fn get_game_time_delta_partial_tick(&self, partially_running_tick: bool) -> f32;
}

/// 1:1 `render` (the `@SuppressWarnings("SuspiciousNameCombination")`
/// width/height swap included).
#[allow(clippy::too_many_arguments)]
pub fn render(
    graphics: &dyn GraphicsHost,
    delta_tracker: Option<&dyn DeltaTrackerHost>,
    effect: &impl MobEffectInstanceHost,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    layout_config: &LayoutConfig,
) {
    // Special cases where the bar is hidden

    if !layout_config.enabled {
        return;
    }

    if effect.get_duration() > status_effect_bars::config().max_remaining_duration
        || effect.is_infinite_duration()
    {
        return; // Too much time remaining
    }

    let age = effect.status_effect_bars_get_max_duration() - effect.get_duration();
    if effect.is_ambient() && age < status_effect_bars::config().min_ambient_age {
        return; // Beacon effect too recent (will probably be refreshed soon)
    }

    let (width, height) = if layout_config.direction.swap_xy() {
        (height, width)
    } else {
        (width, height)
    };

    // start--------+-----end
    // |            |       |
    // +---------middle-----+
    // or
    // start----+
    // |        |
    // |        |
    // +---middle
    // |        |
    // end------+

    let (sx, ex) = (
        layout_config.collinear_offset + layout_config.collinear_padding,
        layout_config.collinear_offset + width - layout_config.collinear_padding,
    );
    let (start_x, end_x) = if layout_config.direction.reverse_axis() {
        (ex, sx)
    } else {
        (sx, ex)
    };

    let tick_delta = match delta_tracker {
        Some(delta_tracker) => delta_tracker.get_game_time_delta_partial_tick(false),
        None => 0.0,
    };
    let progress = (effect.get_duration() as f32 - tick_delta)
        / effect.status_effect_bars_get_max_duration() as f32;
    let middle_x = mth::lerp_int(progress, start_x, end_x);

    let mut start_y = layout_config.orthogonal_offset;
    if layout_config.relative_to_end {
        start_y = height - (start_y + layout_config.thickness);
    }
    let middle_y = start_y + layout_config.thickness;
    let end_y = start_y;

    let (mut start_x, mut middle_x, mut end_x) = (start_x, middle_x, end_x);
    let (mut start_y, mut middle_y, mut end_y) = (start_y, middle_y, end_y);

    if layout_config.direction.swap_xy() {
        // Swapping X and Y to make the bar vertical instead of horizontal
        let mut tmp;

        tmp = start_x;
        start_x = start_y;
        start_y = tmp;

        tmp = middle_x;
        middle_x = middle_y;
        middle_y = tmp;

        tmp = end_x;
        end_x = end_y;
        end_y = tmp;
    }

    start_x += x;
    middle_x += x;
    end_x += x;
    start_y += y;
    middle_y += y;
    end_y += y;

    let config = status_effect_bars::config();
    graphics.fill(start_x, start_y, middle_x, middle_y, config.get_color(effect));
    graphics.fill(middle_x, middle_y, end_x, end_y, config.background_color);
}

}


pub mod config {

pub mod direction {
use std::fmt;

use super::super::i18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    #[default]
    LEFT_TO_RIGHT,
    RIGHT_TO_LEFT,
    BOTTOM_TO_TOP,
    TOP_TO_BOTTOM,
}

impl Direction {
    pub fn swap_xy(&self) -> bool {
        match self {
            Direction::LEFT_TO_RIGHT | Direction::RIGHT_TO_LEFT => false,
            Direction::BOTTOM_TO_TOP | Direction::TOP_TO_BOTTOM => true,
        }
    }

    pub fn reverse_axis(&self) -> bool {
        match self {
            Direction::LEFT_TO_RIGHT | Direction::TOP_TO_BOTTOM => false,
            Direction::RIGHT_TO_LEFT | Direction::BOTTOM_TO_TOP => true,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Direction::LEFT_TO_RIGHT => "LEFT_TO_RIGHT",
            Direction::RIGHT_TO_LEFT => "RIGHT_TO_LEFT",
            Direction::BOTTOM_TO_TOP => "BOTTOM_TO_TOP",
            Direction::TOP_TO_BOTTOM => "TOP_TO_BOTTOM",
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            i18n::get(&format!(
                "text.autoconfig.status-effect-bars.option.direction.{}",
                self.name()
            ))
        )
    }
}

}

pub mod color_mode {
use std::fmt;

use super::super::config::status_effect_bars_config::StatusEffectBarsConfig;
use super::super::i18n;
use super::super::mob_effect_instance_duck::{MobEffectCategory, MobEffectInstanceHost};

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

}

pub mod status_effect_bars_config {
use serde::{Deserialize, Serialize};

use super::super::config::color_mode::ColorMode;
use super::super::config::direction::Direction;
use super::super::mob_effect_instance_duck::MobEffectInstanceHost;
use super::super::status_effect_bars;

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

}

}


pub mod mixins {

pub mod hud {
use super::super::mob_effect_instance_duck::MobEffectInstanceHost;
use super::super::status_effect_bar_renderer::{render, DeltaTrackerHost, GraphicsHost};
use super::super::status_effect_bars;

/// `@Unique private static final int ICON_SIZE = 24;`
pub const ICON_SIZE: i32 = 24;

/// `onExtractEffects` (`extractEffects` at the first `getMobEffectSprite`
/// INVOKE, locals `instance`/`x`/`y`).
pub fn on_extract_effects(
    graphics: &dyn GraphicsHost,
    delta_tracker: Option<&dyn DeltaTrackerHost>,
    instance: &impl MobEffectInstanceHost,
    x: i32,
    y: i32,
) {
    // Copy the layout out of the config static: the Mutex behind the static is
    // not reentrant and render() reads the static again.
    let hud_layout = status_effect_bars::config().hud_layout.clone();
    render(
        graphics,
        delta_tracker,
        instance,
        x,
        y,
        ICON_SIZE,
        ICON_SIZE,
        &hud_layout,
    );
}

}

pub mod living_entity {
use super::super::mob_effect_instance_duck::MobEffectInstanceHost;

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

}

pub mod mob_effect_instance {
use super::super::mob_effect_instance_duck::MobEffectInstanceDuck;

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

}

pub mod effects_in_inventory {
use super::super::mob_effect_instance_duck::MobEffectInstanceHost;
use super::super::status_effect_bar_renderer::{render, GraphicsHost};
use super::super::status_effect_bars;

/// `net.minecraft.client.gui.screens.inventory.EffectsInInventory` host
/// surface; `SPRITE_SQUARE_SIZE` comes from the engine at splice time.
pub trait EffectsInInventoryHost {
    const SPRITE_SQUARE_SIZE: i32;
}

/// `onDrawStatusEffectBackground` (`extractEffects` at the `extractText`
/// INVOKE, locals `y0`/`textureWidth`/`effect`).
pub fn on_draw_status_effect_background<E: EffectsInInventoryHost>(
    graphics: &dyn GraphicsHost,
    effect: &impl MobEffectInstanceHost,
    x0: i32,
    y0: i32,
    texture_width: i32,
) {
    // Copy the layout out of the config static: the Mutex behind the static is
    // not reentrant and render() reads the static again.
    let inventory_layout = status_effect_bars::config().inventory_layout.clone();
    render(
        graphics,
        None,
        effect,
        x0,
        y0,
        texture_width,
        E::SPRITE_SQUARE_SIZE,
        &inventory_layout,
    );
}

}

}


pub mod tests {

use std::cell::RefCell;

use super::config::color_mode::ColorMode;
use super::config::direction::Direction;
use super::config::status_effect_bars_config::{LayoutConfig, StatusEffectBarsConfig};
use super::i18n;
use super::mth;
use super::mixins::effects_in_inventory::{self, EffectsInInventoryHost};
use super::mixins::hud;
use super::mixins::living_entity;
use super::mixins::mob_effect_instance;
use super::mob_effect_instance_duck::{MobEffectCategory, MobEffectInstanceDuck, MobEffectInstanceHost};
use super::status_effect_bar_renderer::{render, DeltaTrackerHost, GraphicsHost};
use super::status_effect_bars;

const ALPHA_OPAQUE: i32 = 0xff000000u32 as i32;

struct FakeEffect {
    duration: i32,
    max_duration: i32,
    infinite: bool,
    ambient: bool,
    amplifier: i32,
    color: i32,
    category: MobEffectCategory,
    format_color: i32,
}

impl FakeEffect {
    fn new(duration: i32, max_duration: i32) -> Self {
        Self {
            duration,
            max_duration,
            infinite: false,
            ambient: false,
            amplifier: 0,
            color: 0x0000ff,
            category: MobEffectCategory::NEUTRAL,
            format_color: 0x5555ff,
        }
    }
}

impl MobEffectInstanceDuck for FakeEffect {
    fn status_effect_bars_get_max_duration(&self) -> i32 {
        self.max_duration
    }

    fn status_effect_bars_set_max_duration(&mut self, max_duration: i32) {
        self.max_duration = max_duration;
    }
}

impl MobEffectInstanceHost for FakeEffect {
    fn get_duration(&self) -> i32 {
        self.duration
    }

    fn set_duration(&mut self, duration: i32) {
        self.duration = duration;
    }

    fn is_infinite_duration(&self) -> bool {
        self.infinite
    }

    fn is_ambient(&self) -> bool {
        self.ambient
    }

    fn get_amplifier(&self) -> i32 {
        self.amplifier
    }

    fn effect_color(&self) -> i32 {
        self.color
    }

    fn category(&self) -> MobEffectCategory {
        self.category
    }

    fn category_format_color(&self) -> i32 {
        self.format_color
    }
}

struct FillRecorder {
    fills: RefCell<Vec<(i32, i32, i32, i32, i32)>>,
}

impl FillRecorder {
    fn new() -> Self {
        Self { fills: RefCell::new(Vec::new()) }
    }

    fn fills(&self) -> Vec<(i32, i32, i32, i32, i32)> {
        self.fills.borrow().clone()
    }
}

impl GraphicsHost for FillRecorder {
    fn fill(&self, min_x: i32, min_y: i32, max_x: i32, max_y: i32, color: i32) {
        self.fills.borrow_mut().push((min_x, min_y, max_x, max_y, color));
    }
}

struct FixedDelta(f32);

impl DeltaTrackerHost for FixedDelta {
    fn get_game_time_delta_partial_tick(&self, _partially_running_tick: bool) -> f32 {
        self.0
    }
}

struct InventorySurface;

impl EffectsInInventoryHost for InventorySurface {
    const SPRITE_SQUARE_SIZE: i32 = 24;
}

fn init_config() {
    status_effect_bars::reset_vars_for_tests();
    status_effect_bars::on_initialize_client();
}

#[test]
fn config_defaults_match_java_field_initializers() {
    init_config();
    let config = status_effect_bars::config();
    assert_eq!(config.color_mode, ColorMode::EFFECT_COLOR);
    assert_eq!(config.background_color, 0x80000000u32 as i32);
    assert_eq!(config.beneficial_foreground_color, 0x80ffffffu32 as i32);
    assert_eq!(config.harmful_foreground_color, config.beneficial_foreground_color);
    assert_eq!(config.neutral_foreground_color, config.beneficial_foreground_color);
    assert_eq!(config.max_remaining_duration, i32::MAX);
    assert_eq!(config.min_ambient_age, 80 + 10);
    assert_eq!(config.hud_layout.collinear_padding, 3);
    assert_eq!(config.hud_layout.orthogonal_offset, 2);
    assert_eq!(config.inventory_layout.collinear_padding, 4);
    assert_eq!(config.inventory_layout.orthogonal_offset, 3);
    for layout in [&config.hud_layout, &config.inventory_layout] {
        assert!(layout.enabled);
        assert_eq!(layout.direction, Direction::LEFT_TO_RIGHT);
        assert!(layout.relative_to_end);
        assert_eq!(layout.thickness, 1);
        assert_eq!(layout.collinear_offset, 0);
    }
}

#[test]
fn config_json_round_trip_and_load_or_default() {
    init_config();
    let config = status_effect_bars::config().clone();
    let text = serde_json::to_string(&config).unwrap();
    let parsed: StatusEffectBarsConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed.background_color, 0x80000000u32 as i32);
    assert_eq!(parsed.hud_layout.collinear_padding, 3);
    // load_or_default with no config file falls back to defaults.
    assert_eq!(StatusEffectBarsConfig::load_or_default().min_ambient_age, 90);
}

#[test]
fn renderer_disabled_layout_draws_nothing() {
    init_config();
    let mut layout = LayoutConfig::default();
    layout.enabled = false;
    let graphics = FillRecorder::new();
    let effect = FakeEffect::new(50, 100);
    render(&graphics, None, &effect, 10, 20, 24, 24, &layout);
    assert!(graphics.fills().is_empty());
}

#[test]
fn renderer_hides_too_much_remaining_and_infinite() {
    init_config();
    status_effect_bars::config().max_remaining_duration = 100;
    let graphics = FillRecorder::new();
    render(&graphics, None, &FakeEffect::new(200, 400), 0, 0, 24, 24, &LayoutConfig::new(3, 2));
    assert!(graphics.fills().is_empty(), "duration above max_remaining_duration hides the bar");
    let mut infinite = FakeEffect::new(50, 100);
    infinite.infinite = true;
    render(&graphics, None, &infinite, 0, 0, 24, 24, &LayoutConfig::new(3, 2));
    assert!(graphics.fills().is_empty(), "infinite duration hides the bar");
}

#[test]
fn renderer_ambient_effect_below_min_age_is_hidden() {
    init_config();
    let graphics = FillRecorder::new();
    let mut effect = FakeEffect::new(50, 100);
    effect.ambient = true; // age = 50 < 90
    render(&graphics, None, &effect, 0, 0, 24, 24, &LayoutConfig::new(3, 2));
    assert!(graphics.fills().is_empty());
    effect.duration = 10; // age = 90 == min_ambient_age -> renders
    render(&graphics, None, &effect, 0, 0, 24, 24, &LayoutConfig::new(3, 2));
    assert_eq!(graphics.fills().len(), 2);
}

#[test]
fn renderer_horizontal_geometry_matches_java_math() {
    init_config();
    let graphics = FillRecorder::new();
    let effect = FakeEffect::new(50, 100); // progress 0.5
    render(&graphics, None, &effect, 10, 20, 24, 24, &LayoutConfig::new(3, 2));
    let fills = graphics.fills();
    assert_eq!(fills.len(), 2);
    // startX = 0+3, endX = 0+24-3, middleX = lerp(0.5, 3, 21) = 12
    // relativeToEnd: startY = 24 - (2+1) = 21, middleY = 22, endY = 21
    assert_eq!(fills[0], (13, 41, 22, 42, 0x0000ff | ALPHA_OPAQUE));
    assert_eq!(fills[1], (22, 42, 31, 41, 0x80000000u32 as i32));
}

#[test]
fn renderer_subtracts_tick_delta_before_progress() {
    init_config();
    let graphics = FillRecorder::new();
    render(&graphics, Some(&FixedDelta(2.0)), &FakeEffect::new(50, 100), 0, 0, 24, 24, &LayoutConfig::new(3, 2));
    // progress = 48/100 = 0.48 -> middle = 3 + floor(0.48*18) = 11
    assert_eq!(graphics.fills()[0], (3, 21, 11, 22, 0x0000ff | ALPHA_OPAQUE));
}

#[test]
fn renderer_reverse_axis_swaps_start_and_end() {
    init_config();
    let graphics = FillRecorder::new();
    let mut layout = LayoutConfig::new(3, 2);
    layout.direction = Direction::RIGHT_TO_LEFT;
    render(&graphics, None, &FakeEffect::new(50, 100), 10, 20, 24, 24, &layout);
    // startX = 21, middleX = 12, endX = 3 (raw, fill normalizes)
    assert_eq!(graphics.fills()[0], (31, 41, 22, 42, 0x0000ff | ALPHA_OPAQUE));
    assert_eq!(graphics.fills()[1], (22, 42, 13, 41, 0x80000000u32 as i32));
}

#[test]
fn renderer_vertical_swaps_xy_after_height_use() {
    init_config();
    let graphics = FillRecorder::new();
    let mut layout = LayoutConfig::new(3, 2);
    layout.direction = Direction::BOTTOM_TO_TOP; // swapXY + reverseAxis
    render(&graphics, None, &FakeEffect::new(50, 100), 5, 7, 30, 20, &layout);
    // width/height swap: width=20, height=30; startY = 30-(2+1) = 27
    // reverseAxis: startX=17, endX=3; middle = lerp(0.5, 17, 3) = 10
    // (startX,startY)=(17,27) middle=(10,28) end=(3,27); swapXY -> (27,17) (28,10) (27,3)
    // + x=5,y=7 -> (32,24) (33,17) (32,10)
    assert_eq!(graphics.fills()[0], (32, 24, 33, 17, 0x0000ff | ALPHA_OPAQUE));
    assert_eq!(graphics.fills()[1], (33, 17, 32, 10, 0x80000000u32 as i32));
}

#[test]
fn color_modes_follow_java_providers() {
    init_config();
    let config = status_effect_bars::config().clone();
    let effect = FakeEffect::new(50, 100);
    assert_eq!(ColorMode::EFFECT_COLOR.get_color(&config, &effect), 0x0000ff | ALPHA_OPAQUE);
    assert_eq!(ColorMode::CATEGORY_COLOR.get_color(&config, &effect), 0x5555ff | ALPHA_OPAQUE);
    assert_eq!(
        ColorMode::CUSTOM.get_color(&config, &effect),
        config.neutral_foreground_color
    );
    let mut beneficial = FakeEffect::new(50, 100);
    beneficial.category = MobEffectCategory::BENEFICIAL;
    assert_eq!(ColorMode::CUSTOM.get_color(&config, &beneficial), config.beneficial_foreground_color);
    let mut harmful = FakeEffect::new(50, 100);
    harmful.category = MobEffectCategory::HARMFUL;
    assert_eq!(ColorMode::CUSTOM.get_color(&config, &harmful), config.harmful_foreground_color);
    assert_eq!(config.get_color(&effect), ColorMode::EFFECT_COLOR.get_color(&config, &effect));
}

#[test]
fn mob_effect_instance_mixin_lifecycle_matches_java() {
    let mut effect = FakeEffect::new(0, 0);
    mob_effect_instance::on_init(&mut effect, 40);
    assert_eq!(effect.status_effect_bars_get_max_duration(), 40);
    let mut copy = FakeEffect::new(0, 0);
    mob_effect_instance::on_copy_from(&mut copy, &effect);
    assert_eq!(copy.status_effect_bars_get_max_duration(), 40);
    let mut take_over = FakeEffect::new(0, 0);
    take_over.status_effect_bars_set_max_duration(60);
    mob_effect_instance::on_upgrade(&mut effect, &take_over);
    assert_eq!(effect.status_effect_bars_get_max_duration(), 60);
}

#[test]
fn living_entity_mixin_keeps_larger_max_for_same_amplifier() {
    let mut new_effect = FakeEffect::new(30, 40);
    let mut previous = FakeEffect::new(100, 120);
    living_entity::on_set_status_effect(&mut new_effect, Some(&previous));
    assert_eq!(new_effect.status_effect_bars_get_max_duration(), 120, "previous max wins when larger");
    let mut shorter = FakeEffect::new(100, 90);
    living_entity::on_set_status_effect(&mut shorter, Some(&previous));
    assert_eq!(shorter.status_effect_bars_get_max_duration(), 120, "Java raises a smaller new max to the previous max (duck.getMaxDuration() < oldDuck.getMaxDuration())");
    let mut other_amplifier = FakeEffect::new(30, 40);
    previous.amplifier = 1;
    living_entity::on_set_status_effect(&mut other_amplifier, Some(&previous));
    assert_eq!(other_amplifier.status_effect_bars_get_max_duration(), 40, "different amplifier leaves the max alone");
    living_entity::on_set_status_effect(&mut other_amplifier, None::<&FakeEffect>);
    assert_eq!(other_amplifier.status_effect_bars_get_max_duration(), 40);
}

#[test]
fn hud_mixin_uses_icon_size_and_hud_layout() {
    init_config();
    let graphics = FillRecorder::new();
    hud::on_extract_effects(&graphics, None, &FakeEffect::new(50, 100), 4, 6);
    assert_eq!(hud::ICON_SIZE, 24);
    // hud layout padding 3 / offset 2, thickness 1, relativeToEnd
    assert_eq!(graphics.fills()[0], (7, 27, 16, 28, 0x0000ff | ALPHA_OPAQUE));
    assert_eq!(graphics.fills()[1], (16, 28, 25, 27, 0x80000000u32 as i32));
}

#[test]
fn effects_in_inventory_mixin_uses_sprite_square_size_and_inventory_layout() {
    init_config();
    let graphics = FillRecorder::new();
    effects_in_inventory::on_draw_status_effect_background::<InventorySurface>(
        &graphics,
        &FakeEffect::new(50, 100),
        100,
        50,
        40,
    );
    // startX = 4, endX = 40-4, middle = lerp(0.5, 4, 36) = 20; SPRITE_SQUARE_SIZE = 24
    // relativeToEnd: startY = 24-(3+1) = 20
    assert_eq!(graphics.fills()[0], (104, 70, 120, 71, 0x0000ff | ALPHA_OPAQUE));
    assert_eq!(graphics.fills()[1], (120, 71, 136, 70, 0x80000000u32 as i32));
}

#[test]
fn mth_lerp_int_floors_the_delta() {
    assert_eq!(mth::lerp_int(0.5, 3, 21), 12);
    assert_eq!(mth::lerp_int(0.48, 3, 21), 11);
    assert_eq!(mth::lerp_int(0.0, 5, 9), 5);
    assert_eq!(mth::lerp_int(1.0, 5, 9), 9);
}

#[test]
fn direction_and_color_mode_display_uses_i18n_keys() {
    static GOT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    GOT.lock().unwrap().clear();
    i18n::set_provider_for_tests(|key| {
        GOT.lock().unwrap().push(key.to_string());
        format!("i18n:{}", key)
    });
    assert_eq!(Direction::BOTTOM_TO_TOP.to_string(), "i18n:text.autoconfig.status-effect-bars.option.direction.BOTTOM_TO_TOP");
    assert_eq!(ColorMode::CUSTOM.to_string(), "i18n:text.autoconfig.status-effect-bars.option.colorMode.CUSTOM");
    assert_eq!(GOT.lock().unwrap().len(), 2);
    i18n::clear_provider_for_tests();
    assert_eq!(Direction::TOP_TO_BOTTOM.to_string(), "text.autoconfig.status-effect-bars.option.direction.TOP_TO_BOTTOM");
}

#[test]
fn direction_axes_match_java_ctor_args() {
    assert!(!Direction::LEFT_TO_RIGHT.swap_xy() && !Direction::LEFT_TO_RIGHT.reverse_axis());
    assert!(!Direction::RIGHT_TO_LEFT.swap_xy() && Direction::RIGHT_TO_LEFT.reverse_axis());
    assert!(Direction::BOTTOM_TO_TOP.swap_xy() && Direction::BOTTOM_TO_TOP.reverse_axis());
    assert!(Direction::TOP_TO_BOTTOM.swap_xy() && !Direction::TOP_TO_BOTTOM.reverse_axis());
}

}
