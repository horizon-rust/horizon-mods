//! Upstream cfg/SmScCfg.java, 1:1: the config template, the static variable
//! imports (into_variables) and the format data corrections.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::cfg_value::{CfgValue, Value};
use crate::cfg_value_builder::{map_f64, CfgValueBuilder};
use crate::new_config::{ConfigHooks, NewConfigBase};
use crate::smooth_sc::SmoothSc;

/// The config statics, exactly the fields SmScCfg declares. Mixins read them
/// every frame, so they are atomics.
pub mod vars {
    use super::*;

    fn set_f32(cell: &AtomicU32, v: f32) {
        cell.store(v.to_bits(), Ordering::Relaxed);
    }
    fn get_f32(cell: &AtomicU32) -> f32 {
        f32::from_bits(cell.load(Ordering::Relaxed))
    }

    pub static HOTBAR_SMOOTHNESS: AtomicU32 = AtomicU32::new(0);
    pub static HOTBAR_ROLLOVER: AtomicBool = AtomicBool::new(false);
    pub static STATIC_SELECTOR: AtomicBool = AtomicBool::new(false);
    pub static CHAT_SMOOTHNESS: AtomicU32 = AtomicU32::new(0);
    pub static CHAT_OPENING_SMOOTHNESS: AtomicU32 = AtomicU32::new(0);
    pub static CHAT_AMOUNT: AtomicU32 = AtomicU32::new(0);
    pub static SUGGESTION_WINDOW_SMOOTHNESS: AtomicU32 = AtomicU32::new(0);
    pub static SUGGESTION_WINDOW_AMOUNT: AtomicU32 = AtomicU32::new(0);
    pub static CREATIVE_SCREEN_SMOOTHNESS: AtomicU32 = AtomicU32::new(0);
    pub static CREATIVE_SCREEN_AMOUNT: AtomicU32 = AtomicU32::new(0);
    pub static CREATIVE_USE_SCISSOR_TEXTURE: AtomicBool = AtomicBool::new(false);
    pub static ENTRY_LIST_SMOOTHNESS: AtomicU32 = AtomicU32::new(0);
    pub static ENTRY_LIST_AMOUNT: AtomicU64 = AtomicU64::new(0);
    pub static TEXT_SMOOTHNESS: AtomicU32 = AtomicU32::new(0);
    pub static TEXT_AMOUNT: AtomicU32 = AtomicU32::new(0);
    pub static TEXT_MARGIN: AtomicU32 = AtomicU32::new(0);
    pub static TEXT_CUSTOM_UPDATE: AtomicBool = AtomicBool::new(false);
    pub static INERTIA_DECELERATION: AtomicU32 = AtomicU32::new(0);
    pub static INERTIA_STOP: AtomicU32 = AtomicU32::new(0);
    pub static INERTIA_THRESHOLD: AtomicU32 = AtomicU32::new(0);
    pub static TOUCHPAD_THRESHOLD: AtomicU32 = AtomicU32::new(0);
    pub static TOUCHPAD_TIME_THRESHOLD: AtomicU32 = AtomicU32::new(0);
    pub static EXPECT_MERGED: AtomicBool = AtomicBool::new(false);
    pub static ENABLE_MASK_DEBUG: AtomicBool = AtomicBool::new(false);

    pub fn hotbar_smoothness() -> f32 {
        get_f32(&HOTBAR_SMOOTHNESS)
    }
    pub fn hotbar_rollover() -> bool {
        HOTBAR_ROLLOVER.load(Ordering::Relaxed)
    }
    pub fn static_selector() -> bool {
        STATIC_SELECTOR.load(Ordering::Relaxed)
    }
    pub fn chat_smoothness() -> f32 {
        get_f32(&CHAT_SMOOTHNESS)
    }
    pub fn chat_opening_smoothness() -> f32 {
        get_f32(&CHAT_OPENING_SMOOTHNESS)
    }
    pub fn chat_amount() -> f32 {
        get_f32(&CHAT_AMOUNT)
    }
    pub fn suggestion_window_smoothness() -> f32 {
        get_f32(&SUGGESTION_WINDOW_SMOOTHNESS)
    }
    pub fn suggestion_window_amount() -> f32 {
        get_f32(&SUGGESTION_WINDOW_AMOUNT)
    }
    pub fn creative_screen_smoothness() -> f32 {
        get_f32(&CREATIVE_SCREEN_SMOOTHNESS)
    }
    pub fn creative_screen_amount() -> f32 {
        get_f32(&CREATIVE_SCREEN_AMOUNT)
    }
    pub fn creative_use_scissor_texture() -> bool {
        CREATIVE_USE_SCISSOR_TEXTURE.load(Ordering::Relaxed)
    }
    pub fn entry_list_smoothness() -> f32 {
        get_f32(&ENTRY_LIST_SMOOTHNESS)
    }
    pub fn entry_list_amount() -> f64 {
        f64::from_bits(ENTRY_LIST_AMOUNT.load(Ordering::Relaxed))
    }
    pub fn text_smoothness() -> f32 {
        get_f32(&TEXT_SMOOTHNESS)
    }
    pub fn text_amount() -> f32 {
        get_f32(&TEXT_AMOUNT)
    }
    pub fn text_margin() -> f32 {
        get_f32(&TEXT_MARGIN)
    }
    pub fn text_custom_update() -> bool {
        TEXT_CUSTOM_UPDATE.load(Ordering::Relaxed)
    }
    pub fn inertia_deceleration() -> f32 {
        get_f32(&INERTIA_DECELERATION)
    }
    pub fn inertia_stop() -> f32 {
        get_f32(&INERTIA_STOP)
    }
    pub fn inertia_threshold() -> f32 {
        get_f32(&INERTIA_THRESHOLD)
    }
    pub fn touchpad_threshold() -> f32 {
        get_f32(&TOUCHPAD_THRESHOLD)
    }
    pub fn touchpad_time_threshold() -> f32 {
        get_f32(&TOUCHPAD_TIME_THRESHOLD)
    }
    pub fn expect_merged() -> bool {
        EXPECT_MERGED.load(Ordering::Relaxed)
    }
    pub fn enable_mask_debug() -> bool {
        ENABLE_MASK_DEBUG.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub fn reset_vars_for_tests() {
        HOTBAR_SMOOTHNESS.store(0, std::sync::atomic::Ordering::Relaxed);
        HOTBAR_ROLLOVER.store(false, std::sync::atomic::Ordering::Relaxed);
        STATIC_SELECTOR.store(false, std::sync::atomic::Ordering::Relaxed);
        CHAT_SMOOTHNESS.store(0, std::sync::atomic::Ordering::Relaxed);
        CHAT_OPENING_SMOOTHNESS.store(0, std::sync::atomic::Ordering::Relaxed);
        CHAT_AMOUNT.store(0, std::sync::atomic::Ordering::Relaxed);
        SUGGESTION_WINDOW_SMOOTHNESS.store(0, std::sync::atomic::Ordering::Relaxed);
        SUGGESTION_WINDOW_AMOUNT.store(0, std::sync::atomic::Ordering::Relaxed);
        CREATIVE_SCREEN_SMOOTHNESS.store(0, std::sync::atomic::Ordering::Relaxed);
        CREATIVE_SCREEN_AMOUNT.store(0, std::sync::atomic::Ordering::Relaxed);
        CREATIVE_USE_SCISSOR_TEXTURE.store(false, std::sync::atomic::Ordering::Relaxed);
        ENTRY_LIST_SMOOTHNESS.store(0, std::sync::atomic::Ordering::Relaxed);
        TEXT_SMOOTHNESS.store(0, std::sync::atomic::Ordering::Relaxed);
        TEXT_AMOUNT.store(0, std::sync::atomic::Ordering::Relaxed);
        TEXT_MARGIN.store(0, std::sync::atomic::Ordering::Relaxed);
        TEXT_CUSTOM_UPDATE.store(false, std::sync::atomic::Ordering::Relaxed);
        INERTIA_DECELERATION.store(0, std::sync::atomic::Ordering::Relaxed);
        INERTIA_STOP.store(0, std::sync::atomic::Ordering::Relaxed);
        INERTIA_THRESHOLD.store(0, std::sync::atomic::Ordering::Relaxed);
        TOUCHPAD_THRESHOLD.store(0, std::sync::atomic::Ordering::Relaxed);
        TOUCHPAD_TIME_THRESHOLD.store(0, std::sync::atomic::Ordering::Relaxed);
        EXPECT_MERGED.store(false, std::sync::atomic::Ordering::Relaxed);
        ENABLE_MASK_DEBUG.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn set_entry_list_smoothness(v: f32) {
        set_f32(&ENTRY_LIST_SMOOTHNESS, v);
    }

    pub(crate) fn store(hotbar_smoothness: f32, values: Vars) {
        set_f32(&HOTBAR_SMOOTHNESS, hotbar_smoothness);
        HOTBAR_ROLLOVER.store(values.hotbar_rollover, Ordering::Relaxed);
        STATIC_SELECTOR.store(values.static_selector, Ordering::Relaxed);
        set_f32(&CHAT_SMOOTHNESS, values.chat_smoothness);
        set_f32(&CHAT_OPENING_SMOOTHNESS, values.chat_opening_smoothness);
        set_f32(&CHAT_AMOUNT, values.chat_amount);
        set_f32(&SUGGESTION_WINDOW_SMOOTHNESS, values.suggestion_window_smoothness);
        set_f32(&SUGGESTION_WINDOW_AMOUNT, values.suggestion_window_amount);
        set_f32(&CREATIVE_SCREEN_SMOOTHNESS, values.creative_screen_smoothness);
        set_f32(&CREATIVE_SCREEN_AMOUNT, values.creative_screen_amount);
        CREATIVE_USE_SCISSOR_TEXTURE.store(values.creative_use_scissor_texture, Ordering::Relaxed);
        set_f32(&ENTRY_LIST_SMOOTHNESS, values.entry_list_smoothness);
        ENTRY_LIST_AMOUNT.store(values.entry_list_amount.to_bits(), Ordering::Relaxed);
        set_f32(&TEXT_SMOOTHNESS, values.text_smoothness);
        set_f32(&TEXT_AMOUNT, values.text_amount);
        set_f32(&TEXT_MARGIN, values.text_margin);
        TEXT_CUSTOM_UPDATE.store(values.text_custom_update, Ordering::Relaxed);
        set_f32(&INERTIA_DECELERATION, values.inertia_deceleration);
        set_f32(&INERTIA_STOP, values.inertia_stop);
        set_f32(&INERTIA_THRESHOLD, values.inertia_threshold);
        set_f32(&TOUCHPAD_THRESHOLD, values.touchpad_threshold);
        set_f32(&TOUCHPAD_TIME_THRESHOLD, values.touchpad_time_threshold);
        EXPECT_MERGED.store(values.expect_merged, Ordering::Relaxed);
        ENABLE_MASK_DEBUG.store(values.enable_mask_debug, Ordering::Relaxed);
    }
}

pub(crate) struct Vars {
    hotbar_rollover: bool,
    static_selector: bool,
    chat_smoothness: f32,
    chat_opening_smoothness: f32,
    chat_amount: f32,
    suggestion_window_smoothness: f32,
    suggestion_window_amount: f32,
    creative_screen_smoothness: f32,
    creative_screen_amount: f32,
    creative_use_scissor_texture: bool,
    entry_list_smoothness: f32,
    entry_list_amount: f64,
    text_smoothness: f32,
    text_amount: f32,
    text_margin: f32,
    text_custom_update: bool,
    inertia_deceleration: f32,
    inertia_stop: f32,
    inertia_threshold: f32,
    touchpad_threshold: f32,
    touchpad_time_threshold: f32,
    expect_merged: bool,
    enable_mask_debug: bool,
}

pub struct SmScCfg {
    pub base: NewConfigBase,
}

/// public final static float format = 2.7f
pub const FORMAT: f32 = 2.7;

fn as_f32(v: &Value) -> Result<f32, String> {
    match v {
        Value::Num(n) => Ok(*n),
        other => Err(format!("not a number: {:?}", other)),
    }
}
fn as_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(format!("not a boolean: {:?}", other)),
    }
}

impl SmScCfg {
    #[allow(clippy::needless_range_loop)]
    fn template() -> Rc<RefCell<CfgValue>> {
        let notes = || {
            CfgValue::builder(
                "Notes",
                Value::Strs(vec![
                    "Smoothness values are in %. Scrolling speed values are in pixels.".to_string(),
                    "0 % means animation off (no smoothness) and bigger values mean slower animation speed (high smoothness).".to_string(),
                    "Press F3+T to update the config.".to_string(),
                    "To access config ingame, use the mod modmenu.".to_string(),
                ]),
            )
            .translatable("smoothscroll.config.notes")
        };
        let smoothness = |default: f32, name: &str, key: &str| {
            CfgValue::builder(name, Value::Num(default))
                .translatable(key)
                .min_max(0.0, 100.0)
                .map(map_f64(0.0), "smoothscroll.config.value.off")
                .map(map_f64(100.0), "smoothscroll.config.value.no_scrolling")
                .step(1.0)
                .format("smoothscroll.config.format.percent")
        };
        let speed = |default: f32, name: &str, key: &str| {
            CfgValue::builder(name, Value::Num(default))
                .translatable(key)
                .min_max(0.0, 100.0)
                .step(1.0)
                .map(map_f64(0.0), "smoothscroll.config.value.auto")
                .format("smoothscroll.config.format.pixels")
        };

        CfgValue::builder(
            "root",
            Value::Cfgs(vec![
                notes().build(),
                CfgValue::builder(
                    "Hotbar",
                    Value::Cfgs(vec![
                        smoothness(20.0, "Smoothness", "smoothscroll.config.hotbar.smoothness").build(),
                        CfgValue::builder("Rollover", Value::Bool(true))
                            .translatable("smoothscroll.config.hotbar.rollover")
                            .build(),
                        CfgValue::builder("Static Selector", Value::Bool(false))
                            .translatable("smoothscroll.config.hotbar.static_selector")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.tooltip.hotbar_static_selector",
                                &[],
                            ))
                            .build(),
                    ]),
                )
                .translatable("smoothscroll.config.hotbar")
                .build(),
                CfgValue::builder(
                    "Chat",
                    Value::Cfgs(vec![
                        smoothness(50.0, "Smoothness", "smoothscroll.config.chat.smoothness").build(),
                        smoothness(50.0, "Opening Smoothness", "smoothscroll.config.chat.opening_smoothness").build(),
                        speed(0.0, "Scrolling Speed", "smoothscroll.config.chat.scrolling_speed").build(),
                        smoothness(50.0, "Suggestion Smoothness", "smoothscroll.config.chat.suggestion_smoothness").build(),
                        speed(30.0, "Suggestion Speed", "smoothscroll.config.chat.suggestion_speed").build(),
                    ]),
                )
                .translatable("smoothscroll.config.chat")
                .build(),
                CfgValue::builder(
                    "Creative Screen",
                    Value::Cfgs(vec![
                        smoothness(50.0, "Smoothness", "smoothscroll.config.creative_screen.smoothness").build(),
                        speed(30.0, "Speed", "smoothscroll.config.creative_screen.speed").build(),
                        CfgValue::builder("RP Compatibility Mode", Value::Bool(false))
                            .translatable("smoothscroll.config.creative_screen.compat_mode")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.creative_screen.compat_mode.tooltip",
                                &[],
                            ))
                            .build(),
                    ]),
                )
                .translatable("smoothscroll.config.creative_screen")
                .build(),
                CfgValue::builder(
                    "Entry List",
                    Value::Cfgs(vec![
                        smoothness(50.0, "Smoothness", "smoothscroll.config.entry_list.smoothness").build(),
                        speed(30.0, "Speed", "smoothscroll.config.entry_list.speed").build(),
                    ]),
                )
                .translatable("smoothscroll.config.entry_list")
                .build(),
                CfgValue::builder(
                    "Text Input Field",
                    Value::Cfgs(vec![
                        smoothness(50.0, "Smoothness", "smoothscroll.config.text_input.smoothness").build(),
                        CfgValue::builder("Speed", Value::Num(100.0))
                            .translatable("smoothscroll.config.text_input.speed")
                            .min_max(0.0, 300.0)
                            .step(1.0)
                            .format("smoothscroll.config.format.pixels")
                            .map(map_f64(0.0), "smoothscroll.config.value.auto")
                            .build(),
                        CfgValue::builder("Cursor Margin", Value::Num(10.0))
                            .translatable("smoothscroll.config.text_input.cursor_margin")
                            .min_max(0.0, 100.0)
                            .step(1.0)
                            .format("smoothscroll.config.format.percent")
                            .disable_when(|val| {
                                let cursor = CfgValue::get(&val, "..");
                                let cursor = CfgValue::get(&cursor, "Custom Cursor Update");
                                !matches!(CfgValue::get_temp_value(&cursor), Value::Bool(true))
                            })
                            .build(),
                        CfgValue::builder("Custom Cursor Update", Value::Bool(true))
                            .translatable("smoothscroll.config.text_input.custom_cursor_update")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.tooltip.custom_cursor_update",
                                &[],
                            ))
                            .build(),
                    ]),
                )
                .translatable("smoothscroll.config.text_input")
                .build(),
                CfgValue::builder(
                    "Touchpad",
                    Value::Cfgs(vec![
                        CfgValue::builder(
                            "Inertia Deceleration",
                            Value::Num(if SmoothSc::is_windows() { 0.0 } else { 90.0 }),
                        )
                        .translatable("smoothscroll.config.touchpad.inertia_deceleration")
                        .tooltip(crate::seams::Component::translatable(
                            "smoothscroll.config.touchpad.inertia_deceleration.tooltip",
                            &[],
                        ))
                        .min_max(0.0, 100.0)
                        .map(map_f64(0.0), "smoothscroll.config.value.off")
                        .map(map_f64(100.0), "smoothscroll.config.value.no_deceleration")
                        .step(1.0)
                        .format("smoothscroll.config.format.percent")
                        .build(),
                        CfgValue::builder("Inertia Stop Threshold", Value::Num(0.005))
                            .translatable("smoothscroll.config.touchpad.inertia_stop")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.touchpad.inertia_stop.tooltip",
                                &[],
                            ))
                            .min_max(0.0, 0.5)
                            .map(map_f64(0.0), "smoothscroll.config.value.off")
                            .step(0.001)
                            .build(),
                        CfgValue::builder("Inertia Start Threshold", Value::Num(0.05))
                            .translatable("smoothscroll.config.touchpad.inertia_start")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.touchpad.inertia_start.tooltip",
                                &[],
                            ))
                            .min_max(0.0, 1.0)
                            .map(map_f64(0.0), "smoothscroll.config.value.off")
                            .step(0.001)
                            .build(),
                        CfgValue::builder("Touchpad Threshold", Value::Num(0.25))
                            .translatable("smoothscroll.config.touchpad.threshold")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.touchpad.threshold.tooltip",
                                &[],
                            ))
                            .min_max(0.0, 5.0)
                            .map(map_f64(0.0), "smoothscroll.config.value.off")
                            .step(0.01)
                            .build(),
                        CfgValue::builder("Touchpad Time Threshold", Value::Num(0.02))
                            .translatable("smoothscroll.config.touchpad.time")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.touchpad.time.tooltip",
                                &[],
                            ))
                            .min_max(0.0, 0.25)
                            .map(map_f64(0.0), "smoothscroll.config.value.off")
                            .step(0.001)
                            .format("smoothscroll.config.format.second")
                            .build(),
                        // Windows tends to add up scroll events when they happen in the same frame
                        // this means we need to have an even less reliable way to tell if the event
                        // is made by a touchpad or a mouse to not appear buggy to regular mouse users
                        CfgValue::builder("Expect Merged Scroll Values", Value::Bool(SmoothSc::is_windows()))
                            .translatable("smoothscroll.config.touchpad.expect_merged")
                            .tooltip(crate::seams::Component::translatable(
                                "smoothscroll.config.touchpad.expect_merged.tooltip",
                                &[],
                            ))
                            .build(),
                    ]),
                )
                .translatable("smoothscroll.config.touchpad")
                .build(),
                CfgValue::builder(
                    "Misc",
                    Value::Cfgs(vec![CfgValue::builder("Enable mask debug", Value::Bool(false))
                        .translatable("smoothscroll.config.misc.enable_mask_debug")
                        .build()]),
                )
                .translatable("smoothscroll.config.misc")
                .build(),
                CfgValue::builder("Format", Value::Num(FORMAT))
                    .translatable("smoothscroll.config.format")
                    .build(),
            ]),
        )
        .build()
    }

    /// Java: public SmScCfg() { super("smoothscroll.json", template); }
    /// (the NewConfig constructor runs loadAndSave, which dispatches to these
    /// overrides, so the Rust port builds the base then loads here).
    pub fn new() -> Rc<RefCell<SmScCfg>> {
        let cfg = Rc::new(RefCell::new(SmScCfg {
            base: NewConfigBase::new("smoothscroll.json", SmScCfg::template()),
        }));
        cfg.borrow().load_and_save();
        cfg
    }

    /// NewConfig.loadAndSave, dispatched to this subclass's overrides.
    pub fn load_and_save(&self) {
        let dont_save = self.base.from_file(self);
        match self.data_correct_permanent() {
            Ok(()) => {
                if !dont_save {
                    self.base.to_file(self);
                }
                match self.into_variables() {
                    Ok(()) => self.data_correct_temporary(),
                    Err(_) => self.problem_reading(),
                }
            }
            Err(_) => self.problem_reading(),
        }
    }

    /// NewConfig.save, dispatched to this subclass's overrides.
    pub fn save(&self) {
        self.base.to_file(self);
        let _ = self.into_variables();
        self.data_correct_temporary();
    }

    pub fn get_config_for_modifying(&self) -> Rc<RefCell<CfgValue>> {
        self.base.get_config_for_modifying()
    }

    fn get_f(&self, section: &str, name: &str) -> Result<f32, String> {
        let root = self.get_config_for_modifying();
        let section = CfgValue::get(&root, section);
        let value = CfgValue::get(&section, name);
        as_f32(&CfgValue::get_value(&value))
    }

    fn get_b(&self, section: &str, name: &str) -> Result<bool, String> {
        let root = self.get_config_for_modifying();
        let section = CfgValue::get(&root, section);
        let value = CfgValue::get(&section, name);
        as_bool(&CfgValue::get_value(&value))
    }

    fn into_variables(&self) -> Result<(), String> {
        let values = Vars {
            hotbar_rollover: self.get_b("Hotbar", "Rollover")?,
            static_selector: self.get_b("Hotbar", "Static Selector")?,
            chat_smoothness: self.get_f("Chat", "Smoothness")? / 100.0,
            chat_opening_smoothness: self.get_f("Chat", "Opening Smoothness")? / 100.0,
            chat_amount: self.get_f("Chat", "Scrolling Speed")?,
            suggestion_window_smoothness: self.get_f("Chat", "Suggestion Smoothness")? / 100.0,
            suggestion_window_amount: self.get_f("Chat", "Suggestion Speed")?,
            creative_screen_smoothness: self.get_f("Creative Screen", "Smoothness")? / 100.0,
            creative_screen_amount: self.get_f("Creative Screen", "Speed")?,
            creative_use_scissor_texture: self.get_b("Creative Screen", "RP Compatibility Mode")?,
            entry_list_smoothness: self.get_f("Entry List", "Smoothness")? / 100.0,
            entry_list_amount: self.get_f("Entry List", "Speed")? as f64,
            text_smoothness: self.get_f("Text Input Field", "Smoothness")? / 100.0,
            text_amount: self.get_f("Text Input Field", "Speed")?,
            text_margin: self.get_f("Text Input Field", "Cursor Margin")?,
            text_custom_update: self.get_b("Text Input Field", "Custom Cursor Update")?,
            inertia_deceleration: self.get_f("Touchpad", "Inertia Deceleration")? / 100.0,
            inertia_stop: self.get_f("Touchpad", "Inertia Stop Threshold")?,
            inertia_threshold: self.get_f("Touchpad", "Inertia Start Threshold")?,
            touchpad_threshold: self.get_f("Touchpad", "Touchpad Threshold")?,
            touchpad_time_threshold: self.get_f("Touchpad", "Touchpad Time Threshold")? * 1_000_000_000.0,
            expect_merged: self.get_b("Touchpad", "Expect Merged Scroll Values")?,
            enable_mask_debug: self.get_b("Misc", "Enable mask debug")?,
        };
        let hotbar_smoothness = self.get_f("Hotbar", "Smoothness")? / 100.0;
        vars::store(hotbar_smoothness, values);
        Ok(())
    }

    // TODO probably would be better to have a getOrMake("") and perform
    // everything in rawRoot and then merge it
    fn data_correct_permanent(&self) -> Result<(), String> {
        let root = self.get_config_for_modifying();
        let raw_root = self.base.raw_root.borrow().clone();
        let cfg_version = CfgValue::get(&raw_root, "cfgVersion");
        if CfgValue::exists(&cfg_version) {
            if let Value::Num(cfgver) = CfgValue::get_value(&cfg_version) {
                SmoothSc::print("Found old format entries in the config file, attempting to update them.");

                for (file_key, section, name) in [
                    ("hotbarSpeed", "Hotbar", "Smoothness"),
                    ("chatSpeed", "Chat", "Smoothness"),
                    ("chatOpeningSpeed", "Chat", "Opening Smoothness"),
                    ("creativeScreenSpeed", "Creative Screen", "Smoothness"),
                ] {
                    let a = CfgValue::get(&raw_root, file_key);
                    if CfgValue::exists(&a) {
                        if let Value::Num(v) = CfgValue::get_value(&a) {
                            if file_key != "chatOpeningSpeed" {
                                SmoothSc::print(&format!("{}", v));
                                if cfgver < 1.6 && v >= 1.0 {
                                    CfgValue::set_value(&a, Value::Num(1.0 / v));
                                }
                            }
                            let section_node = CfgValue::get(&root, section);
                            let target = CfgValue::get(&section_node, name);
                            CfgValue::set_value(&target, CfgValue::get_value(&a));
                        }
                    }
                }
                // entryListSpeed has the extra 1.9 correction
                let a = CfgValue::get(&raw_root, "entryListSpeed");
                if CfgValue::exists(&a) {
                    if let Value::Num(v) = CfgValue::get_value(&a) {
                        SmoothSc::print(&format!("{}", v));
                        if cfgver < 1.6 && v >= 1.0 {
                            CfgValue::set_value(&a, Value::Num(1.0 / v));
                        }
                        if cfgver < 1.9 && v == 0.334 {
                            CfgValue::set_value(&a, Value::Num(0.5));
                        }
                        let entry_list = CfgValue::get(&root, "Entry List");
                        let target = CfgValue::get(&entry_list, "Smoothness");
                        CfgValue::set_value(&target, CfgValue::get_value(&a));
                    }
                }
                let a = CfgValue::get(&raw_root, "enableMaskDebug");
                if CfgValue::exists(&a) {
                    if let Value::Num(_) = CfgValue::get_value(&a) {
                        // upstream writes this into Chat/Smoothness (kept 1:1)
                        let chat = CfgValue::get(&root, "Chat");
                        let target = CfgValue::get(&chat, "Smoothness");
                        CfgValue::set_value(&target, CfgValue::get_value(&a));
                    }
                }
            }
        }
        // New file format corrections go here
        // the corrections can try to be performed when the values aren't set,
        // so every correction should be done from rawRoot and inside the try block
        // to not perform corrections on new default values
        // this could also be done with if blocks but i didn't want to be more
        // verbose than i already am

        let format_node = CfgValue::get(&root, "Format");
        let prev_format = match as_f32(&CfgValue::get_value(&format_node)) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        if prev_format > FORMAT {
            return Err("Incompatible config file format".to_string());
        }
        if prev_format < 2.4 {
            let apply = |section: &str, names: &[(&str, &str)]| {
                let merged = CfgValue::get(&root, section);
                let raw = CfgValue::get(&raw_root, section);
                for (merged_name, raw_name) in names {
                    let raw_value = CfgValue::get(&raw, raw_name);
                    if let Value::Num(v) = CfgValue::get_value(&raw_value) {
                        let target = CfgValue::get(&merged, merged_name);
                        CfgValue::set_value(&target, Value::Num(v * 100.0));
                    }
                }
            };
            apply("Hotbar", &[("Smoothness", "Smoothness")]);
            apply(
                "Chat",
                &[
                    ("Smoothness", "Smoothness"),
                    ("Opening Smoothness", "Opening Speed"),
                    ("Suggestion Smoothness", "Suggestion Smoothness"),
                ],
            );
            apply("Creative Screen", &[("Smoothness", "Smoothness")]);
            apply("Entry List", &[("Smoothness", "Smoothness")]);
        }

        if prev_format < 2.41 {
            // previously if you updated and didn't have suggestion
            // smoothness set (possible without tinkering),
            // it would data correct the new default value
            let chat = CfgValue::get(&root, "Chat");
            let suggestion = CfgValue::get(&chat, "Suggestion Smoothness");
            if let Ok(v) = as_f32(&CfgValue::get_value(&suggestion)) {
                if v == 5000.0 {
                    CfgValue::set_value(&suggestion, Value::Num(50.0));
                }
            }
        }

        // Notes and Format should always be up to date and not modified
        CfgValue::default_to_temp(&CfgValue::get(&root, "Notes"));
        CfgValue::save_temp_value(&CfgValue::get(&root, "Notes"));
        CfgValue::default_to_temp(&CfgValue::get(&root, "Format"));
        CfgValue::save_temp_value(&CfgValue::get(&root, "Format"));

        Ok(())
    }

    fn data_correct_temporary(&self) {
        // Disable entry list smooth scrolling if the mod smooth scrolling refurbished is present
        if crate::smooth_sc::is_smooth_scrolling_refurbished_loaded() {
            crate::sm_sc_cfg::vars::set_entry_list_smoothness(0.0);
        }
    }

    fn problem_reading(&self) {
        SmoothSc::print("There was a problem reading the config file. Using default values.");
    }
    fn problem_writing(&self) {
        SmoothSc::print("There was a problem writing to the config file.");
    }
    fn file_not_found(&self) {
        self.base.to_file(&FileNotFoundHooks);
        SmoothSc::print("There is no config file, creating a new one.");
    }
}

/// The ConfigHooks adapter so NewConfigBase::from_file reaches these overrides
/// (Java does this with inheritance).
struct FileNotFoundHooks;
impl ConfigHooks for FileNotFoundHooks {}

impl ConfigHooks for SmScCfg {
    fn problem_writing(&self) {
        SmScCfg::problem_writing(self);
    }
    fn problem_reading(&self) {
        SmScCfg::problem_reading(self);
    }
    fn file_not_found(&self) {
        SmScCfg::file_not_found(self);
    }
    fn data_correct_permanent(&self) -> Result<(), String> {
        SmScCfg::data_correct_permanent(self)
    }
    fn into_variables(&self) -> Result<(), String> {
        SmScCfg::into_variables(self)
    }
    fn data_correct_temporary(&self) {
        SmScCfg::data_correct_temporary(self);
    }
}
