//! Port tests asserting the upstream behavior of the translated code.

use std::rc::Rc;

use crate::cfg_value::{CfgValue, MapKey, Value};
use crate::sm_sc_cfg::{vars, SmScCfg};
use crate::smooth_sc::{set_touchpad_scrolled_y, SmoothSc, TOUCHPAD_SCROLLED_Y};

fn num(v: f32) -> Value {
    Value::Num(v)
}

/// The SmScCfg variables are process-global statics (Java statics), so every
/// test that loads a config file takes this lock to run serially.
static CONFIG_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn lock_config() -> std::sync::MutexGuard<'static, ()> {
    CONFIG_MUTEX.lock().unwrap()
}

/// Each config test starts from a clean variable state (Java uninitialized
/// statics).
fn reset_vars() {
    crate::sm_sc_cfg::vars::reset_vars_for_tests();
}

fn use_config_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("smoothscroll-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    dir
}

#[test]
fn builder_defaults_match_java_field_initializers() {
    let v = CfgValue::builder("Test", num(1.0)).build();
    let b = v.borrow();
    assert_eq!(b.step.get(), 0.01);
    assert_eq!(b.min_val.get(), 0.0);
    assert_eq!(b.max_val.get(), 0.0);
    assert_eq!(*b.format_key.borrow(), "smoothscroll.config.format.default");
    // translationMap = {true: "options.on", false: "options.off"}
    let map = b.translation_map.borrow();
    assert_eq!(map.get(&MapKey::B(true)).unwrap(), "options.on");
    assert_eq!(map.get(&MapKey::B(false)).unwrap(), "options.off");
}

#[test]
fn builder_overrides_stick() {
    let v = CfgValue::builder("Test", num(1.0))
        .step(1.0)
        .min_max(0.0, 100.0)
        .format("fmt")
        .translatable("key")
        .build();
    let b = v.borrow();
    assert_eq!(b.step.get(), 1.0);
    assert_eq!(b.min_val.get(), 0.0);
    assert_eq!(b.max_val.get(), 100.0);
    assert_eq!(*b.format_key.borrow(), "fmt");
    assert_eq!(*b.translation_key.borrow(), Some("key".to_string()));
}

#[test]
fn get_finds_named_children_and_returns_fakes_otherwise() {
    let child = CfgValue::builder("Smoothness", num(20.0)).build();
    let section = CfgValue::builder("Hotbar", Value::Cfgs(vec![child])).build();
    let found = CfgValue::get(&section, "Smoothness");
    assert!(CfgValue::exists(&found));
    assert_eq!(CfgValue::get_value(&found), num(20.0));

    let missing = CfgValue::get(&section, "Nope");
    assert!(!CfgValue::exists(&missing));

    // a leaf has no list, so get() returns a fake there too
    let leaf_missing = CfgValue::get(&found, "Anything");
    assert!(!CfgValue::exists(&leaf_missing));
}

#[test]
fn get_dotdot_climbs_to_the_parent() {
    let child = CfgValue::builder("Smoothness", num(20.0)).build();
    let section = CfgValue::builder("Hotbar", Value::Cfgs(vec![child.clone()])).build();
    // the constructor links children to their parent
    let up = CfgValue::get(&child, "..");
    assert!(CfgValue::exists(&up));
    assert_eq!(CfgValue::get_name(&up), "Hotbar");
    assert!(Rc::ptr_eq(&up, &section));
    let _ = section;
}

#[test]
fn root_has_no_parent_so_dotdot_gives_a_fake() {
    let root = CfgValue::builder("root", Value::Cfgs(vec![])).build();
    assert!(!CfgValue::exists(&CfgValue::get(&root, "..")));
}

#[test]
fn temp_value_flow_matches_java() {
    let v = CfgValue::builder("V", num(1.0)).build();
    // setValue updates both current and temporary
    CfgValue::set_value(&v, num(5.0));
    assert_eq!(CfgValue::get_temp_value(&v), num(5.0));
    CfgValue::set_temp_value(&v, num(9.0));
    assert_eq!(CfgValue::get_temp_value(&v), num(9.0));
    assert_eq!(CfgValue::get_value(&v), num(5.0));
    CfgValue::save_temp_value(&v);
    assert_eq!(CfgValue::get_value(&v), num(9.0));
    CfgValue::reset_temp_value(&v);
    assert_eq!(CfgValue::get_temp_value(&v), num(9.0));
}

#[test]
fn step_and_minmax_math_matches_upstream() {
    let v = CfgValue::builder("S", num(20.0)).step(1.0).min_max(0.0, 100.0).build();
    assert_eq!(CfgValue::en_helf_step(&v, 20.6), 21);
    assert_eq!(CfgValue::en_step(&v, 20.6), 21.0);
    // enMinMax: val * (max - min) + min
    assert_eq!(CfgValue::en_min_max(&v, 0.2), 20.0);
    // deMinMax: (val - min) / max  (the upstream formula, kept 1:1)
    assert_eq!(CfgValue::de_min_max(&v, 20.0), 0.2);
    let _ = &v;
}

#[test]
fn try_translate_matches_booleans_and_misses_float_keys_like_java() {
    let v = CfgValue::builder("B", Value::Bool(true)).build();
    let t = CfgValue::try_translate(&v, &Value::Bool(false));
    assert_eq!(t.key, "options.off");

    // Float values cannot match the boxed Double keys upstream
    let w = CfgValue::builder("F", num(0.0))
        .map(MapKey::D(0.0), "zero.key")
        .build();
    let t = CfgValue::try_translate(&w, &num(0.0));
    assert!(t.literal.is_some(), "a float must fall through to a literal");
    assert_eq!(t.text(), "0");
}

#[test]
fn parse_json_covers_primitives_and_objects_in_order() {
    let json: serde_json::Value = serde_json::from_str(
        r#"{"A": "str", "B": true, "C": 1.5, "D": {"D1": false, "D2": 2}}"#,
    )
    .unwrap();
    let parsed = CfgValue::parse_json("root", Some(json));
    let list = CfgValue::get_list(&parsed).unwrap();
    let names: Vec<String> = list.iter().map(|c| CfgValue::get_name(c)).collect();
    assert_eq!(names, vec!["A", "B", "C", "D"]);
    assert_eq!(CfgValue::get_value(&list[0]), Value::Str("str".to_string()));
    assert_eq!(CfgValue::get_value(&list[1]), Value::Bool(true));
    assert_eq!(CfgValue::get_value(&list[2]), num(1.5));
    let d = CfgValue::get_list(&list[3]).unwrap();
    assert_eq!(CfgValue::get_value(&d[0]), Value::Bool(false));
    assert_eq!(CfgValue::get_value(&d[1]), num(2.0));
}

#[test]
fn export_json_round_trips_parse() {
    let json: serde_json::Value = serde_json::from_str(
        r#"{"A": "str", "B": true, "C": 1.5, "D": {"D1": false, "D2": 2}}"#,
    )
    .unwrap();
    let parsed = CfgValue::parse_json("root", Some(json));
    let out = CfgValue::export_json(&parsed);
    assert_eq!(
        serde_json::to_string(&out).unwrap(),
        r#"{"A":"str","B":true,"C":1.5,"D":{"D1":false,"D2":2.0}}"#
    );
}

#[test]
fn match_values_copies_leaves_and_skips_unknown_and_mismatched() {
    let template = CfgValue::builder(
        "root",
        Value::Cfgs(vec![
            CfgValue::builder("Section", Value::Cfgs(vec![
                CfgValue::builder("A", num(1.0)).build(),
                CfgValue::builder("B", Value::Bool(false)).build(),
            ])).build(),
        ]),
    )
    .build();

    let file: serde_json::Value = serde_json::from_str(
        r#"{"Section": {"A": 7.0, "B": true, "Unknown": 3.0}}"#,
    )
    .unwrap();
    let raw = CfgValue::parse_json("root", Some(file));

    CfgValue::match_values(&raw, &template);
    let section = CfgValue::get(&template, "Section");
    let a = CfgValue::get(&section, "A");
    let b = CfgValue::get(&section, "B");
    assert_eq!(CfgValue::get_value(&a), num(7.0));
    assert_eq!(CfgValue::get_value(&b), Value::Bool(true));
    // the template shape is untouched by the unknown file key
    assert_eq!(CfgValue::get_list(&section).unwrap().len(), 2);
}

#[test]
fn match_values_skips_null_source_values() {
    let template = CfgValue::builder("T", num(1.0)).build();
    let raw = CfgValue::parse_json("raw", None); // null value
    CfgValue::match_values(&raw, &template);
    assert_eq!(CfgValue::get_value(&template), num(1.0));
}

#[test]
fn smooth_scroll_config_defaults_load_without_a_file() {
    let _g = lock_config();
    let dir = std::env::temp_dir().join(format!("smoothscroll-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    TOUCHPAD_SCROLLED_Y.set(false);

    let cfg = SmScCfg::new();
    // a fresh config file was created
    assert!(dir.join("smoothscroll.json").exists());
    // into_variables ran: hotbar 20f -> 0.2, chat 50f -> 0.5, touchpad defaults
    assert_eq!(crate::sm_sc_cfg::vars::hotbar_smoothness(), 0.2);
    assert!(crate::sm_sc_cfg::vars::hotbar_rollover());
    assert_eq!(crate::sm_sc_cfg::vars::chat_smoothness(), 0.5);
    assert_eq!(crate::sm_sc_cfg::vars::chat_amount(), 0.0);
    assert_eq!(crate::sm_sc_cfg::vars::suggestion_window_amount(), 30.0);
    assert_eq!(crate::sm_sc_cfg::vars::text_amount(), 100.0);
    assert_eq!(crate::sm_sc_cfg::vars::inertia_deceleration(), 0.9);
    assert!((crate::sm_sc_cfg::vars::touchpad_time_threshold() - 20_000_000.0).abs() < 1000.0);
    assert!(!crate::sm_sc_cfg::vars::enable_mask_debug());
    let _ = cfg;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saved_values_reload_from_the_file() {
    let _g = lock_config();
    let dir = std::env::temp_dir().join(format!("smoothscroll-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());

    let cfg = SmScCfg::new();
    let root = cfg.borrow().get_config_for_modifying();
    let hotbar = CfgValue::get(&root, "Hotbar");
    let smoothness = CfgValue::get(&hotbar, "Smoothness");
    CfgValue::set_temp_value(&smoothness, num(80.0));
    CfgValue::save_temp_value(&smoothness);
    cfg.borrow().save();
    assert_eq!(crate::sm_sc_cfg::vars::hotbar_smoothness(), 0.8);

    // a fresh load reads the saved file back
    let cfg2 = SmScCfg::new();
    assert_eq!(crate::sm_sc_cfg::vars::hotbar_smoothness(), 0.8);
    let _ = cfg2;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_format_config_data_corrects() {
    let _g = lock_config();
    let dir = std::env::temp_dir().join(format!("smoothscroll-old-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    std::fs::write(
        dir.join("smoothscroll.json"),
        r#"{"cfgVersion": 1.5, "hotbarSpeed": 2.0, "chatSpeed": 4.0, "entryListSpeed": 0.334, "Format": 2.7}"#,
    )
    .unwrap();

    SmScCfg::new();
    // cfgVersion 1.5 < 1.6, so speeds >= 1 invert (2.0 -> 0.5, 4.0 -> 0.25);
    // entryListSpeed 0.334 -> 0.5 at < 1.9; into_variables then divides by 100
    assert_eq!(crate::sm_sc_cfg::vars::hotbar_smoothness(), 0.005);
    assert_eq!(crate::sm_sc_cfg::vars::chat_smoothness(), 0.0025);
    assert_eq!(crate::sm_sc_cfg::vars::entry_list_smoothness(), 0.005);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn incompatible_config_format_falls_back_to_problem_reading() {
    let _g = lock_config();
    reset_vars();
    let dir = std::env::temp_dir().join(format!("smoothscroll-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    std::fs::write(
        dir.join("smoothscroll.json"),
        r#"{"Format": 9.9}"#,
    )
    .unwrap();

    SmScCfg::new();
    // loadAndSave caught the RuntimeException: into_variables never ran,
    // so the statics keep their previous values
    assert_eq!(crate::sm_sc_cfg::vars::hotbar_smoothness(), 0.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn notes_and_format_always_reset_to_the_built_in_defaults() {
    let _g = lock_config();
    let dir = std::env::temp_dir().join(format!("smoothscroll-notes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    // a file with a mutated Notes entry (parse leaves arrays unhandled, so it
    // stays null and matchValues skips it)
    std::fs::write(
        dir.join("smoothscroll.json"),
        r#"{"Notes": ["hacked"], "Format": 2.0, "Hotbar": {"Smoothness": 33.0}}"#,
    )
    .unwrap();
    SmScCfg::new();
    let cfg = SmScCfg::new();
    let root = cfg.borrow().get_config_for_modifying();
    let notes = CfgValue::get(&root, "Notes");
    match CfgValue::get_value(&notes) {
        Value::Strs(lines) => assert_eq!(lines.len(), 4),
        other => panic!("notes should stay the string list, got {:?}", other),
    }
    assert_eq!(crate::sm_sc_cfg::vars::hotbar_smoothness(), 33.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn disable_when_gates_the_cursor_margin_slider() {
    use crate::cfg_value::WidgetHandle;
    let dir = std::env::temp_dir().join(format!("smoothscroll-disable-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    let cfg = SmScCfg::new();
    let root = cfg.borrow().get_config_for_modifying();
    let text = CfgValue::get(&root, "Text Input Field");
    let cursor_margin = CfgValue::get(&text, "Cursor Margin");
    let custom = CfgValue::get(&text, "Custom Cursor Update");

    let [widget, reset] = CfgValue::generate_widget(&cursor_margin);
    CfgValue::generate_widget(&custom);
    // Custom Cursor Update defaults to true, so the margin is enabled
    CfgValue::refresh_disable_recursive(&text);
    let widget = widget.unwrap();
    let active = match &widget {
        WidgetHandle::Slider(s) => s.borrow().base.base.active.get(),
        _ => panic!("expected a slider"),
    };
    assert!(active);
    if let WidgetHandle::Button(r) = reset.unwrap() {
        assert!(r.borrow().base.active.get());
    }

    // flip the toggle: the margin disables
    CfgValue::set_temp_value(&custom, Value::Bool(false));
    CfgValue::refresh_disable_recursive(&text);
    let active = match &widget {
        WidgetHandle::Slider(s) => s.borrow().base.base.active.get(),
        _ => panic!("expected a slider"),
    };
    assert!(!active);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn slider_writes_stepped_temp_values() {
    use crate::cfg_value::WidgetHandle;
    let dir = std::env::temp_dir().join(format!("smoothscroll-slider-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    let cfg = SmScCfg::new();
    let root = cfg.borrow().get_config_for_modifying();
    let hotbar = CfgValue::get(&root, "Hotbar");
    let smoothness = CfgValue::get(&hotbar, "Smoothness");

    let [Some(WidgetHandle::Slider(slider)), _] = CfgValue::generate_widget(&smoothness) else {
        panic!("expected a slider");
    };
    // drag to the 0.37 position: value maps back through enStep
    slider.borrow().base.value.set(0.37);
    slider.borrow().apply_value();
    assert_eq!(CfgValue::get_temp_value(&smoothness), num(37.0));
    // the message text carries the translated format key with the display name
    let msg = slider.borrow().base.message.borrow().clone();
    assert_eq!(msg.key, "smoothscroll.config.format.percent");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn boolean_button_toggles_its_temp_value() {
    use crate::cfg_value::WidgetHandle;
    let dir = std::env::temp_dir().join(format!("smoothscroll-btn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    let cfg = SmScCfg::new();
    let root = cfg.borrow().get_config_for_modifying();
    let hotbar = CfgValue::get(&root, "Hotbar");
    let rollover = CfgValue::get(&hotbar, "Rollover");

    let [Some(WidgetHandle::Button(button)), _] = CfgValue::generate_widget(&rollover) else {
        panic!("expected a button");
    };
    assert_eq!(CfgValue::get_temp_value(&rollover), Value::Bool(true));
    crate::seams::Button::click(&button);
    assert_eq!(CfgValue::get_temp_value(&rollover), Value::Bool(false));
    crate::seams::Button::click(&button);
    assert_eq!(CfgValue::get_temp_value(&rollover), Value::Bool(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn globals_and_smooth_sc_helpers() {
    set_touchpad_scrolled_y(true);
    assert!(TOUCHPAD_SCROLLED_Y.get());
    set_touchpad_scrolled_y(false);
    assert!(!TOUCHPAD_SCROLLED_Y.get());

    assert_eq!(SmoothSc::clamp(5, 0, 3), 3);
    assert_eq!(SmoothSc::clamp(-2, 0, 3), 0);

    use crate::seams::UVPair;
    let a = UVPair::new(1.0, 2.0);
    let b = UVPair::new(3.0, 4.0);
    assert_eq!(SmoothSc::vec2f_add(a, b), UVPair::new(4.0, 6.0));
    assert_eq!(SmoothSc::vec2f_sub(b, a), UVPair::new(2.0, 2.0));
    assert_eq!(SmoothSc::vec2f_mul(a, 2.0), UVPair::new(2.0, 4.0));
}

#[test]
fn mixin_plugin_gates_condensed_creative_only() {
    use crate::mixins::mixin_plugin::should_apply_mixin;
    assert!(should_apply_mixin("net.minecraft.X", "io.github.smajloslovakian.smoothscroll.mixin.client.Chat.ChatComponentMixin"));
    assert!(should_apply_mixin(
        "net.minecraft.X",
        "io.github.smajloslovakian.smoothscroll.mixin.client.CreativeScreen.CreativeModeInventoryScreenMixin"
    ));
    // the CC handler applies only when condensed_creative is loaded
    // (the seam defaults to not loaded)
    assert!(!should_apply_mixin(
        "net.minecraft.X",
        "io.github.smajloslovakian.smoothscroll.mixin.client.CreativeScreen.CreativeScreenHandlerMixin_CC"
    ));
}

#[test]
fn button_reset_reverts_to_the_default() {
    use crate::cfg_value::WidgetHandle;
    let dir = std::env::temp_dir().join(format!("smoothscroll-reset-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    let cfg = SmScCfg::new();
    let root = cfg.borrow().get_config_for_modifying();
    let hotbar = CfgValue::get(&root, "Hotbar");
    let smoothness = CfgValue::get(&hotbar, "Smoothness");
    CfgValue::set_temp_value(&smoothness, num(80.0));

    let [widget, reset] = CfgValue::generate_widget(&smoothness);
    let _ = widget;
    if let WidgetHandle::Button(reset) = reset.unwrap() {
        crate::seams::Button::click(&reset);
    }
    assert_eq!(CfgValue::get_temp_value(&smoothness), num(20.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_screen_generates_rows_for_every_section() {
    use crate::menu::smooth_scroll_config_screen::{Row, SmoothScrollConfigScreen, ScreenHost};
    struct Host {
        pairs: usize,
        wides: usize,
        closed_to: Option<u64>,
    }
    impl ScreenHost for Host {
        fn add_renderable_widget(&mut self, _w: crate::cfg_value::WidgetHandle) {}
        fn set_screen(&mut self, parent: u64) {
            self.closed_to = Some(parent);
        }
        fn content_height(&self) -> i32 {
            200
        }
        fn add_row_pair(&mut self, _l: crate::cfg_value::WidgetHandle, _r: crate::cfg_value::WidgetHandle) {
            self.pairs += 1;
        }
        fn add_wide(&mut self, _w: crate::cfg_value::WidgetHandle, _span: i32) {
            self.wides += 1;
        }
    }

    let dir = std::env::temp_dir().join(format!("smoothscroll-screen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::seams::set_config_dir(dir.clone());
    let cfg = SmScCfg::new();
    // onInitializeClient sets SmoothSc.cfg; the config screen reads it there
    crate::smooth_sc::CFG.with(|c| *c.borrow_mut() = Some(cfg.clone()));

    let screen = SmoothScrollConfigScreen::new(7);
    let mut host = Host { pairs: 0, wides: 0, closed_to: None };
    screen.init(&mut host);
    // sections with lists: Hotbar, Chat, Creative Screen, Entry List,
    // Text Input Field, Touchpad, Misc = 7 headers + 7 blank-string trailers
    assert_eq!(host.wides, 14);
    // pairs: 3 + 5 + 3 + 2 + 4 + 6 + 1 = 24 config rows, plus the save/exit pair
    assert_eq!(host.pairs, 25);
    assert_eq!(screen.rows.borrow().len(), 24);
    assert!(matches!(screen.rows.borrow()[0], Row::Pair(_, _)));

    // the save button saves the config; the exit button just closes
    let b1 = screen.button1.borrow().clone().unwrap();
    crate::seams::Button::click(&b1);
    screen.on_close(&mut host);
    assert_eq!(host.closed_to, Some(7));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// mixin/client/Miscellaneous + PreciseScissor
// ---------------------------------------------------------------------------

use crate::mixins::miscellaneous::render_pass_mixin::RenderPassMixin;
use crate::mixins::miscellaneous::sdl_event_handler_mixin::SdlEventHandlerMixin;
use crate::mixins::miscellaneous::transformation_mixin::TransformationMixin;
use crate::seams::{MouseHost, RenderArea, ScreenRectangle, SdlEvent};
use crate::transformation_access::Parameters;

struct FrameEnv(f32);
impl crate::seams::ClientEnv for FrameEnv {
    fn get_game_time_delta_ticks(&self) -> f32 {
        self.0
    }
    fn font(&self) -> String {
        String::new()
    }
}

struct MouseRecorder {
    window: u64,
    scrolls: Rc<std::cell::RefCell<Vec<(u64, f64, f64)>>>,
    tasks: std::cell::RefCell<Vec<Box<dyn FnOnce()>>>,
}

impl MouseRecorder {
    fn run_all(&self) {
        while let Some(t) = self.tasks.borrow_mut().pop() {
            t();
        }
    }
}

impl MouseHost for MouseRecorder {
    fn execute(&self, task: Box<dyn FnOnce()>) {
        self.tasks.borrow_mut().push(task);
    }
    fn window_handle(&self) -> u64 {
        self.window
    }
    fn on_scroll(&self, window: u64, x: f64, y: f64) {
        self.scrolls.borrow_mut().push((window, x, y));
    }
}

fn mixin_setup(tag: &str, frame: f32) -> (std::sync::MutexGuard<'static, ()>, std::path::PathBuf) {
    let guard = lock_config();
    reset_vars();
    crate::smooth_sc::set_touchpad_scrolled_y(false);
    let dir = use_config_dir(tag);
    crate::seams::set_client_env(Rc::new(FrameEnv(frame)) as Rc<dyn crate::seams::ClientEnv>);
    (guard, dir)
}

fn set_touchpad_vars(
    touch: f32,
    time: f32,
    inertia_threshold: f32,
    inertia_stop: f32,
    deceleration: f32,
    expect_merged: bool,
) {
    use std::sync::atomic::Ordering;
    vars::TOUCHPAD_THRESHOLD.store(touch.to_bits(), Ordering::Relaxed);
    vars::TOUCHPAD_TIME_THRESHOLD.store(time.to_bits(), Ordering::Relaxed);
    vars::INERTIA_THRESHOLD.store(inertia_threshold.to_bits(), Ordering::Relaxed);
    vars::INERTIA_STOP.store(inertia_stop.to_bits(), Ordering::Relaxed);
    vars::INERTIA_DECELERATION.store(deceleration.to_bits(), Ordering::Relaxed);
    vars::EXPECT_MERGED.store(expect_merged, Ordering::Relaxed);
}

fn wheel_event(y: f32) -> SdlEvent {
    SdlEvent { event_type: 32768, wheel_which: 0, wheel_y: y }
}

#[test]
fn sdl_low_wheel_counts_touchpad_without_inertia() {
    let (_guard, _dir) = mixin_setup("ss-m-low", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(1_000_000));
    let mut m = SdlEventHandlerMixin::default();
    m.handle_mouse_wheel_event(&wheel_event(0.2));
    assert!(crate::smooth_sc::touchpad_scrolled_y());
    assert_eq!(m.inertia, 0.0);
    assert_eq!(m.last_scroll_y, 0.2f32 as f64);
    assert_eq!(m.last_scroll_y_time, 1_000_000);
    crate::seams::set_util_nanos(None);
}

#[test]
fn sdl_strong_wheel_cold_is_not_touchpad() {
    let (_guard, _dir) = mixin_setup("ss-m-cold", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(2_000_000_000_000));
    let mut m = SdlEventHandlerMixin::default();
    m.handle_mouse_wheel_event(&wheel_event(3.0));
    assert!(!crate::smooth_sc::touchpad_scrolled_y());
    assert_eq!(m.inertia, 0.0);
    assert_eq!(m.last_scroll_y, 3.0);
    crate::seams::set_util_nanos(None);
}

#[test]
fn sdl_touchpad_strong_wheel_sets_inertia() {
    let (_guard, _dir) = mixin_setup("ss-m-inertia", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(1_000_000));
    let mut m = SdlEventHandlerMixin::default();
    m.handle_mouse_wheel_event(&wheel_event(0.2));
    crate::seams::set_util_nanos(Some(2_000_000));
    m.handle_mouse_wheel_event(&wheel_event(3.0));
    assert!(crate::smooth_sc::touchpad_scrolled_y());
    assert_eq!(m.inertia, 3.0);
    crate::seams::set_util_nanos(None);
}

#[test]
fn sdl_time_threshold_differing_amount_is_touchpad() {
    let (_guard, _dir) = mixin_setup("ss-m-differ", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(2_000_000_000_000));
    let mut m = SdlEventHandlerMixin::default();
    m.handle_mouse_wheel_event(&wheel_event(1.0));
    assert!(!crate::smooth_sc::touchpad_scrolled_y());
    crate::seams::set_util_nanos(Some(2_000_000_000_000 + 1_000_000));
    m.handle_mouse_wheel_event(&wheel_event(-2.0));
    assert!(crate::smooth_sc::touchpad_scrolled_y());
    assert_eq!(m.inertia, -2.0);
    crate::seams::set_util_nanos(None);
}

#[test]
fn sdl_time_threshold_same_amount_is_not_touchpad() {
    let (_guard, _dir) = mixin_setup("ss-m-same", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(2_000_000_000_000));
    let mut m = SdlEventHandlerMixin::default();
    m.handle_mouse_wheel_event(&wheel_event(1.0));
    crate::seams::set_util_nanos(Some(2_000_000_000_000 + 1_000_000));
    m.handle_mouse_wheel_event(&wheel_event(-1.0));
    assert!(!crate::smooth_sc::touchpad_scrolled_y());
    crate::seams::set_util_nanos(None);
}

#[test]
fn sdl_time_threshold_same_amount_touchpad_after_touchpad() {
    let (_guard, _dir) = mixin_setup("ss-m-same2", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(1_000_000));
    let mut m = SdlEventHandlerMixin::default();
    m.handle_mouse_wheel_event(&wheel_event(0.2));
    assert!(crate::smooth_sc::touchpad_scrolled_y());
    crate::seams::set_util_nanos(Some(2_000_000));
    m.handle_mouse_wheel_event(&wheel_event(-0.2));
    assert!(crate::smooth_sc::touchpad_scrolled_y());
    crate::seams::set_util_nanos(None);
}

fn poll_tail_host() -> (Rc<MouseRecorder>, Rc<std::cell::RefCell<Vec<(u64, f64, f64)>>>) {
    let scrolls = Rc::new(std::cell::RefCell::new(Vec::new()));
    let host = Rc::new(MouseRecorder {
        window: 7,
        scrolls: scrolls.clone(),
        tasks: std::cell::RefCell::new(Vec::new()),
    });
    crate::seams::set_mouse_host(host.clone() as Rc<dyn MouseHost>);
    (host, scrolls)
}

#[test]
fn poll_tail_skips_when_time_delta_zero() {
    let (_guard, _dir) = mixin_setup("ss-m-p0", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(5_000_000));
    let (host, scrolls) = poll_tail_host();
    let m = Rc::new(std::cell::RefCell::new(SdlEventHandlerMixin::default()));
    m.borrow_mut().last_scroll_y_time = 5_000_000;
    m.borrow_mut().inertia = 5.0;
    SdlEventHandlerMixin::poll_events_tail(&m, &mut || {});
    host.run_all();
    assert!(scrolls.borrow().is_empty());
    assert_eq!(m.borrow().inertia, 5.0);
    crate::seams::set_util_nanos(None);
}

#[test]
fn poll_tail_skips_below_inertia_stop() {
    let (_guard, _dir) = mixin_setup("ss-m-p1", 0.0);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 100.0, 0.9, false);
    crate::seams::set_util_nanos(Some(5_000_000));
    let (host, scrolls) = poll_tail_host();
    let m = Rc::new(std::cell::RefCell::new(SdlEventHandlerMixin::default()));
    m.borrow_mut().last_scroll_y_time = 4_000_000;
    m.borrow_mut().inertia = 0.5;
    SdlEventHandlerMixin::poll_events_tail(&m, &mut || {});
    host.run_all();
    assert!(scrolls.borrow().is_empty());
    assert_eq!(m.borrow().inertia, 0.5);
    crate::seams::set_util_nanos(None);
}

#[test]
fn poll_tail_decelerates_and_scrolls() {
    let (_guard, _dir) = mixin_setup("ss-m-p2", 0.5);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 1.0, 0.9, false);
    crate::seams::set_util_nanos(Some(5_000_000));
    let (host, scrolls) = poll_tail_host();
    let m = Rc::new(std::cell::RefCell::new(SdlEventHandlerMixin::default()));
    m.borrow_mut().last_scroll_y_time = 4_000_000;
    m.borrow_mut().inertia = 2.0;
    SdlEventHandlerMixin::poll_events_tail(&m, &mut || {});
    let expected = 2.0 * (0.9f32 as f64).powf(0.5);
    assert!((m.borrow().inertia - expected).abs() < 1e-9);
    assert!(scrolls.borrow().is_empty());
    host.run_all();
    let got = scrolls.borrow().clone();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, 7);
    assert_eq!(got[0].1, 0.0);
    assert!((got[0].2 - expected * 0.5).abs() < 1e-9);
    crate::seams::set_util_nanos(None);
}

#[test]
fn poll_tail_lambda_reads_inertia_at_run_time() {
    let (_guard, _dir) = mixin_setup("ss-m-p3", 0.5);
    set_touchpad_vars(0.5, 1_000_000_000.0, 1.0, 1.0, 0.9, false);
    crate::seams::set_util_nanos(Some(5_000_000));
    let (host, scrolls) = poll_tail_host();
    let m = Rc::new(std::cell::RefCell::new(SdlEventHandlerMixin::default()));
    m.borrow_mut().last_scroll_y_time = 4_000_000;
    m.borrow_mut().inertia = 2.0;
    SdlEventHandlerMixin::poll_events_tail(&m, &mut || {});
    // the upstream lambda captures `this`, so it reads inertia when executed
    m.borrow_mut().inertia = 9.0;
    host.run_all();
    let got = scrolls.borrow().clone();
    assert_eq!(got.len(), 1);
    assert!((got[0].2 - 9.0 * 0.5).abs() < 1e-9);
    crate::seams::set_util_nanos(None);
}

#[test]
fn transformation_with_scissor_wrap_branches() {
    let rect = ScreenRectangle::new(0, 0, 10, 10);
    let m = TransformationMixin { scissor: Some(rect), pose: [1.0; 6], opacity: 0.5 };
    // a new scissor passes through to the host's original withScissor
    let other = ScreenRectangle::new(1, 1, 5, 5);
    let mut seen = None;
    let out = m.with_scissor_wrap(Some(other), &mut |s| {
        seen = s;
        Parameters::new([9.0; 6], 9.0, s)
    });
    assert_eq!(seen, Some(other));
    assert_eq!(out.pose, [9.0; 6]);
    assert_eq!(out.opacity, 9.0);
    // null scissor with a shadowed scissor: fresh Parameters with null scissor,
    // the operation is not called
    let mut called = false;
    let out2 = m.with_scissor_wrap(None, &mut |s| {
        called = true;
        Parameters::new([0.0; 6], 0.0, s)
    });
    assert!(!called);
    assert_eq!(out2.scissor, None);
    assert_eq!(out2.pose, [1.0; 6]);
    assert_eq!(out2.opacity, 0.5);
    // null scissor with a null shadow: returns this unchanged, operation not called
    let m2 = TransformationMixin { scissor: None, ..m };
    let mut called2 = false;
    let out3 = m2.with_scissor_wrap(None, &mut |s| {
        called2 = true;
        Parameters::new([0.0; 6], 0.0, s)
    });
    assert!(!called2);
    assert_eq!(out3, Parameters::new(m2.pose, m2.opacity, None));
}

#[test]
fn render_pass_clamps_scissor_to_render_area() {
    let m = RenderPassMixin { render_area: RenderArea { x: 10, y: 20, width: 100, height: 50 } };
    let mut got = None;
    m.ignore_argument_exception(20, 30, 10, 10, &mut |x, y, w, h| got = Some((x, y, w, h)));
    assert_eq!(got, Some((20, 30, 10, 10)));
    let mut got2 = None;
    m.ignore_argument_exception(0, 0, 500, 500, &mut |x, y, w, h| got2 = Some((x, y, w, h)));
    assert_eq!(got2, Some((10, 20, 100, 50)));
    let mut got3 = None;
    m.ignore_argument_exception(500, 500, 10, 10, &mut |x, y, w, h| got3 = Some((x, y, w, h)));
    assert_eq!(got3, Some((10, 20, 1, 1)));
}

#[test]
fn minecraft_client_res_reload_reads_config() {
    let (_guard, dir) = mixin_setup("ss-m-resre", 0.0);
    let file = dir.join("smoothscroll.json");
    crate::seams::set_config_dir(dir.clone());
    let cfg = SmScCfg::new();
    crate::smooth_sc::CFG.with(|c| *c.borrow_mut() = Some(cfg.clone()));
    let _ = std::fs::remove_file(&file);
    assert!(!file.exists());
    crate::mixins::miscellaneous::minecraft_client_mixin::on_res_reload();
    assert!(file.exists());
}

// ---------------------------------------------------------------------------
// mixin/client/Hotbar: HudMixin + MouseHandlerMixin
// ---------------------------------------------------------------------------

use crate::duck::HudDuck;
use crate::mixins::hud::HudMixin;
use crate::seams::{ExtractorSeam, PlayerSeam};

type Log = Rc<std::cell::RefCell<Vec<String>>>;

struct RecExtractor {
    width: i32,
    height: i32,
    log: Log,
    translates: Rc<std::cell::RefCell<Vec<(f32, f32)>>>,
    scissors: Rc<std::cell::RefCell<Vec<(i32, i32, i32, i32)>>>,
    blits: Rc<std::cell::RefCell<Vec<(i32, i32, i32, i32)>>>,
}

impl RecExtractor {
    fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            log: Rc::new(std::cell::RefCell::new(Vec::new())),
            translates: Rc::new(std::cell::RefCell::new(Vec::new())),
            scissors: Rc::new(std::cell::RefCell::new(Vec::new())),
            blits: Rc::new(std::cell::RefCell::new(Vec::new())),
        }
    }
}

impl ExtractorSeam for RecExtractor {
    fn blit_sprite(&self, _p: &str, _l: &str, _x: i32, _y: i32, _w: i32, _h: i32) {
        self.log.borrow_mut().push("blit".into());
    }
    fn extract_slot(&self, _x: i32, _y: i32, _s: i32) {
        self.log.borrow_mut().push("slot".into());
    }
    fn pose_push(&self) {
        self.log.borrow_mut().push("push".into());
    }
    fn pose_pop(&self) {
        self.log.borrow_mut().push("pop".into());
    }
    fn pose_translate(&self, dx: f32, dy: f32) {
        self.translates.borrow_mut().push((dx, dy));
    }
    fn enable_scissor(&self, x1: i32, y1: i32, x2: i32, y2: i32) {
        self.scissors.borrow_mut().push((x1, y1, x2, y2));
        self.log.borrow_mut().push("scissor".into());
    }
    fn disable_scissor(&self) {
        self.log.borrow_mut().push("unscissor".into());
    }
    fn fill(&self, _x1: i32, _y1: i32, _x2: i32, _y2: i32, _c: u32) {
        self.log.borrow_mut().push("fill".into());
    }
    fn blit_full(&self, _pipeline: &str, _texture: &str, x: i32, y: i32, _u: f32, _v: f32, width: i32, height: i32, _tw: i32, _th: i32) {
        self.blits.borrow_mut().push((x, y, width, height));
        self.log.borrow_mut().push("blit_full".into());
    }
    fn item(&self, _item: &str, _x: i32, _y: i32) {
        self.log.borrow_mut().push("item".into());
    }
    fn gui_width(&self) -> i32 {
        self.width
    }
    fn gui_height(&self) -> i32 {
        self.height
    }
}

struct RecPlayer {
    present: bool,
    slot: std::cell::Cell<i32>,
}

impl RecPlayer {
    fn new(present: bool, slot: i32) -> Self {
        Self { present, slot: std::cell::Cell::new(slot) }
    }
    fn swap(&self, to: i32) {
        self.slot.set(to);
    }
}

impl PlayerSeam for RecPlayer {
    fn player_present(&self) -> bool {
        self.present
    }
    fn selected_slot(&self) -> i32 {
        self.slot.get()
    }
}

struct RecDuck(Rc<std::cell::RefCell<Vec<&'static str>>>);

impl HudDuck for RecDuck {
    fn increase_rollover(&mut self) {
        self.0.borrow_mut().push("inc");
    }
    fn decrease_rollover(&mut self) {
        self.0.borrow_mut().push("dec");
    }
}

fn duck_recorder() -> (Rc<std::cell::RefCell<Vec<&'static str>>>, Rc<std::cell::RefCell<dyn HudDuck>>) {
    let log: Rc<std::cell::RefCell<Vec<&'static str>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let duck: Rc<std::cell::RefCell<dyn HudDuck>> = Rc::new(std::cell::RefCell::new(RecDuck(log.clone())));
    (log, duck)
}
fn set_hotbar_vars(smoothness: f32, rollover: bool, static_selector: bool, mask_debug: bool) {
    use std::sync::atomic::Ordering;
    vars::HOTBAR_SMOOTHNESS.store(smoothness.to_bits(), Ordering::Relaxed);
    vars::HOTBAR_ROLLOVER.store(rollover, Ordering::Relaxed);
    vars::STATIC_SELECTOR.store(static_selector, Ordering::Relaxed);
    vars::ENABLE_MASK_DEBUG.store(mask_debug, Ordering::Relaxed);
}

fn hud_setup(tag: &str) -> std::sync::MutexGuard<'static, ()> {
    let guard = lock_config();
    reset_vars();
    set_client_env_for_tests(tag);
    guard
}

fn set_client_env_for_tests(tag: &str) {
    let _ = tag;
    crate::seams::set_client_env(Rc::new(FrameEnv(1.0)) as Rc<dyn crate::seams::ClientEnv>);
}

#[test]
fn hud_extract_wrap_smooths_selector_toward_target() {
    let _guard = hud_setup("hud-smooth");
    let player = Rc::new(RecPlayer::new(true, 3));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let mut m = HudMixin::default();
    let mut calls = 0;
    m.extract_hotbar_wrap(&mut || calls += 1);

    // target = 3*20 = 60; smooth = (0 - 60) * 0.5^1 + 60 = 30
    let (smooth, rollover, _masked) = m.state();
    assert_eq!(smooth, 30.0);
    assert_eq!(rollover, 0);
    assert_eq!(calls, 1);
}

#[test]
fn hud_extract_wrap_rollover_wraps_negative() {
    let _guard = hud_setup("hud-wrap-neg");
    let player = Rc::new(RecPlayer::new(true, 3));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    crate::seams::set_extractor(Rc::new(RecExtractor::new(320, 240)) as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.0, true, false, false);

    let mut m = HudMixin::default();
    m.smooth_selector_pos = -1.0;
    m.rollover = 1;
    m.extract_hotbar_wrap(&mut || {});

    // smoothness 0 -> pow(0,1)=0, smooth = target = (3-9)*20 - 4 = -124;
    // then wrap: += 9*20+4 = 184 -> 60, rollover back to 0
    let (smooth, rollover, _masked) = m.state();
    assert_eq!(smooth, 60.0);
    assert_eq!(rollover, 0);
}

#[test]
fn hud_extract_wrap_rollover_wraps_positive() {
    let _guard = hud_setup("hud-wrap-pos");
    let player = Rc::new(RecPlayer::new(true, 3));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    crate::seams::set_extractor(Rc::new(RecExtractor::new(320, 240)) as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(1.0, true, false, false);

    let mut m = HudMixin::default();
    m.smooth_selector_pos = 200.0;
    m.rollover = 0;
    m.extract_hotbar_wrap(&mut || {});

    // smoothness 1 -> smooth stays 200 > 4*2+180 = 188; wrap: -= 184 -> 16
    let (smooth, rollover, _masked) = m.state();
    assert_eq!(smooth, 16.0);
    assert_eq!(rollover, 1);
}

#[test]
fn hud_move_selector_static_translates_centered_once() {
    let _guard = hud_setup("hud-sel-static");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.5, true, true, false);

    let mut m = HudMixin::default();
    let ex2 = ex.clone();
    m.move_selector(91, &mut || ex2.blit_sprite("p", "l", 91, 0, 182, 9));

    // static selector: translate(-(x - hotbarStart) + 4*20, 0) = (80, 0),
    // one blit inside push/pop, then return (no mask)
    assert_eq!(*ex.log.borrow(), ["push", "blit", "pop"]);
    assert_eq!(*ex.translates.borrow(), [(80.0, 0.0)]);
    assert!(ex.scissors.borrow().is_empty());
}

#[test]
fn hud_move_selector_nonstatic_masks_when_past_edge() {
    let _guard = hud_setup("hud-sel-mask");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let mut m = HudMixin::default();
    m.smooth_selector_pos = 161.0; // round 161 > 8*20 = 160
    let ex2 = ex.clone();
    m.move_selector(91, &mut || ex2.blit_sprite("p", "l", 91, 0, 182, 9));

    // mask scissor (68,217,252,241), first blit at 161-0-184 = -23, second at 161
    assert_eq!(
        *ex.scissors.borrow(),
        [(68, 217, 252, 241)]
    );
    assert_eq!(*ex.log.borrow(), ["scissor", "push", "blit", "pop", "push", "blit", "pop", "unscissor"]);
    assert_eq!(*ex.translates.borrow(), [(-23.0, 0.0), (161.0, 0.0)]);
    let (_s, _r, masked) = m.state();
    assert!(!masked);
}

#[test]
fn hud_move_selector_nonstatic_no_mask_within_edge() {
    let _guard = hud_setup("hud-sel-nomask");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let mut m = HudMixin::default();
    m.smooth_selector_pos = 100.0; // round 100 <= 160
    let ex2 = ex.clone();
    m.move_selector(91, &mut || ex2.blit_sprite("p", "l", 91, 0, 182, 9));

    assert_eq!(*ex.log.borrow(), ["push", "blit", "pop"]);
    assert_eq!(*ex.translates.borrow(), [(100.0, 0.0)]);
}

#[test]
fn hud_mask_debug_draws_fill_before_unscissor() {
    let _guard = hud_setup("hud-mask-debug");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.5, true, false, true);

    let mut m = HudMixin::default();
    m.smooth_selector_pos = 161.0;
    m.move_selector(91, &mut || {});

    let log = ex.log.borrow().clone();
    let fill_pos = log.iter().position(|e| e == "fill").unwrap();
    let unscissor_pos = log.iter().position(|e| e == "unscissor").unwrap();
    assert!(fill_pos < unscissor_pos);
}

#[test]
fn hud_move_whole_hotbar_static_draws_twice_with_wrap_translate() {
    let _guard = hud_setup("hud-hotbar-static");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.5, true, true, false);

    let mut m = HudMixin::default();
    m.smooth_selector_pos = 90.0; // > 4*20 = 80 -> positive wrap branch
    let ex2 = ex.clone();
    m.move_hotbar(&mut || ex2.blit_sprite("p", "l", 91, 0, 182, 9));

    // mask (static rect), push, translate(-90+80 = -10), draw, translate(+184), draw, pop
    assert_eq!(*ex.scissors.borrow(), [(69, 217, 251, 241)]);
    assert_eq!(*ex.translates.borrow(), [(-10.0, 0.0), (184.0, 0.0)]);
    let blits = ex.log.borrow().iter().filter(|e| *e == "blit").count();
    assert_eq!(blits, 2);
    assert_eq!(*ex.log.borrow(), ["scissor", "push", "blit", "blit", "pop", "unscissor"]);
}

#[test]
fn hud_move_whole_hotbar_nonstatic_draws_once_no_mask() {
    let _guard = hud_setup("hud-hotbar-dyn");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let mut m = HudMixin::default();
    m.move_items(20, 0, 1, &mut || {});

    assert_eq!(*ex.log.borrow(), ["slot"]);
    let (_s, _r, masked) = m.state();
    assert!(!masked);
}

#[test]
fn hud_duck_rollover_mutators() {
    let mut m = HudMixin::default();
    m.increase_rollover();
    assert_eq!(m.rollover, 1);
    m.decrease_rollover();
    m.decrease_rollover();
    assert_eq!(m.rollover, -1);
}

#[test]
fn mouse_handler_rollover_increases_on_left_wrap() {
    let _guard = hud_setup("mh-inc");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let (log, duck) = duck_recorder();
    crate::seams::set_hud_duck(duck);

    // scrolling left (yoffset > 0), 0 wraps to 8
    crate::mixins::mouse_handler::on_scroll_wrap(1.0, &mut || player.swap(8));
    assert_eq!(*log.borrow(), ["inc"]);
}

#[test]
fn mouse_handler_rollover_decreases_on_right_wrap() {
    let _guard = hud_setup("mh-dec");
    let player = Rc::new(RecPlayer::new(true, 8));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let (log, duck) = duck_recorder();
    crate::seams::set_hud_duck(duck);

    // scrolling right (yoffset < 0), 8 wraps to 0
    crate::mixins::mouse_handler::on_scroll_wrap(-1.0, &mut || player.swap(0));
    assert_eq!(*log.borrow(), ["dec"]);
}

#[test]
fn mouse_handler_player_absent_calls_operation_only() {
    let _guard = hud_setup("mh-null");
    let player = Rc::new(RecPlayer::new(false, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let (log, duck) = duck_recorder();
    crate::seams::set_hud_duck(duck);

    let mut ran = false;
    crate::mixins::mouse_handler::on_scroll_wrap(1.0, &mut || ran = true);
    assert!(ran);
    assert!(log.borrow().is_empty());
}

#[test]
fn mouse_handler_gated_when_hotbar_smoothness_zero() {
    let _guard = hud_setup("mh-gate");
    let player = Rc::new(RecPlayer::new(true, 0));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    set_hotbar_vars(0.0, true, false, false);

    let (log, duck) = duck_recorder();
    crate::seams::set_hud_duck(duck);

    crate::mixins::mouse_handler::on_scroll_wrap(1.0, &mut || player.swap(8));
    assert!(log.borrow().is_empty());
}

#[test]
fn mouse_handler_unchanged_slot_no_rollover() {
    let _guard = hud_setup("mh-same");
    let player = Rc::new(RecPlayer::new(true, 3));
    crate::seams::set_player_inventory(player.clone() as Rc<dyn PlayerSeam>);
    set_hotbar_vars(0.5, true, false, false);

    let (log, duck) = duck_recorder();
    crate::seams::set_hud_duck(duck);

    crate::mixins::mouse_handler::on_scroll_wrap(1.0, &mut || {});
    assert!(log.borrow().is_empty());
}

// ---------------------------------------------------------------------------
// mixin/client/Widgets: AbstractScrollAreaMixin + EditBoxMixin
// ---------------------------------------------------------------------------

use crate::mixins::abstract_scroll_area::{AbstractScrollAreaMixin, ScrollAreaHost};
use crate::mixins::edit_box::{EditBoxHost, EditBoxMixin};

fn set_widget_vars(entry_smoothness: f32, entry_amount: f64, text_smoothness: f32, text_amount: f32) {
    use std::sync::atomic::Ordering;
    vars::ENTRY_LIST_SMOOTHNESS.store(entry_smoothness.to_bits(), Ordering::Relaxed);
    vars::ENTRY_LIST_AMOUNT.store(entry_amount.to_bits(), Ordering::Relaxed);
    vars::TEXT_SMOOTHNESS.store(text_smoothness.to_bits(), Ordering::Relaxed);
    vars::TEXT_AMOUNT.store(text_amount.to_bits(), Ordering::Relaxed);
}

struct RecScrollArea {
    amount: std::cell::Cell<f64>,
    mixin: Rc<AbstractScrollAreaMixin>,
}

impl ScrollAreaHost for RecScrollArea {
    fn scroll_amount(&self) -> f64 {
        self.amount.get()
    }
    fn set_scroll_amount(&self, v: f64) {
        self.amount.set(v);
        self.mixin.set_scroll_amount_tail(self);
    }
}

#[test]
fn scroll_area_set_scroll_tail_syncs_target() {
    let _guard = hud_setup("sa-tail");
    let mixin = Rc::new(AbstractScrollAreaMixin::default());
    let host = RecScrollArea { amount: std::cell::Cell::new(0.0), mixin: mixin.clone() };

    host.set_scroll_amount(42.0);

    assert_eq!(mixin.target_scroll_pos.get(), 42.0);
    assert_eq!(mixin.smooth_scroll_pos.get(), 42.0);
}

#[test]
fn scroll_area_update_scroll_smooths_and_rounds() {
    let _guard = hud_setup("sa-update");
    let mixin = Rc::new(AbstractScrollAreaMixin::default());
    let host = RecScrollArea { amount: std::cell::Cell::new(0.0), mixin: mixin.clone() };
    set_widget_vars(0.5, 3.0, 0.5, 4.0);

    mixin.smooth_scroll_pos.set(90.0);
    mixin.target_scroll_pos.set(100.0);
    mixin.update_scroll(&host);

    // smooth = (90-100)*0.5^1 + 100 = 95; scrollAmount = round(95) = 95;
    // the workaround setScrollAmount runs with the tail guard up
    assert_eq!(host.amount.get(), 95.0);
    assert_eq!(mixin.smooth_scroll_pos.get(), 95.0);
    assert_eq!(mixin.target_scroll_pos.get(), 100.0);
    assert!(!mixin.no_set_scroll_t.get());
}

#[test]
fn scroll_area_update_scroll_skips_when_smoothness_zero() {
    let _guard = hud_setup("sa-zero");
    let mixin = Rc::new(AbstractScrollAreaMixin::default());
    let host = RecScrollArea { amount: std::cell::Cell::new(30.0), mixin: mixin.clone() };
    set_widget_vars(0.0, 3.0, 0.5, 4.0);

    mixin.smooth_scroll_pos.set(90.0);
    mixin.target_scroll_pos.set(100.0);
    mixin.update_scroll(&host);

    assert_eq!(host.amount.get(), 30.0);
    assert_eq!(mixin.smooth_scroll_pos.get(), 90.0);
}

#[test]
fn scroll_area_mouse_scrolled_wrap_moves_target_by_amount() {
    let _guard = hud_setup("sa-wrap");
    let mixin = Rc::new(AbstractScrollAreaMixin::default());
    let host = RecScrollArea { amount: std::cell::Cell::new(50.0), mixin: mixin.clone() };
    set_widget_vars(0.0, 3.0, 0.5, 4.0);

    mixin.target_scroll_pos.set(50.0);
    let ret = mixin.mouse_scrolled_wrap(&host, 0.0, 0.0, 0.0, 1.0, &mut || true);

    assert!(ret);
    assert_eq!(mixin.target_scroll_pos.get(), 47.0); // 50 - 3*1
    assert_eq!(host.amount.get(), 47.0);
    assert!(!mixin.no_set_scroll_t.get());
}

#[test]
fn scroll_area_mouse_scrolled_wrap_restores_prev_when_smooth() {
    let _guard = hud_setup("sa-restore");
    let mixin = Rc::new(AbstractScrollAreaMixin::default());
    let host = RecScrollArea { amount: std::cell::Cell::new(50.0), mixin: mixin.clone() };
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    crate::smooth_sc::set_touchpad_scrolled_y(false);

    mixin.target_scroll_pos.set(50.0);
    let ret = mixin.mouse_scrolled_wrap(&host, 0.0, 0.0, 0.0, 1.0, &mut || true);

    // smoothness != 0: the amount is restored to prevScrollPos after the move
    assert!(ret);
    assert_eq!(host.amount.get(), 50.0);
    assert_eq!(mixin.target_scroll_pos.get(), 47.0);
    assert!(!mixin.no_set_scroll_t.get());
}

#[test]
fn scroll_area_touchpad_snap_sets_smooth_to_target() {
    let _guard = hud_setup("sa-touch");
    let mixin = Rc::new(AbstractScrollAreaMixin::default());
    let host = RecScrollArea { amount: std::cell::Cell::new(50.0), mixin: mixin.clone() };
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    crate::smooth_sc::set_touchpad_scrolled_y(true);

    mixin.target_scroll_pos.set(50.0);
    mixin.mouse_scrolled_wrap(&host, 0.0, 0.0, 0.0, 1.0, &mut || true);

    assert_eq!(mixin.smooth_scroll_pos.get(), 47.0);
    crate::smooth_sc::set_touchpad_scrolled_y(false);
}

#[test]
fn scroll_area_wrap_no_target_move_when_entry_list_amount_zero() {
    let _guard = hud_setup("sa-noamount");
    let mixin = Rc::new(AbstractScrollAreaMixin::default());
    let host = RecScrollArea { amount: std::cell::Cell::new(50.0), mixin: mixin.clone() };
    set_widget_vars(0.0, 0.0, 0.5, 4.0);

    mixin.target_scroll_pos.set(50.0);
    let ret = mixin.mouse_scrolled_wrap(&host, 0.0, 0.0, 0.0, 1.0, &mut || true);

    assert!(ret);
    assert_eq!(mixin.target_scroll_pos.get(), 50.0);
    assert_eq!(host.amount.get(), 50.0);
}

struct RecEditBox {
    hovered: bool,
    value: String,
    display_pos: std::cell::Cell<i32>,
}

impl EditBoxHost for RecEditBox {
    fn is_hovered(&self) -> bool {
        self.hovered
    }
    fn value(&self) -> String {
        self.value.clone()
    }
    fn font_width(&self, s: &str) -> i32 {
        s.chars().count() as i32 * 6
    }
    fn plain_substr(&self, s: &str, width: i32) -> String {
        s.chars().take((width.max(0) as usize) / 6).collect()
    }
    fn display_pos(&self) -> i32 {
        self.display_pos.get()
    }
    fn set_display_pos(&self, v: i32) {
        self.display_pos.set(v);
    }
    fn text_x(&self) -> i32 {
        5
    }
    fn text_y(&self) -> i32 {
        30
    }
    fn inner_width(&self) -> i32 {
        50
    }
}

#[test]
fn edit_box_mouse_scrolled_requires_hover() {
    let _guard = hud_setup("eb-hover");
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    let host = RecEditBox {
        hovered: false,
        value: "hello world".into(),
        display_pos: std::cell::Cell::new(0),
    };
    let mut m = EditBoxMixin::default();

    assert!(!m.mouse_scrolled(&host, 0.0, -2.0));
    assert_eq!(m.target_scroll_pos, 0.0);
}

#[test]
fn edit_box_mouse_scrolled_moves_target_clamped() {
    let _guard = hud_setup("eb-scroll");
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    crate::smooth_sc::set_touchpad_scrolled_y(false);
    let host = RecEditBox {
        hovered: true,
        value: "hello world".into(), // 66px wide
        display_pos: std::cell::Cell::new(0),
    };
    let mut m = EditBoxMixin::default();

    // target = 0 - (-2 - 0) * 4 = 8, clamped to [0, 66]
    assert!(m.mouse_scrolled(&host, 0.0, -2.0));
    assert_eq!(m.target_scroll_pos, 8.0);
}

#[test]
fn edit_box_mouse_scrolled_touchpad_snaps_smooth() {
    let _guard = hud_setup("eb-touch");
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    crate::smooth_sc::set_touchpad_scrolled_y(true);
    let host = RecEditBox {
        hovered: true,
        value: "hello world".into(),
        display_pos: std::cell::Cell::new(0),
    };
    let mut m = EditBoxMixin::default();

    m.mouse_scrolled(&host, 0.0, -2.0);
    assert_eq!(m.smooth_scroll_pos, m.target_scroll_pos);
    crate::smooth_sc::set_touchpad_scrolled_y(false);
}

#[test]
fn edit_box_render_wrap_smooths_sets_display_pos_and_scissor() {
    let _guard = hud_setup("eb-render");
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    let host = RecEditBox {
        hovered: true,
        value: "hello world".into(),
        display_pos: std::cell::Cell::new(0),
    };
    let mut m = EditBoxMixin::default();
    m.smooth_scroll_pos = 10.0;
    m.target_scroll_pos = 20.0;

    let ex2 = ex.clone();
    m.render_widget_wrap(&host, &mut || ex2.blit_sprite("p", "l", 0, 0, 0, 0));

    // smooth = (10-20)*0.5^1 + 20 = 15; shown = 2 chars ("he"); displayPos = 2;
    // drawOffset = width("he") - 15 = -3; scissor (5, 20, 55, 40)
    assert_eq!(host.display_pos.get(), 2);
    assert_eq!(m.smooth_scroll_pos, 15.0);
    assert_eq!(*ex.scissors.borrow(), [(5, 20, 55, 40)]);
    assert_eq!(*ex.translates.borrow(), [(-3.0, 0.0)]);
    assert_eq!(*ex.log.borrow(), ["scissor", "push", "blit", "pop", "unscissor"]);
}

#[test]
fn edit_box_draw_background_restores_scissor_and_translate() {
    let _guard = hud_setup("eb-bg");
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    let host = RecEditBox {
        hovered: true,
        value: "hello world".into(),
        display_pos: std::cell::Cell::new(0),
    };
    let mut m = EditBoxMixin::default();
    m.smooth_scroll_pos = 15.0;

    let ex2 = ex.clone();
    m.draw_background_wrap(&host, &mut || ex2.blit_sprite("p", "l", 0, 0, 0, 0));

    // pop, disableScissor, op, enableScissor(textX, textY-10, ...), push, translate(drawOffset)
    assert_eq!(*ex.log.borrow(), ["pop", "unscissor", "blit", "scissor", "push"]);
    assert_eq!(*ex.translates.borrow(), [(-3.0, 0.0)]);
}

#[test]
fn edit_box_add_characters_appends_overflow_char() {
    let _guard = hud_setup("eb-add");
    let host = RecEditBox {
        hovered: true,
        value: "abcdef".into(),
        display_pos: std::cell::Cell::new(0),
    };
    let m = EditBoxMixin::default();

    // firstCharWidth = 6 -> additional "d" (1 char); totalLen 4 < 6 -> += 'e'
    assert_eq!(m.add_characters(&host, "abc"), "abcde");
}

#[test]
fn edit_box_add_characters_passthrough_at_end_and_empty() {
    let _guard = hud_setup("eb-add2");
    let host = RecEditBox {
        hovered: true,
        value: "abcdef".into(),
        display_pos: std::cell::Cell::new(3),
    };
    let m = EditBoxMixin::default();

    assert_eq!(m.add_characters(&host, "def"), "def");
    assert_eq!(m.add_characters(&host, ""), "");
}

#[test]
fn edit_box_scroll_to_without_custom_update_sets_target_from_display_pos() {
    let _guard = hud_setup("eb-srollto-plain");
    set_widget_vars(0.5, 3.0, 0.5, 4.0); // text_custom_update defaults false
    let host = RecEditBox {
        hovered: true,
        value: "abcdef".into(),
        display_pos: std::cell::Cell::new(2),
    };
    let mut m = EditBoxMixin::default();

    let mut calls = 0;
    m.scroll_to_wrap(&host, 5, &mut |p| {
        assert_eq!(p, 5);
        calls += 1;
    });
    assert_eq!(calls, 1);
    assert_eq!(m.target_scroll_pos, 12.0); // width("ab") = 12
    assert_eq!(host.display_pos.get(), 2); // untouched
}

#[test]
fn edit_box_scroll_to_custom_update_margin_low_moves_target() {
    let _guard = hud_setup("eb-scrollto-cu");
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    vars::TEXT_CUSTOM_UPDATE.store(true, std::sync::atomic::Ordering::Relaxed);
    vars::TEXT_MARGIN.store(30.0f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
    let host = RecEditBox {
        hovered: true,
        value: "abcdefghijkl".into(), // 72px wide
        display_pos: std::cell::Cell::new(5),
    };
    let mut m = EditBoxMixin::default();

    // cursorPixel = width("abcdefghi") = 54 > target(0) + 50 - 15 = 35
    // -> target = 54 - 50 + 15 = 19; clamp(19, 4, 54) then clamp(19, 0, 72)
    m.scroll_to_wrap(&host, 9, &mut |_| {});
    assert_eq!(m.target_scroll_pos, 19.0);
    assert_eq!(m.prev_cursor_pixel, 54.0);
    assert_eq!(host.display_pos.get(), 0);
}

#[test]
fn edit_box_scroll_to_custom_update_margin_high_left_clamps() {
    let _guard = hud_setup("eb-scrollto-high");
    set_widget_vars(0.5, 3.0, 0.5, 4.0);
    vars::TEXT_CUSTOM_UPDATE.store(true, std::sync::atomic::Ordering::Relaxed);
    vars::TEXT_MARGIN.store(60.0f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
    let host = RecEditBox {
        hovered: true,
        value: "abcdefghijkl".into(),
        display_pos: std::cell::Cell::new(5),
    };
    let mut m = EditBoxMixin::default();
    m.prev_cursor_pixel = 60.0;

    // cursorPixel = 24 < prev(60) and < target(0)+30 -> target = 24-30 = -6;
    // inner clamp(-6, -26, 24) keeps -6; final clamp to [0, 72] -> 0
    m.scroll_to_wrap(&host, 4, &mut |_| {});
    assert_eq!(m.target_scroll_pos, 0.0);
    assert_eq!(m.prev_cursor_pixel, 24.0);
    assert_eq!(host.display_pos.get(), 0);
}

// ---------------------------------------------------------------------------
// mixin/client/Chat: ChatScreenMixin + CommandSuggestionsMixin + SuggestionsListMixin
// ---------------------------------------------------------------------------

use crate::mixins::command_suggestions as cmd_sugg;
use crate::mixins::chat_screen;
use crate::mixins::suggestions_list::{SuggestionsListHost, SuggestionsListMixin};

struct RecChatComponent {
    floaty_scroll_y: std::cell::Cell<f64>,
    calls: std::cell::Cell<i32>,
}

impl crate::duck::ChatComponentDuck for RecChatComponent {
    fn floaty_scroll(&mut self, scroll_y: f64) {
        self.floaty_scroll_y.set(scroll_y);
        self.calls.set(self.calls.get() + 1);
    }
}

#[test]
fn chat_screen_unclamp_returns_value() {
    assert_eq!(chat_screen::unclamp_scroll(7.0, 0.0, 3.0), 7.0);
}

#[test]
fn chat_screen_mouse_scrolled_wrap_calls_floaty_scroll() {
    let mut chat = RecChatComponent { floaty_scroll_y: std::cell::Cell::new(0.0), calls: std::cell::Cell::new(0) };

    chat_screen::mouse_scrolled_wrap(&mut chat, 1, 2.5);

    assert_eq!(chat.calls.get(), 1);
    assert_eq!(chat.floaty_scroll_y.get(), 2.5);
}

#[test]
fn command_suggestions_unclamp_returns_value() {
    assert_eq!(cmd_sugg::unclamp_scroll(-4.0, 0.0, 3.0), -4.0);
}

struct RecSuggestions {
    offset: std::cell::Cell<i32>,
    contains: bool,
}

impl SuggestionsListHost for RecSuggestions {
    fn offset(&self) -> i32 {
        self.offset.get()
    }
    fn set_offset(&self, v: i32) {
        self.offset.set(v);
    }
    fn rect_y(&self) -> i32 {
        40
    }
    fn rect_height(&self) -> i32 {
        60
    }
    fn rect_contains(&self, _x: i32, _y: i32) -> bool {
        self.contains
    }
}

fn sl_vars(smoothness: f32, amount: f32) {
    use std::sync::atomic::Ordering;
    vars::SUGGESTION_WINDOW_SMOOTHNESS.store(smoothness.to_bits(), Ordering::Relaxed);
    vars::SUGGESTION_WINDOW_AMOUNT.store(amount.to_bits(), Ordering::Relaxed);
}

#[test]
fn sl_render_head_smooths_and_sets_offset() {
    let _guard = hud_setup("sl-head");
    sl_vars(0.5, 12.0);
    let host = RecSuggestions { offset: std::cell::Cell::new(0), contains: true };
    let m = SuggestionsListMixin::default();
    m.smooth_scroll_pos.set(3.4);
    m.target_scroll_pos.set(3.0);

    m.render_head(&host);

    // smooth = (3.4-3)*0.5^1 + 3 = 3.2; |0.2| >= 1/12/2 so no snap
    assert_eq!(m.smooth_scroll_pos.get(), 3.2);
    assert_eq!(host.offset.get(), 3); // floor(3.2)
}

#[test]
fn sl_render_head_snaps_half_pixel() {
    let _guard = hud_setup("sl-snap");
    sl_vars(0.5, 12.0);
    let host = RecSuggestions { offset: std::cell::Cell::new(0), contains: true };
    let m = SuggestionsListMixin::default();
    m.smooth_scroll_pos.set(3.02);
    m.target_scroll_pos.set(3.0);

    m.render_head(&host);

    // |3.01 - 3| < 1/12/2 = 0.0416... -> snap
    assert_eq!(m.smooth_scroll_pos.get(), 3.0);
    assert_eq!(host.offset.get(), 3);
}

#[test]
fn sl_mouse_scrolled_wrap_moves_target() {
    let _guard = hud_setup("sl-scroll");
    sl_vars(0.5, 12.0);
    crate::smooth_sc::set_touchpad_scrolled_y(false);
    let host = RecSuggestions { offset: std::cell::Cell::new(0), contains: true };
    let m = SuggestionsListMixin::default();
    m.target_scroll_pos.set(2.0);

    let ret = m.mouse_scrolled_wrap(&host, 1.0, &mut |s| {
        assert_eq!(s, 0.0); // the call uses only the clamp and return value
        true
    });

    assert!(ret);
    // newTarget = 2 - 1 * (12/12) = 1; offset = ceil(1) = 1; target = 1
    assert_eq!(m.target_scroll_pos.get(), 1.0);
    assert_eq!(host.offset.get(), 1);
}

#[test]
fn sl_mouse_scrolled_wrap_restores_offset_on_false() {
    let _guard = hud_setup("sl-false");
    sl_vars(0.5, 12.0);
    let host = RecSuggestions { offset: std::cell::Cell::new(3), contains: true };
    let m = SuggestionsListMixin::default();
    m.target_scroll_pos.set(2.0);

    let ret = m.mouse_scrolled_wrap(&host, 1.0, &mut |_| false);

    assert!(!ret);
    assert_eq!(host.offset.get(), 3); // indexBefore restored
    assert_eq!(m.target_scroll_pos.get(), 2.0);
}

#[test]
fn sl_mouse_scrolled_wrap_zero_amount_passthrough() {
    let _guard = hud_setup("sl-zero");
    sl_vars(0.5, 0.0);
    let host = RecSuggestions { offset: std::cell::Cell::new(3), contains: true };
    let m = SuggestionsListMixin::default();
    m.target_scroll_pos.set(2.0);

    let ret = m.mouse_scrolled_wrap(&host, 1.0, &mut |s| s == 1.0);

    assert!(ret);
    assert_eq!(host.offset.get(), 3);
    assert_eq!(m.target_scroll_pos.get(), 2.0);
}

#[test]
fn sl_mouse_scrolled_wrap_clamps_negative_target() {
    let _guard = hud_setup("sl-neg");
    sl_vars(0.5, 12.0);
    let host = RecSuggestions { offset: std::cell::Cell::new(0), contains: true };
    let m = SuggestionsListMixin::default();
    m.target_scroll_pos.set(1.0);

    let ret = m.mouse_scrolled_wrap(&host, 5.0, &mut |_| true);

    // newTarget = 1 - 5 = -4 -> clamped to 0
    assert!(ret);
    assert_eq!(m.target_scroll_pos.get(), 0.0);
}

#[test]
fn sl_cycle_wrap_syncs_target_without_touching_scroll() {
    let _guard = hud_setup("sl-cycle");
    sl_vars(0.5, 12.0);
    let host = RecSuggestions { offset: std::cell::Cell::new(0), contains: true };
    let m = SuggestionsListMixin::default();
    m.target_scroll_pos.set(3.0);

    let host2 = &host;
    m.cycle_wrap(&host, 1, &mut |off| {
        assert_eq!(off, 1);
        host2.offset.set(2); // keyboard selection moves the list
    });

    // offset ran at floor(target) = 3, target re-synced from the op's offset,
    // then the original offset restored
    assert_eq!(m.target_scroll_pos.get(), 2.0);
    assert_eq!(host.offset.get(), 0);
}

#[test]
fn sl_mouse_clicked_wrap_uses_draw_offset_and_outside_rejects() {
    let _guard = hud_setup("sl-click");
    sl_vars(0.5, 12.0);
    let m = SuggestionsListMixin::default();
    m.smooth_scroll_pos.set(3.4);
    m.target_scroll_pos.set(3.0);

    // drawOffset = (3 - 3.4) * 12 = -4.8; Java round -> -5
    let inside = RecSuggestions { offset: std::cell::Cell::new(3), contains: true };
    let mut clicked_y = 0;
    let ret = m.mouse_clicked_wrap(&inside, 10, 50, &mut |x, y| {
        assert_eq!(x, 10);
        clicked_y = y;
        true
    });
    assert!(ret);
    assert_eq!(clicked_y, 55); // 50 - (-5)

    let outside = RecSuggestions { offset: std::cell::Cell::new(3), contains: false };
    assert!(!m.mouse_clicked_wrap(&outside, 10, 50, &mut |_, _| true));
}

#[test]
fn sl_extract_render_state_wrap_translates_and_untranslates() {
    let _guard = hud_setup("sl-render");
    sl_vars(0.5, 12.0);
    let ex = Rc::new(RecExtractor::new(320, 240));
    crate::seams::set_extractor(ex.clone() as Rc<dyn ExtractorSeam>);
    let host = RecSuggestions { offset: std::cell::Cell::new(0), contains: true };
    let m = SuggestionsListMixin::default();
    m.smooth_scroll_pos.set(3.25);
    m.target_scroll_pos.set(3.0);

    let ex2 = ex.clone();
    m.extract_render_state_wrap(ex.as_ref(), &host, 15, 50, &mut |mx, my, hooks| {
        assert_eq!(mx, 15); // rect contains -> mouseX kept
        // my = floor(50 - (-1.5)) = 51
        assert_eq!(my, 51);
        hooks.translate(&host);
        ex2.blit_sprite("p", "l", 0, 0, 0, 0);
        hooks.try_untranslate();
    });

    // render_head: smooth = (3.25-3)*0.5 + 3 = 3.125; offset = floor = 3;
    // drawOffset = (3-3.125)*12 = -1.5 -> Java round = -1... but the translate
    // hook passes the float drawOffset (pose_translate takes floats)
    assert_eq!(host.offset.get(), 3);
    assert_eq!(*ex.log.borrow(), ["scissor", "push", "blit", "pop", "unscissor"]);
    assert_eq!(*ex.scissors.borrow(), [(0, 40, 320, 100)]);
    assert_eq!(*ex.translates.borrow(), [(0.0, -1.0)]);
}

#[test]
fn sl_add_line_under_and_cursor_rect() {
    let _guard = hud_setup("sl-line");
    sl_vars(0.5, 12.0);
    let host = RecSuggestions { offset: std::cell::Cell::new(3), contains: true };
    let m = SuggestionsListMixin::default();
    m.smooth_scroll_pos.set(3.25); // drawOffset = -1.5 (align_pixels)

    assert_eq!(m.add_line_under(&host, 8), 9);
    // drawOffset = (3 - 3.25) * 12 = -3.0; floor(50 + (-3.0)) = 47
    assert_eq!(m.request_cursor_rect_y(&host, 50), 47);

    m.smooth_scroll_pos.set(3.0); // drawOffset = 0
    assert_eq!(m.add_line_under(&host, 8), 8);
}

#[test]
fn sl_draw_offset_java_round_semantics() {
    let _guard = hud_setup("sl-round");
    let host = RecSuggestions { offset: std::cell::Cell::new(3), contains: true };
    let m = SuggestionsListMixin::default();
    m.smooth_scroll_pos.set(3.25);
    m.target_scroll_pos.set(3.0);

    // (3 - 3.25) * 12 = -3.0 exact -> -3.0
    assert_eq!(m.get_draw_offset(&host), -3.0);
    m.align_pixels.set(false);
    assert_eq!(m.get_draw_offset(&host), -3.0);
}

// ---------------------------------------------------------------- chat_component

use crate::mixins::chat_component::{ChatComponentHost, ChatComponentMixin, ChatParams, TransformationAccess};

struct RecChatHost {
    scrollbar_pos: std::cell::Cell<i32>,
    line_height: i32,
    width: i32,
    lines_per_page: i32,
    added: std::cell::RefCell<Vec<i64>>,
    max_scroll: i32,
}

impl RecChatHost {
    fn new(max_scroll: i32) -> Self {
        Self {
            scrollbar_pos: std::cell::Cell::new(0),
            line_height: 12,
            width: 320,
            lines_per_page: 2,
            added: std::cell::RefCell::new(Vec::new()),
            max_scroll,
        }
    }
}

impl ChatComponentHost for RecChatHost {
    fn chat_scrollbar_pos(&self) -> i32 {
        self.scrollbar_pos.get()
    }
    fn set_chat_scrollbar_pos(&self, v: i32) {
        self.scrollbar_pos.set(v);
    }
    fn line_height(&self) -> i32 {
        self.line_height
    }
    fn width(&self) -> i32 {
        self.width
    }
    fn lines_per_page(&self) -> i32 {
        self.lines_per_page
    }
    fn trimmed_len(&self) -> usize {
        self.added.borrow().len()
    }
    fn trimmed_added_time(&self, index: usize) -> i64 {
        self.added.borrow()[index]
    }
    // vanilla scrollChat: moves pos by dir and clamps into [0, maxScroll]
    fn scroll_chat(&self, dir: i32) {
        let p = (self.scrollbar_pos.get() + dir).clamp(0, self.max_scroll);
        self.scrollbar_pos.set(p);
    }
}

struct RecTransAccess {
    params: std::cell::RefCell<Option<ChatParams>>,
    context: Option<Rc<RecExtractor>>,
}

impl TransformationAccess for RecTransAccess {
    fn get_transformation(&self) -> Option<ChatParams> {
        self.params.borrow().clone()
    }
    fn set_transformation(&self, t: Option<ChatParams>) {
        *self.params.borrow_mut() = t;
    }
    fn get_context(&self) -> Option<&dyn ExtractorSeam> {
        self.context.as_ref().map(|c| c.as_ref() as &dyn ExtractorSeam)
    }
}

fn cc_vars(smoothness: f32, opening: f32, amount: f32) {
    use std::sync::atomic::Ordering;
    vars::CHAT_SMOOTHNESS.store(smoothness.to_bits(), Ordering::Relaxed);
    vars::CHAT_OPENING_SMOOTHNESS.store(opening.to_bits(), Ordering::Relaxed);
    vars::CHAT_AMOUNT.store(amount.to_bits(), Ordering::Relaxed);
}

#[test]
fn cc_render_wrap_smooths_scroll_and_mask() {
    let _guard = mixin_setup("cc-smooth", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    host.added.borrow_mut().extend([1000, 999, 500]);
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_scroll_pos = 2.4;
    m.target_scroll_pos = 2.0;
    m.smooth_mask_height = 1.0;

    m.render_wrap(1000, false);

    // smooth = (2.4 - 2) * 0.5 + 2 = 2.2, offset 2.4 >= epsilon, pos = 2
    assert!((m.smooth_scroll_pos - 2.2).abs() < 1e-9);
    assert_eq!(host.chat_scrollbar_pos(), 2);
    // shownLineCount = 2 (times 1000, 999), target 2, smooth = (1-2)*0.5+2 = 1.5
    assert_eq!(m.target_mask_height, 2.0);
    assert!((m.smooth_mask_height - 1.5).abs() < 1e-9);
}

#[test]
fn cc_render_wrap_snaps_small_scroll_offset() {
    let _guard = mixin_setup("cc-snap", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    host.added.borrow_mut().extend([1000, 999]);
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_scroll_pos = 2.005;
    m.target_scroll_pos = 2.0;
    m.smooth_mask_height = 2.0;
    m.target_mask_height = 2.0;

    m.render_wrap(1000, false);

    // |0.005| * 12 = 0.06 < epsilon -> snap to target
    assert_eq!(m.smooth_scroll_pos, 2.0);
    assert_eq!(host.chat_scrollbar_pos(), 2);
}

#[test]
fn cc_render_wrap_mask_snaps_when_close() {
    let _guard = mixin_setup("cc-mask-snap", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    host.added.borrow_mut().extend([1000, 999]);
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_scroll_pos = 2.0;
    m.target_scroll_pos = 2.0;
    m.smooth_mask_height = 2.005;
    m.target_mask_height = 2.0;

    m.render_wrap(1000, false);

    // |0.005| * 12 = 0.06 < epsilon -> mask snaps
    assert_eq!(m.smooth_mask_height, 2.0);
}

#[test]
fn cc_modify_y_pos_seeds_and_translates() {
    let _guard = mixin_setup("cc-yp", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());

    // first call seeds smoothChatBottom = chatBottom / lineHeight = 96/12 = 8.0
    let out = m.modify_y_pos(ex.as_ref(), 96, 1.0);
    assert_eq!(out, 96);
    assert_eq!(m.smooth_chat_bottom, 8.0);
    // dy = 8.0 * 12 * 1.0 - 96 * 1.0 = 0.0
    assert_eq!(*ex.translates.borrow(), vec![(0.0, 0.0)]);

    // chatBottom moved to 120: smooth = (8-10)*0.5+10 = 9.0; dy = 108 - 120 = -12
    let out = m.modify_y_pos(ex.as_ref(), 120, 1.0);
    assert_eq!(out, 120);
    assert_eq!(m.smooth_chat_bottom, 9.0);
    assert_eq!(*ex.translates.borrow(), vec![(0.0, 0.0), (0.0, -12.0)]);
}

#[test]
fn cc_for_visible_line_wrap_scissors_translates_and_unscissors() {
    let _guard = mixin_setup("cc-wrap", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());
    host.set_chat_scrollbar_pos(2); // drawOffset = -(2 - 2) * 12 = 0
    m.smooth_scroll_pos = 2.0;
    m.target_scroll_pos = 2.0;
    m.smooth_mask_height = 2.0;
    m.target_mask_height = 2.0;

    let ta = RecTransAccess { params: std::cell::RefCell::new(Some(ChatParams::default())), context: Some(ex.clone()) };
    let mut op = || 7;
    let ret = m.for_visible_line_wrap(ex.as_ref(), Some(&ta), 100, &mut op);

    // maskTop = round(-2*12) + 100 = 76, snapped -> 44; maskBottom = 100 + 32 = 132
    assert_eq!(ret, 7);
    assert_eq!(*ex.scissors.borrow(), vec![(-10, 44, 1320, 132)]);
    // drawOffset 0 -> translate(0,0) and back
    assert_eq!(*ex.translates.borrow(), vec![(0.0, 0.0), (0.0, 0.0)]);
    // deMask cleared the scissor in the transformation
    assert_eq!(ta.params.borrow().as_ref().unwrap().scissor, None);
    assert_eq!(*ex.log.borrow(), vec!["scissor".to_string(), "unscissor".to_string()]);
}

#[test]
fn cc_for_visible_line_wrap_widens_only_when_snapped() {
    let _guard = mixin_setup("cc-wide", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_scroll_pos = 2.25; // drawOffset = -(2 - 2.25) * 12 = 3.0 != 0 -> no widening
    m.target_scroll_pos = 2.0;
    host.set_chat_scrollbar_pos(2);
    m.smooth_mask_height = 2.0;
    m.target_mask_height = 2.0;

    let ta = RecTransAccess { params: std::cell::RefCell::new(Some(ChatParams::default())), context: Some(ex.clone()) };
    let mut op = || 0;
    m.for_visible_line_wrap(ex.as_ref(), Some(&ta), 100, &mut op);

    assert_eq!(*ex.scissors.borrow(), vec![(-10, 76, 1320, 100)]);
}

#[test]
fn cc_floaty_scroll_scales_and_clamps() {
    let _guard = mixin_setup("cc-float", 1.0);
    cc_vars(0.5, 0.5, 24.0); // amount 24 / lineHeight 12 = factor 2
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());

    let wrapped = |_d: i32| host.scroll_chat(0);
    m.floaty_scroll_impl(7.0, &mut |_d| host.scroll_chat(0));
    // target = 0 + (7/7) * 2 = 2; clamp: pos = 2, within max 5 -> target 2
    assert_eq!(m.target_scroll_pos, 2.0);
    assert_eq!(host.chat_scrollbar_pos(), 2);
    assert_eq!(m.smooth_scroll_pos, 0.0); // touchpad off

    // clamped by the host's scrollChat clamp
    let host2 = Rc::new(RecChatHost::new(1));
    let mut m2 = ChatComponentMixin::new(host2.clone());
    m2.floaty_scroll_impl(7.0, &mut |_d| host2.scroll_chat(0));
    // pos = ceil(2) = 2 -> wrapped(0) clamps to 1 -> clamping 2 > 1 -> 1
    assert_eq!(m2.target_scroll_pos, 1.0);
    assert_eq!(host2.chat_scrollbar_pos(), 1);
}

#[test]
fn cc_floaty_scroll_zero_amount_uses_seven_lines() {
    let _guard = mixin_setup("cc-float0", 1.0);
    cc_vars(0.5, 0.5, 0.0); // amount 0 -> factor 7
    let host = Rc::new(RecChatHost::new(10));
    let mut m = ChatComponentMixin::new(host.clone());

    m.floaty_scroll_impl(7.0, &mut |_d| host.scroll_chat(0));
    // target = 0 + (7/7) * 7 = 7
    assert_eq!(m.target_scroll_pos, 7.0);
    assert_eq!(host.chat_scrollbar_pos(), 7);
}

#[test]
fn cc_on_new_message_shifts_target_and_smooth() {
    let _guard = mixin_setup("cc-newmsg", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_scroll_pos = 1.0;
    m.target_scroll_pos = 1.0;

    let calls = Rc::new(std::cell::Cell::new(0usize));
    m.on_new_message(2, &mut |_d| calls.set(calls.get() + 1));

    // target = 1 + 2 = 3 -> clamp (max 5) -> 3; smooth = 1 + 2 = 3
    // scrollChat(0) runs twice: once inside clampScroll, once at the tail
    assert_eq!(m.target_scroll_pos, 3.0);
    assert_eq!(m.smooth_scroll_pos, 3.0);
    assert_eq!(calls.get(), 2);
    assert_eq!(host.chat_scrollbar_pos(), 3);

    // refreshing guard: onNewMessage does nothing
    let calls2 = Rc::new(std::cell::Cell::new(0usize));
    m.refresh_head();
    m.on_new_message(2, &mut |_d| calls2.set(calls2.get() + 1));
    assert_eq!(m.target_scroll_pos, 3.0);
    assert_eq!(m.smooth_scroll_pos, 3.0);
    assert_eq!(calls2.get(), 0);
    m.refresh_tail();
}

#[test]
fn cc_add_lines_above_and_visible_lines_use_draw_offset() {
    let _guard = mixin_setup("cc-lines", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_mask_height = 2.0;
    m.smooth_scroll_pos = 2.5; // drawOffset = -(2 - 2.5) * 12 = 6.0
    host.set_chat_scrollbar_pos(2);

    assert_eq!(m.add_lines_above(1), 3); // ceil(2 + 6/12)
    assert_eq!(m.scrollbar_visible_lines(10), 9); // 10 - 1

    m.smooth_scroll_pos = 2.0; // drawOffset = 0
    assert_eq!(m.add_lines_above(1), 2);
    assert_eq!(m.scrollbar_visible_lines(10), 10);
}

#[test]
fn cc_scrollbar_smooth_java_round() {
    let _guard = mixin_setup("cc-sbar", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_scroll_pos = 2.5;

    // round(2.5 * 60 / 10 - 100) = round(-85) = -85
    assert_eq!(m.scrollbar_smooth(0, 10, 100, 60), -85);
}

#[test]
fn cc_scroll_wrap_and_reset_wrap() {
    let _guard = mixin_setup("cc-wrapm", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());

    // scrollWrap discards the direction and always calls operation(0)
    let seen = Rc::new(std::cell::Cell::new(-1i32));
    ChatComponentMixin::scroll_wrap(&mut |d| seen.set(d));
    assert_eq!(seen.get(), 0);

    // resetScrollWrap runs the op, then syncs target to chatScrollbarPos
    host.set_chat_scrollbar_pos(7);
    m.reset_scroll_wrap(&mut || {});
    assert_eq!(m.target_scroll_pos, 7.0);
}

#[test]
fn cc_opacity_by_opening_smoothness() {
    let _guard = mixin_setup("cc-opac", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());

    assert_eq!(m.opacity(0.3), 1.0);

    use std::sync::atomic::Ordering;
    vars::CHAT_OPENING_SMOOTHNESS.store(0.0f32.to_bits(), Ordering::Relaxed);
    assert_eq!(m.opacity(0.3), 0.3);
}

#[test]
fn cc_en_mask_debug_fills_when_enabled() {
    let _guard = mixin_setup("cc-debug", 1.0);
    cc_vars(0.5, 0.5, 24.0);
    use std::sync::atomic::Ordering;
    vars::ENABLE_MASK_DEBUG.store(true, Ordering::Relaxed);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let host = Rc::new(RecChatHost::new(5));
    let mut m = ChatComponentMixin::new(host.clone());
    m.smooth_scroll_pos = 2.0;
    m.target_scroll_pos = 2.0;
    m.smooth_mask_height = 2.0;
    m.target_mask_height = 2.0;

    let ta = RecTransAccess { params: std::cell::RefCell::new(None), context: Some(ex.clone()) };
    let mut op = || 0;
    m.for_visible_line_wrap(ex.as_ref(), Some(&ta), 100, &mut op);

    // debug fill ran between scissor and unscissor
    assert_eq!(*ex.log.borrow(), vec!["scissor".to_string(), "fill".to_string(), "unscissor".to_string()]);
}

// ------------------------------------------------------------- creative screen

use crate::duck::CreativeModeInventoryScreenDuck as CreativeDuck;
use crate::mixins::abstract_container_screen::{extract_slot_highlight_back_wrap, extract_slot_highlight_front_wrap};
use crate::mixins::creative_screen::{BlitArgs, CreativeModeInventoryScreenMixin, CreativeScreenHost, MouseButtonEvent};
use crate::mixins::item_picker_menu::{ItemPickerMenuHost, ItemPickerMenuMixin};

struct RecCreative {
    scroll_offs: std::cell::Cell<f32>,
    can_scroll: std::cell::Cell<bool>,
    items: std::cell::RefCell<Vec<String>>,
    row_count: i32,
    scroll_to: std::cell::RefCell<Vec<f32>>,
    caching: bool,
}

impl RecCreative {
    fn new() -> Self {
        Self {
            scroll_offs: std::cell::Cell::new(0.0),
            can_scroll: std::cell::Cell::new(true),
            items: std::cell::RefCell::new(Vec::new()),
            row_count: 2,
            scroll_to: std::cell::RefCell::new(Vec::new()),
            caching: false,
        }
    }
}

impl ItemPickerMenuHost for RecCreative {
    fn calculate_row_count(&self) -> i32 {
        self.row_count
    }
}

impl CreativeScreenHost for RecCreative {
    fn scroll_offs(&self) -> f32 {
        self.scroll_offs.get()
    }
    fn set_scroll_offs(&self, v: f32) {
        self.scroll_offs.set(v);
    }
    fn selected_tab_can_scroll(&self) -> bool {
        self.can_scroll.get()
    }
    fn menu_items_len(&self) -> usize {
        self.items.borrow().len()
    }
    fn menu_item(&self, index: usize) -> String {
        self.items.borrow()[index].clone()
    }
    fn menu_scroll_to(&self, offs: f32) {
        self.scroll_to.borrow_mut().push(offs);
    }
    fn calculated_row_count(&self) -> i32 {
        self.row_count
    }
    fn flow_is_caching_screen(&self) -> bool {
        self.caching
    }
}

fn cc_blit_args() -> BlitArgs {
    BlitArgs {
        pipeline: "p".into(),
        texture: "t".into(),
        x: 100,
        y: 200,
        u: 0.0,
        v: 0.0,
        width: 196,
        height: 166,
        texture_width: 256,
        texture_height: 256,
    }
}

fn ccr_vars(smoothness: f32, amount: f32, scissor_texture: bool) {
    use std::sync::atomic::Ordering;
    vars::CREATIVE_SCREEN_SMOOTHNESS.store(smoothness.to_bits(), Ordering::Relaxed);
    vars::CREATIVE_SCREEN_AMOUNT.store(amount.to_bits(), Ordering::Relaxed);
    vars::CREATIVE_USE_SCISSOR_TEXTURE.store(scissor_texture, Ordering::Relaxed);
}

fn cc_run_blit(ex: &Rc<RecExtractor>) -> impl FnMut(&BlitArgs) + '_ {
    move |a: &BlitArgs| {
        ex.blit_full(&a.pipeline, &a.texture, a.x, a.y, a.u, a.v, a.width, a.height, a.texture_width, a.texture_height);
    }
}

#[test]
fn cc_draw_background_scissored_strips_and_caching() {
    let _guard = mixin_setup("ccr-cache", 1.0);
    ccr_vars(0.5, 36.0, false);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let mut host = RecCreative::new();
    host.caching = true;
    let m = CreativeModeInventoryScreenMixin::new(Rc::new(host) as Rc<dyn CreativeScreenHost>);
    let a = cc_blit_args();
    let mut op = cc_run_blit(&ex);

    m.draw_background_wrap(ex.as_ref(), &a, &mut op, 100, 100);

    // iy = 217, iy + iheight - 1 = 306, ix = 108, ix + iwidth = 270
    assert_eq!(
        *ex.scissors.borrow(),
        vec![(-10000, -10000, 10000, 218), (-10000, 306, 10000, 10000), (-10000, 218, 108, 306), (270, 218, 10000, 306)]
    );
    // four strip blits with the original args
    assert_eq!(*ex.blits.borrow(), vec![(100, 200, 196, 166); 4]);
    // caching screen: no smoothing, no scroll, no item region
    assert_eq!(m.smooth_scroll_offs.get(), 0.0);
    assert_eq!(m.mouse_in_bounds.get(), true); // untouched default
}

#[test]
fn cc_draw_background_smooths_scroll_to_and_bounds() {
    let _guard = mixin_setup("ccr-smooth", 1.0);
    ccr_vars(0.5, 36.0, false);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let host = Rc::new(RecCreative::new());
    host.scroll_offs.set(0.25);
    let m = CreativeModeInventoryScreenMixin::new(host.clone());
    let a = cc_blit_args();
    let mut op = cc_run_blit(&ex);

    // mouse inside the item region: ix=108..270, iy=217..307
    m.draw_background_wrap(ex.as_ref(), &a, &mut op, 150, 250);

    // smooth = (0 - 0.25) * 0.5 + 0.25 = 0.125
    assert_eq!(m.smooth_scroll_offs.get(), 0.125);
    // mouseY - y = 50 <= 60 -> rowOffset 0 -> row_count 0 -> scroll_to(0.125)
    assert_eq!(m.row_offset.get(), 0);
    assert_eq!(*host.scroll_to.borrow(), vec![0.125]);
    assert_eq!(m.mouse_in_bounds.get(), true);

    // item region scissor after the four strips
    assert_eq!(ex.scissors.borrow()[4], (108, 218, 270, 306));
    // no scissor texture: two inner blits with the replaced args
    assert_eq!(ex.blits.borrow().len(), 6);
    assert_eq!(ex.blits.borrow()[4], (108, 217, 162, 90));
    assert_eq!(ex.blits.borrow()[5], (108, 217, 162, 90));
    // translates: y_offset, slotSize*5, drawOffset
    // y_offset = -(0.125 * 2 * 18 % 90) = -4.5 ; drawOffset = -(4.5 % 18) = -4.5
    assert_eq!(*ex.translates.borrow(), vec![(0.0, -4.5), (0.0, 90.0), (0.0, -4.5)]);
    // empty items -> no item draws
    assert_eq!(ex.log.borrow().iter().filter(|l| l.as_str() == "item").count(), 0);
}

#[test]
fn cc_draw_background_row_offset_when_mouse_low() {
    let _guard = mixin_setup("ccr-row", 1.0);
    ccr_vars(0.5, 36.0, false);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let mut host = RecCreative::new();
    for i in 0..63 {
        host.items.borrow_mut().push(format!("item{}", i));
    }
    let host = Rc::new(host);
    host.scroll_offs.set(0.25);
    let m = CreativeModeInventoryScreenMixin::new(host.clone());
    let a = cc_blit_args();
    let mut op = cc_run_blit(&ex);

    // mouseY - y = 70 > 60 and smooth != 0 -> rowOffset 1
    m.draw_background_wrap(ex.as_ref(), &a, &mut op, 150, 270);

    assert_eq!(m.row_offset.get(), 1);
    // row_count = 1 / 2 = 0 -> scroll_to(0.125 + 0)
    assert_eq!(*host.scroll_to.borrow(), vec![0.125]);
    // currRow = (int)(0.125 * 2) = 0 -> fromIndex = (0 + 5 - 5) * 9 = 0 -> 9 items
    assert_eq!(ex.log.borrow().iter().filter(|l| l.as_str() == "item").count(), 9);
}

#[test]
fn cc_get_draw_offset_and_conversions() {
    let _guard = mixin_setup("ccr-off", 1.0);
    ccr_vars(0.5, 36.0, false);
    let host = Rc::new(RecCreative::new());
    let m = CreativeModeInventoryScreenMixin::new(host.clone());
    m.smooth_scroll_offs.set(0.25);

    // pixels = 0.25 * 2 * 18 = 9 ; offset = -(9 % 18) + 0 = -9
    assert_eq!(m.get_draw_offset(), -9.0);
    m.row_offset.set(1);
    assert_eq!(m.get_draw_offset(), 9.0);
    m.row_offset.set(0);
    host.can_scroll.set(false);
    assert_eq!(m.get_draw_offset(), 0.0);
}

#[test]
fn cc_mouse_scrolled_wrap_scales_and_clamps() {
    let _guard = mixin_setup("ccr-scroll", 1.0);
    ccr_vars(0.5, 36.0, false);
    let host = Rc::new(RecCreative::new());
    host.scroll_offs.set(0.5);
    let m = CreativeModeInventoryScreenMixin::new(host.clone());

    // prev pixels = 0.5 * 2 * 18 = 9 ; 9 - 36 * 0.5 = -9 ; -9 / 18 / 2 = -0.25 -> clamp 0
    let ret = m.mouse_scrolled_wrap(0.0, 0.0, 0.0, 0.5, &mut |_, _, _, _| true);
    assert!(ret);
    assert_eq!(host.scroll_offs.get(), 0.0);
    assert_eq!(m.smooth_scroll_offs.get(), 0.0); // touchpad off

    // unscrollable: passthrough without touching scrollOffs
    host.scroll_offs.set(0.5);
    let ret = m.mouse_scrolled_wrap(0.0, 0.0, 0.0, 0.5, &mut |_, _, _, _| false);
    assert!(!ret);
    assert_eq!(host.scroll_offs.get(), 0.5);
}

#[test]
fn cc_extract_slots_mask_and_counter_demask() {
    let _guard = mixin_setup("ccr-slots", 1.0);
    ccr_vars(0.5, 36.0, false);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let host = Rc::new(RecCreative::new());
    let m = CreativeModeInventoryScreenMixin::new(host.clone());
    m.smooth_scroll_offs.set(0.25); // drawOffset -9

    m.extract_slots(ex.as_ref(), &mut || ex.log.borrow_mut().push("slots".into()));
    // mask opened: scissor (8,18,170,106), push, translate(0, -9)
    assert_eq!(ex.scissors.borrow().len(), 1);
    assert_eq!(ex.scissors.borrow()[0], (8, 18, 170, 106));
    assert_eq!(*ex.translates.borrow(), vec![(0.0, -9.0)]);

    // 45 slots -> counter hits 45 on the last one -> deMask
    for _ in 0..45 {
        m.extract_slot(ex.as_ref(), &mut || ex.log.borrow_mut().push("slot".into()));
    }
    {
        let log = ex.log.borrow();
        assert_eq!(log[0], "scissor");
        assert_eq!(log[log.len() - 1], "unscissor");
        assert_eq!(log[log.len() - 2], "pop");
        assert_eq!(log.iter().filter(|l| l.as_str() == "slot").count(), 45);
    }

    // non-scrollable tab: no mask at all
    ex.log.borrow_mut().clear();
    ex.scissors.borrow_mut().clear();
    ex.translates.borrow_mut().clear();
    host.can_scroll.set(false);
    m.extract_slots(ex.as_ref(), &mut || ex.log.borrow_mut().push("slots".into()));
    assert_eq!(*ex.log.borrow(), vec!["slots".to_string()]);
    assert!(ex.scissors.borrow().is_empty());
}

#[test]
fn cc_extract_contents_and_click_release_offset() {
    let _guard = mixin_setup("ccr-contents", 1.0);
    ccr_vars(0.5, 36.0, false);
    let host = Rc::new(RecCreative::new());
    let m = CreativeModeInventoryScreenMixin::new(host.clone());
    m.smooth_scroll_offs.set(0.25); // drawOffset -9 -> java round -> -9
    m.mouse_in_bounds.set(true);

    let ex2 = Rc::new(RecExtractor::new(1, 1));
    let got = std::cell::RefCell::new((0, 0));
    m.extract_contents(ex2.as_ref(), 10, 50, 0.0, &mut |x, y| { got.replace((x, y)); });
    // offset = -round(-9.0) = 9 -> mouseY + 9
    assert_eq!(got.borrow().1, 59);

    m.mouse_in_bounds.set(false);
    m.extract_contents(ex2.as_ref(), 10, 50, 0.0, &mut |x, y| { got.replace((x, y)); });
    assert_eq!(got.borrow().1, 50);

    // clicked / released shift the event y by the same offset
    m.mouse_in_bounds.set(true);
    let seen = std::cell::RefCell::new((0.0, false));
    let ret = m.mouse_clicked_wrap(&MouseButtonEvent { x: 10.0, y: 50.0, button: 1 }, true, &mut |e, d| {
        seen.replace((e.y, d));
        true
    });
    assert!(ret);
    assert_eq!(seen.borrow().0, 59.0);
    assert!(seen.borrow().1);
    m.mouse_released_wrap(&MouseButtonEvent { x: 10.0, y: 50.0, button: 1 }, &mut |e| {
        seen.replace((e.y, false));
        true
    });
    assert_eq!(seen.borrow().0, 59.0);
}

#[test]
fn cc_select_tab_resets_smooth() {
    let _guard = mixin_setup("ccr-tab", 1.0);
    ccr_vars(0.5, 36.0, false);
    let host = Rc::new(RecCreative::new());
    let m = CreativeModeInventoryScreenMixin::new(host);
    m.smooth_scroll_offs.set(0.5);

    let ran = std::cell::Cell::new(false);
    m.select_tab_wrap(&mut || ran.set(true));
    assert!(ran.get());
    assert_eq!(m.smooth_scroll_offs.get(), 0.0);
}

#[test]
fn cc_item_picker_row_index_and_scroll_passthrough() {
    let _guard = mixin_setup("ccr-picker", 1.0);
    let mut host = RecCreative::new();
    host.row_count = 5;
    let pm = ItemPickerMenuMixin;

    // (int)(scrollOffs * rowCount), floored at 0
    assert_eq!(pm.get_row_index_for_scroll(&host, 1.5), 7);
    assert_eq!(pm.get_row_index_for_scroll(&host, 0.4), 2);
    assert_eq!(pm.get_row_index_for_scroll(&host, -0.5), 0);

    let seen = std::cell::RefCell::new(0.0f32);
    pm.scroll_to_wrap(0.75, &mut |v| *seen.borrow_mut() = v);
    assert_eq!(*seen.borrow(), 0.75);
}

#[test]
fn cc_highlight_wraps_gate_on_duck() {
    let _guard = mixin_setup("ccr-hl", 1.0);
    let ex = Rc::new(RecExtractor::new(320, 240));

    struct RecDuck {
        log: Log,
        inbounds: bool,
        scrollable: bool,
    }
    impl CreativeDuck for RecDuck {
        fn en_mask(&self, _g: &dyn crate::seams::ExtractorSeam) {
            self.log.borrow_mut().push("mask".into());
        }
        fn de_mask(&self, _g: &dyn crate::seams::ExtractorSeam) {
            self.log.borrow_mut().push("unmask".into());
        }
        fn is_selected_tab_scrollable(&self) -> bool {
            self.scrollable
        }
        fn is_mouse_inbounds(&self) -> bool {
            self.inbounds
        }
    }

    // no creative screen: plain call
    extract_slot_highlight_back_wrap(ex.as_ref(), None, &mut |_| ex.log.borrow_mut().push("hl".into()));
    assert_eq!(*ex.log.borrow(), vec!["hl".to_string()]);

    // creative, in bounds, scrollable: masked
    ex.log.borrow_mut().clear();
    let duck = RecDuck { log: ex.log.clone(), inbounds: true, scrollable: true };
    extract_slot_highlight_back_wrap(ex.as_ref(), Some(&duck), &mut |_| ex.log.borrow_mut().push("hl".into()));
    assert_eq!(*ex.log.borrow(), vec!["mask".to_string(), "hl".to_string(), "unmask".to_string()]);

    // front wrap behaves the same
    ex.log.borrow_mut().clear();
    extract_slot_highlight_front_wrap(ex.as_ref(), Some(&duck), &mut |_| ex.log.borrow_mut().push("hl".into()));
    assert_eq!(*ex.log.borrow(), vec!["mask".to_string(), "hl".to_string(), "unmask".to_string()]);

    // out of bounds: plain call
    ex.log.borrow_mut().clear();
    let duck2 = RecDuck { log: ex.log.clone(), inbounds: false, scrollable: true };
    extract_slot_highlight_back_wrap(ex.as_ref(), Some(&duck2), &mut |_| ex.log.borrow_mut().push("hl".into()));
    assert_eq!(*ex.log.borrow(), vec!["hl".to_string()]);

    // tab not scrollable: plain call
    ex.log.borrow_mut().clear();
    let duck3 = RecDuck { log: ex.log.clone(), inbounds: true, scrollable: false };
    extract_slot_highlight_front_wrap(ex.as_ref(), Some(&duck3), &mut |_| ex.log.borrow_mut().push("hl".into()));
    assert_eq!(*ex.log.borrow(), vec!["hl".to_string()]);
}

#[test]
fn cc_draw_background_scissor_texture_path() {
    let _guard = mixin_setup("ccr-sct", 1.0);
    ccr_vars(0.5, 36.0, true);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let host = Rc::new(RecCreative::new());
    host.scroll_offs.set(0.25);
    let m = CreativeModeInventoryScreenMixin::new(host);
    let a = cc_blit_args();
    let mut op = cc_run_blit(&ex);

    m.draw_background_wrap(ex.as_ref(), &a, &mut op, 150, 250);

    // scissors: 4 strips + item region + two texture ops (iy and iy+iheight+1)
    let sc = ex.scissors.borrow();
    assert_eq!(sc[4], (108, 218, 270, 306));
    assert_eq!(sc[5], (108, 217, 270, 308));
    assert_eq!(sc[6], (108, 218, 270, 307));
    assert_eq!(sc.len(), 7);
    // all six blits keep the original args in this mode
    assert_eq!(*ex.blits.borrow(), vec![(100, 200, 196, 166); 6]);
}

#[test]
fn cc_draw_background_zero_row_count_skips_row_logic() {
    let _guard = mixin_setup("ccr-zero", 1.0);
    ccr_vars(0.5, 36.0, false);
    let ex = Rc::new(RecExtractor::new(320, 240));
    let mut host = RecCreative::new();
    host.row_count = 0;
    let host = Rc::new(host);
    host.scroll_offs.set(0.25);
    let m = CreativeModeInventoryScreenMixin::new(host.clone());
    let a = cc_blit_args();
    let mut op = cc_run_blit(&ex);

    m.draw_background_wrap(ex.as_ref(), &a, &mut op, 150, 270);

    // rowOffset untouched, no division, scroll_to(smooth + 0)
    assert_eq!(m.row_offset.get(), 0);
    assert_eq!(*host.scroll_to.borrow(), vec![0.125]);
    assert_eq!(m.mouse_in_bounds.get(), true);
}
