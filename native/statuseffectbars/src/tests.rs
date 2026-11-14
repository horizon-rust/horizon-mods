//! Port tests: deterministic host doubles standing in for the Minecraft
//! classes the mixins splice into.

use std::cell::RefCell;

use crate::config::color_mode::ColorMode;
use crate::config::direction::Direction;
use crate::config::status_effect_bars_config::{LayoutConfig, StatusEffectBarsConfig};
use crate::i18n;
use crate::mth;
use crate::mixins::effects_in_inventory::{self, EffectsInInventoryHost};
use crate::mixins::hud;
use crate::mixins::living_entity;
use crate::mixins::mob_effect_instance;
use crate::mob_effect_instance_duck::{MobEffectCategory, MobEffectInstanceDuck, MobEffectInstanceHost};
use crate::status_effect_bar_renderer::{render, DeltaTrackerHost, GraphicsHost};
use crate::status_effect_bars;

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
