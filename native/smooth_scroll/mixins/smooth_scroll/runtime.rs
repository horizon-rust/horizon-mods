//! Exact 1:1 translation of Smooth Scrolling (SmajloSlovakian/
//! Minecraft-Smooth-Scrolling), upstream Java @639d45bcde33d7145c5282f81f4309dad31e6e2f,
//! licensed GPL-3.0-only: https://www.gnu.org/licenses/gpl-3.0.txt
//!
//! The Minecraft/Fabric APIs that do not exist on Horizon are host seams with
//! the same shapes and semantics (module seams). Rust port by sparticle,
//! upstream by SmajloSlovakian.

pub mod seams {
//! Host seams: every Minecraft / platform type upstream touches directly is
//! translated here as the exact 1:1 logic plus a thin injectable surface, so
//! the ported code keeps the upstream shape without a live Minecraft client.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// net.minecraft.network.chat.Component
// ---------------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub key: String,
    pub args: Vec<String>,
    pub literal: Option<String>,
}

impl Component {
    pub fn translatable(key: &str, args: &[&str]) -> Self {
        Self { key: key.to_string(), args: args.iter().map(|a| a.to_string()).collect(), literal: None }
    }
    pub fn literal(text: &str) -> Self {
        Self { key: String::new(), args: Vec::new(), literal: Some(text.to_string()) }
    }
    pub fn empty() -> Self {
        Self::literal("")
    }
    /// The resolved text under the test locale: literal text, else the key.
    pub fn text(&self) -> String {
        match &self.literal {
            Some(s) => s.clone(),
            None => {
                let mut s = self.key.clone();
                for a in &self.args {
                    s.push(' ');
                    s.push_str(a);
                }
                s
            }
        }
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.client.gui.components.AbstractWidget surface used upstream
// (active flag, tooltip, width, setMessage)
// ---------------------------------------------------------------------------
#[derive(Default)]
pub struct WidgetBase {
    pub active: Cell<bool>,
    pub visible: Cell<bool>,
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub x: Cell<i32>,
    pub y: Cell<i32>,
    pub tooltip: RefCell<Option<Component>>,
    pub focused: Cell<bool>,
}

// ---------------------------------------------------------------------------
// net.minecraft.client.gui.components.Button
// ---------------------------------------------------------------------------
pub struct Button {
    pub base: WidgetBase,
    pub message: RefCell<Component>,
    pub on_click: RefCell<Option<Rc<dyn Fn(&Rc<RefCell<Button>>)>>>,
}

impl Button {
    pub fn builder(
        message: Component,
        on_click: impl Fn(&Rc<RefCell<Button>>) + 'static,
    ) -> ButtonBuilder {
        ButtonBuilder {
            message,
            on_click: Rc::new(on_click),
            width: 150,
        }
    }
    pub fn set_message(&self, message: Component) {
        *self.message.borrow_mut() = message;
    }
    pub fn set_tooltip(&self, tooltip: Component) {
        *self.base.tooltip.borrow_mut() = Some(tooltip);
    }
    pub fn set_width(&self, width: i32) {
        self.base.width.set(width);
    }
    pub fn click(self_: &Rc<RefCell<Button>>) {
        let cb = self_.borrow().on_click.borrow().clone();
        if let Some(cb) = cb {
            cb(self_);
        }
    }
}

pub struct ButtonBuilder {
    message: Component,
    on_click: Rc<dyn Fn(&Rc<RefCell<Button>>)>,
    width: i32,
}

impl ButtonBuilder {
    pub fn build(self) -> Rc<RefCell<Button>> {
        let b = Button {
            base: WidgetBase { width: Cell::new(self.width), height: Cell::new(20), ..Default::default() },
            message: RefCell::new(self.message),
            on_click: RefCell::new(Some(self.on_click)),
        };
        Rc::new(RefCell::new(b))
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.client.gui.components.AbstractSliderButton base
// ---------------------------------------------------------------------------
pub struct SliderBase {
    pub base: WidgetBase,
    pub value: Cell<f64>,
    pub message: RefCell<Component>,
}

impl SliderBase {
    pub fn new(x: i32, y: i32, width: i32, height: i32, message: Component, value: f64) -> Self {
        Self {
            base: WidgetBase {
                x: Cell::new(x),
                y: Cell::new(y),
                width: Cell::new(width),
                height: Cell::new(height),
                ..Default::default()
            },
            value: Cell::new(value),
            message: RefCell::new(message),
        }
    }
    pub fn set_message(&self, message: Component) {
        *self.message.borrow_mut() = message;
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.client.model.geom.builders.UVPair
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct UVPair {
    pub u: f32,
    pub v: f32,
}

impl UVPair {
    pub fn new(u: f32, v: f32) -> Self {
        Self { u, v }
    }
}

// ---------------------------------------------------------------------------
// net.minecraft.client.gui.GuiGraphicsExtractor surface used upstream.
// The real screen driving is a host seam; the ported logic only needs the
// calls upstream makes.
// ---------------------------------------------------------------------------
pub trait GuiGraphicsSeam {
    fn text(&self, font_text: &str, text: &str, x: i32, y: i32, color: u32, shadow: bool);
}

struct NoGraphics;
impl GuiGraphicsSeam for NoGraphics {
    fn text(&self, _font_text: &str, _text: &str, _x: i32, _y: i32, _color: u32, _shadow: bool) {}
}

pub fn graphics() -> Rc<dyn GuiGraphicsSeam> {
    GRAPHICS.with(|g| g.borrow().clone().unwrap_or(Rc::new(NoGraphics) as Rc<dyn GuiGraphicsSeam>))
}

thread_local! {
    static GRAPHICS: RefCell<Option<Rc<dyn GuiGraphicsSeam>>> = const { RefCell::new(None) };
}

pub fn set_graphics(g: Rc<dyn GuiGraphicsSeam>) {
    GRAPHICS.with(|slot| *slot.borrow_mut() = Some(g));
}

// ARGB.color(alpha, r, g, b) 1:1.
pub fn argb_color(alpha: i32, r: i32, g: i32, b: i32) -> u32 {
    (((alpha & 0xFF) << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)) as u32
}

// ---------------------------------------------------------------------------
// Logging. Upstream logs through the SLF4J/Log4j loggers.
// ---------------------------------------------------------------------------
pub fn log_info(msg: &str) {
    println!("[Smooth Scrolling] {}", msg);
}

// ---------------------------------------------------------------------------
// The Minecraft client surface SmoothSc reads: the delta tracker (frame
// duration) and os.name. mc = Minecraft.getInstance() is a singleton in Java;
// the ported logic holds the one value it actually reads.
// ---------------------------------------------------------------------------
pub trait ClientEnv {
    /// Minecraft.getDeltaTracker().getGameTimeDeltaTicks()
    fn get_game_time_delta_ticks(&self) -> f32;
    /// Minecraft.font (for debugTextDraw)
    fn font(&self) -> String;
}

struct NoClient;
impl ClientEnv for NoClient {
    fn get_game_time_delta_ticks(&self) -> f32 {
        0.0
    }
    fn font(&self) -> String {
        String::new()
    }
}

thread_local! {
    static CLIENT_ENV: RefCell<Option<Rc<dyn ClientEnv>>> = const { RefCell::new(None) };
}

pub fn set_client_env(env: Rc<dyn ClientEnv>) {
    CLIENT_ENV.with(|slot| *slot.borrow_mut() = Some(env));
}

pub fn client_env() -> Rc<dyn ClientEnv> {
    CLIENT_ENV.with(|slot| {
        slot.borrow().clone().unwrap_or(Rc::new(NoClient) as Rc<dyn ClientEnv>)
    })
}

// ---------------------------------------------------------------------------
// FabricLoader.getInstance().getConfigDir()
// ---------------------------------------------------------------------------
pub fn config_dir() -> PathBuf {
    CONFIG_DIR.with(|d| d.borrow().clone())
}

pub fn set_config_dir(dir: PathBuf) {
    CONFIG_DIR.with(|d| *d.borrow_mut() = dir);
}

thread_local! {
    static CONFIG_DIR: RefCell<PathBuf> = RefCell::new(std::env::current_dir().unwrap_or_default());
}

// FabricLoader.getInstance().isModLoaded(name)
pub fn is_mod_loaded(_name: &str) -> bool {
    // Host seam: the packaging layer wires the real registry in.
    false
}

// ---------------------------------------------------------------------------
// org.lwjgl.sdl.SDL_Event, as SDLEventHandlerMixin reads it.
// ---------------------------------------------------------------------------
/// The SDL event surface SDLEventHandlerMixin touches: the event type and the
/// mouse-wheel sub-event (which device, y amount).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SdlEvent {
    /// SDL_Event.type()
    pub event_type: i32,
    /// SDL_Event.wheel().which()
    pub wheel_which: i32,
    /// SDL_Event.wheel().y()
    pub wheel_y: f32,
}

// ---------------------------------------------------------------------------
// net.minecraft.util.Util.getNanos()
// ---------------------------------------------------------------------------
thread_local! {
    static UTIL_NANOS_OVERRIDE: Cell<Option<u64>> = const { Cell::new(None) };
}

/// net.minecraft.util.Util.getNanos().
pub fn util_nanos() -> u64 {
    UTIL_NANOS_OVERRIDE.with(|c| c.get()).unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    })
}

/// Test hook pinning getNanos() so time-threshold logic is deterministic.
#[cfg(test)]
pub fn set_util_nanos(v: Option<u64>) {
    UTIL_NANOS_OVERRIDE.with(|c| c.set(v));
}

// ---------------------------------------------------------------------------
// net.minecraft.client.Minecraft as SDLEventHandlerMixin's pollEvents tail
// reaches it: execute(...), the window handle, MouseHandler.onScroll.
// ---------------------------------------------------------------------------
/// The Minecraft surface the pollEvents tail lambda uses.
pub trait MouseHost {
    /// Minecraft.execute(Runnable)
    fn execute(&self, task: Box<dyn FnOnce()>);
    /// Minecraft.getWindow().handle()
    fn window_handle(&self) -> u64;
    /// Minecraft.mouseHandler.onScroll(window, horizontal, vertical)
    fn on_scroll(&self, window: u64, horizontal: f64, vertical: f64);
}

thread_local! {
    static MOUSE_HOST: RefCell<Option<Rc<dyn MouseHost>>> = const { RefCell::new(None) };
}

pub fn set_mouse_host(h: Rc<dyn MouseHost>) {
    MOUSE_HOST.with(|c| *c.borrow_mut() = Some(h));
}

pub fn mouse_host() -> Rc<dyn MouseHost> {
    MOUSE_HOST.with(|c| c.borrow().clone()).expect("MouseHost seam not set")
}

// ---------------------------------------------------------------------------
// net.minecraft.client.gui.navigation.ScreenRectangle
// ---------------------------------------------------------------------------
/// ScreenRectangle as the ported code touches it (opaque geometry; only the
/// extents the translated logic needs so far).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRectangle {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl ScreenRectangle {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { x1, y1, x2, y2 }
    }
}

// ---------------------------------------------------------------------------
// com.mojang.renderpearl.api.commands.RenderPass.RenderArea
// ---------------------------------------------------------------------------
/// RenderArea as RenderPassMixin reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

// ---------------------------------------------------------------------------
// net.minecraft.client.gui.GuiGraphicsExtractor, as HudMixin touches it.
// ---------------------------------------------------------------------------
/// The GuiGraphicsExtractor surface the Hotbar mixins use.
pub trait ExtractorSeam {
    /// blitSprite(pipeline, location, x, y, width, height)
    fn blit_sprite(&self, pipeline: &str, location: &str, x: i32, y: i32, width: i32, height: i32);
    /// Hud.extractSlot(graphics, x, y, deltaTracker, player, stack, seed)
    fn extract_slot(&self, x: i32, y: i32, seed: i32);
    /// graphics.pose().pushMatrix() / popMatrix() / translate(dx, dy)
    fn pose_push(&self);
    fn pose_pop(&self);
    fn pose_translate(&self, dx: f32, dy: f32);
    /// enableScissor(x1, y1, x2, y2) / disableScissor()
    fn enable_scissor(&self, x1: i32, y1: i32, x2: i32, y2: i32);
    fn disable_scissor(&self);
    /// fill(x1, y1, x2, y2, color)
    fn fill(&self, x1: i32, y1: i32, x2: i32, y2: i32, color: u32);
    /// blit(pipeline, texture, x, y, u, v, width, height, textureWidth, textureHeight)
    fn blit_full(&self, pipeline: &str, texture: &str, x: i32, y: i32, u: f32, v: f32, width: i32, height: i32, texture_width: i32, texture_height: i32);
    /// graphics.item(stack, x, y)
    fn item(&self, item: &str, x: i32, y: i32);
    /// guiWidth() / guiHeight()
    fn gui_width(&self) -> i32;
    fn gui_height(&self) -> i32;
}

thread_local! {
    static EXTRACTOR: RefCell<Option<Rc<dyn ExtractorSeam>>> = const { RefCell::new(None) };
}

pub fn set_extractor(e: Rc<dyn ExtractorSeam>) {
    EXTRACTOR.with(|c| *c.borrow_mut() = Some(e));
}

pub fn extractor() -> Rc<dyn ExtractorSeam> {
    EXTRACTOR.with(|c| c.borrow().clone()).expect("ExtractorSeam not set")
}

// ---------------------------------------------------------------------------
// SmoothSc.mc.player's inventory + mc.gui.hud as MouseHandlerMixin reads them.
// ---------------------------------------------------------------------------
/// The Minecraft surface MouseHandlerMixin touches: player presence, the
/// selected hotbar slot, and the Hud cast to HudDuck.
pub trait PlayerSeam {
    /// SmoothSc.mc.player == null
    fn player_present(&self) -> bool;
    /// player.getInventory().getSelectedSlot()
    fn selected_slot(&self) -> i32;
}

thread_local! {
    static PLAYER_SEAM: RefCell<Option<Rc<dyn PlayerSeam>>> = const { RefCell::new(None) };
    static HUD_DUCK: RefCell<Option<Rc<std::cell::RefCell<dyn super::duck::HudDuck>>>> =
        const { RefCell::new(None) };
}

pub fn set_player_inventory(p: Rc<dyn PlayerSeam>) {
    PLAYER_SEAM.with(|c| *c.borrow_mut() = Some(p));
}

pub fn player_inventory() -> Rc<dyn PlayerSeam> {
    PLAYER_SEAM.with(|c| c.borrow().clone()).expect("PlayerSeam not set")
}

pub fn set_hud_duck(h: Rc<std::cell::RefCell<dyn super::duck::HudDuck>>) {
    HUD_DUCK.with(|c| *c.borrow_mut() = Some(h));
}

pub fn hud_duck() -> Rc<std::cell::RefCell<dyn super::duck::HudDuck>> {
    HUD_DUCK.with(|c| c.borrow().clone()).expect("HudDuck not set")
}

/// Java Math.round(float): floor(x + 0.5), not Rust's round-half-away.
pub fn java_round_f32(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}
}

pub mod smooth_sc {
//! Upstream SmoothScrolling.java (main entry, empty), SmoothSc.java (client
//! entry + shared statics + helpers) and Globals.java, 1:1.

use std::cell::RefCell;
use std::rc::Rc;

use super::seams::{self, GuiGraphicsSeam, UVPair};
use super::sm_sc_cfg::SmScCfg;

/// Upstream SmoothScrolling.java implements ModInitializer with an empty
/// onInitialize and only the id/logger.
pub mod smooth_scrolling {
    pub const LOGGER_NAME: &str = "smoothscroll";

    pub fn on_initialize() {
        // empty upstream
    }
}

/// Upstream SmoothSc.java.
pub struct SmoothSc;

impl SmoothSc {
    /// public static final Logger LOGGER ("Smooth Scrolling")
    pub const LOGGER_NAME: &'static str = "Smooth Scrolling";
}

// Java statics that mixins read live in globals.
thread_local! {
    /// Upstream SmoothSc.cfg (set in onInitializeClient).
    pub static CFG: RefCell<Option<Rc<RefCell<SmScCfg>>>> = const { RefCell::new(None) };
    /// Upstream SmoothSc.isSmoothScrollingRefurbishedLoaded.
    pub static IS_SMOOTH_SCROLLING_REFURBISHED_LOADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Upstream Globals.touchpadScrolledY.
    pub static TOUCHPAD_SCROLLED_Y: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn is_smooth_scrolling_refurbished_loaded() -> bool {
    IS_SMOOTH_SCROLLING_REFURBISHED_LOADED.get()
}

impl SmoothSc {
    /// public static boolean isWindows = System.getProperty("os.name").contains("Windows")
    pub fn is_windows() -> bool {
        std::env::consts::OS == "windows"
    }

    pub fn on_initialize_client() {
        IS_SMOOTH_SCROLLING_REFURBISHED_LOADED
            .set(seams::is_mod_loaded("smoothscrollingrefurbished"));
        SmoothSc::printt(&[
            SmoothSc::is_windows().to_string(),
            std::env::consts::OS.to_string(),
        ]);
        let cfg = SmScCfg::new();
        CFG.with(|slot| *slot.borrow_mut() = Some(cfg));
    }

    /// SmoothSc.cfg access, as the Java field would give.
    pub fn cfg() -> Rc<RefCell<SmScCfg>> {
        CFG.with(|slot| {
            slot.borrow()
                .clone()
                .expect("SmoothSc.cfg used before onInitializeClient")
        })
    }

    pub fn print(s: &str) {
        seams::log_info(s);
    }

    /// printt(Object... s) -> joinPrint(", ", s)
    pub fn printt(parts: &[String]) {
        SmoothSc::join_print(", ", parts);
    }

    pub fn join_print(join: &str, parts: &[String]) {
        let mut a = String::new();
        for object in parts {
            a.push_str(object);
            a.push_str(join);
        }
        seams::log_info(&a);
    }

    pub fn read_config() {
        SmoothSc::cfg().borrow().load_and_save();
    }

    pub fn clamp(val: i32, min: i32, max: i32) -> i32 {
        val.max(min).min(max)
    }

    pub fn get_last_frame_duration() -> f32 {
        seams::client_env().get_game_time_delta_ticks()
    }

    pub fn debug_text_draw(graphics: &dyn GuiGraphicsSeam, s: &str, x: i32, y: i32) {
        graphics.text(
            &seams::client_env().font(),
            s,
            x,
            y,
            seams::argb_color(255, 0, 255, 255),
            true,
        );
    }

    pub fn vec2f_add(a: UVPair, b: UVPair) -> UVPair {
        UVPair::new(a.u + b.u, a.v + b.v)
    }
    pub fn vec2f_sub(a: UVPair, b: UVPair) -> UVPair {
        UVPair::new(a.u - b.u, a.v - b.v)
    }
    pub fn vec2f_mul(a: UVPair, b: f32) -> UVPair {
        UVPair::new(a.u * b, a.v * b)
    }
}

/// Upstream Globals.touchpadScrolledY read/write.
pub fn touchpad_scrolled_y() -> bool {
    TOUCHPAD_SCROLLED_Y.get()
}

pub fn set_touchpad_scrolled_y(v: bool) {
    TOUCHPAD_SCROLLED_Y.set(v);
}
}

pub mod cfg_value {
//! Upstream cfg/CfgValue.java, 1:1.
//!
//! Java stores the value as Object; here it is the Value enum. Booleans and
//! doubles are the only boxed types the translation map ever keys on, which
//! keeps the upstream quirk where its Double map keys never match the Float
//! values looked up (see try_translate).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use serde_json::Value as Json;

use super::menu::custom_slider::CustomSlider;
use super::seams::{Button, Component};
use super::cfg_value_builder::CfgValueBuilder;

/// The widget handle kinds upstream checks with instanceof.
#[derive(Clone)]
pub enum WidgetHandle {
    Slider(Rc<RefCell<CustomSlider>>),
    Button(Rc<RefCell<Button>>),
    /// StringWidget: the section headers the config screen adds.
    Label(Component),
}

impl WidgetHandle {
    pub fn set_tooltip(&self, tooltip: Component) {
        match self {
            WidgetHandle::Slider(s) => *s.borrow().base.base.tooltip.borrow_mut() = Some(tooltip),
            WidgetHandle::Button(b) => b.borrow().set_tooltip(tooltip),
            WidgetHandle::Label(_) => {}
        }
    }
    pub fn set_active(&self, active: bool) {
        match self {
            WidgetHandle::Slider(s) => s.borrow().base.base.active.set(active),
            WidgetHandle::Button(b) => b.borrow().base.active.set(active),
            WidgetHandle::Label(_) => {}
        }
    }
    pub fn set_width(&self, width: i32) {
        match self {
            WidgetHandle::Slider(s) => s.borrow().base.base.width.set(width),
            WidgetHandle::Button(b) => b.borrow().set_width(width),
            WidgetHandle::Label(_) => {}
        }
    }
}

/// A CfgValue node, shared exactly like the Java references (children, parent,
/// widgets and screens all hold the same object).
pub struct CfgValue {
    pub(crate) default_value: Value,
    pub(crate) current_value: RefCell<Value>,
    pub(crate) value_name: String,
    pub(crate) is_fake: Cell<bool>,
    pub(crate) parent: RefCell<Option<Weak<RefCell<CfgValue>>>>,

    // GUI menu options
    pub(crate) min_val: Cell<f32>,
    pub(crate) max_val: Cell<f32>,
    pub(crate) step: Cell<f64>,
    pub(crate) temporary_value: RefCell<Value>,
    pub(crate) my_widget: RefCell<Option<WidgetHandle>>,
    pub(crate) my_reset_button: RefCell<Option<Rc<RefCell<Button>>>>,
    pub(crate) tooltiptxt: RefCell<Option<Component>>,
    pub(crate) translation_map: RefCell<HashMap<MapKey, String>>,
    pub(crate) format_key: RefCell<String>,
    pub(crate) translation_key: RefCell<Option<String>>,
    pub(crate) disable_when: RefCell<Option<Rc<dyn Fn(&Rc<RefCell<CfgValue>>) -> bool>>>,
}

/// Key of translationMap. Java boxes Boolean / Double objects; a Float value
/// looked up later can never equal a boxed Double key in Java, so Float values
/// keep their own kind here and miss on purpose (upstream quirk kept 1:1).
#[derive(Clone, Copy, Debug)]
pub enum MapKey {
    B(bool),
    D(f64),
}

impl PartialEq for MapKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MapKey::B(a), MapKey::B(b)) => a == b,
            (MapKey::D(a), MapKey::D(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for MapKey {}

impl std::hash::Hash for MapKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            MapKey::B(v) => {
                state.write_u8(0);
                state.write_u8(*v as u8);
            }
            MapKey::D(v) => {
                state.write_u8(1);
                state.write_u64(v.to_bits());
            }
        }
    }
}

#[derive(Clone, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    /// Upstream Number storage; parseJson and arithmetic store Float.
    Num(f32),
    Str(String),
    /// List<String> (the Notes block).
    Strs(Vec<String>),
    /// List<CfgValue>.
    Cfgs(Vec<Rc<RefCell<CfgValue>>>),
}

/// Java's equals()/toString() semantics for the boxed values. Lists compare
/// by element identity, the way holding references does.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Strs(a), Value::Strs(b)) => a == b,
            (Value::Cfgs(a), Value::Cfgs(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Rc::ptr_eq(x, y))
            }
            _ => false,
        }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Num(v) => write!(f, "{}", v),
            Value::Str(s) => write!(f, "{:?}", s),
            Value::Strs(l) => write!(f, "{:?}", l),
            Value::Cfgs(l) => write!(f, "Cfgs({})", l.len()),
        }
    }
}

/// Java value.getClass() equality: every List is the same class upstream.
pub(crate) fn class_of(v: &Value) -> u8 {
    match v {
        Value::Null => 4,
        Value::Bool(_) => 0,
        Value::Num(_) => 1,
        Value::Str(_) => 2,
        Value::Strs(_) | Value::Cfgs(_) => 3,
    }
}

impl CfgValue {
    pub fn new(name: &str, default_val: Value) -> Self {
        Self {
            current_value: RefCell::new(default_val.clone()),
            default_value: default_val.clone(),
            value_name: name.to_string(),
            is_fake: Cell::new(false),
            parent: RefCell::new(None),
            min_val: Cell::new(0.0),
            max_val: Cell::new(0.0),
            step: Cell::new(0.01),
            temporary_value: RefCell::new(default_val.clone()),
            my_widget: RefCell::new(None),
            my_reset_button: RefCell::new(None),
            tooltiptxt: RefCell::new(None),
            translation_map: RefCell::new(HashMap::from([
                (MapKey::B(true), "options.on".to_string()),
                (MapKey::B(false), "options.off".to_string()),
            ])),
            format_key: RefCell::new("smoothscroll.config.format.default".to_string()),
            translation_key: RefCell::new(None),
            disable_when: RefCell::new(None),
        }
    }

    /// Links the children of a Cfgs default to this node, as the Java
    /// constructor does. Runs after the node is behind an Rc.
    pub(crate) fn link_children(self_: &Rc<RefCell<CfgValue>>) {
        if let Some(list) = CfgValue::get_list(self_) {
            for cfg_value in list {
                *cfg_value.borrow().parent.borrow_mut() = Some(Rc::downgrade(self_));
            }
        }
    }

    pub fn builder(name: &str, default_val: Value) -> CfgValueBuilder {
        CfgValueBuilder::new(name, default_val)
    }

    /// get(String): ".." climbs to the parent, names look inside a CfgValue
    /// list, anything else returns a fake instance.
    pub fn get(self_: &Rc<RefCell<CfgValue>>, name: &str) -> Rc<RefCell<CfgValue>> {
        if name == ".." {
            let parent = self_.borrow().parent.borrow().clone();
            return match parent.and_then(|p| p.upgrade()) {
                Some(p) => p,
                None => CfgValue::make_fake(),
            };
        }

        let cfg_value_list = match CfgValue::get_list(self_) {
            Some(l) => l,
            None => return CfgValue::make_fake(),
        };

        for cfg_value in cfg_value_list {
            if cfg_value.borrow().value_name == name {
                return cfg_value;
            }
        }

        CfgValue::make_fake()
    }

    /// getList(): the list only when it is a CfgValue list.
    pub fn get_list(self_: &Rc<RefCell<CfgValue>>) -> Option<Vec<Rc<RefCell<CfgValue>>>> {
        let cur = self_.borrow().current_value.borrow().clone();
        match cur {
            Value::Cfgs(list) => Some(list),
            _ => None,
        }
    }

    pub fn get_name(self_: &Rc<RefCell<CfgValue>>) -> String {
        self_.borrow().value_name.clone()
    }

    pub fn get_value(self_: &Rc<RefCell<CfgValue>>) -> Value {
        self_.borrow().current_value.borrow().clone()
    }

    pub fn exists(self_: &Rc<RefCell<CfgValue>>) -> bool {
        !self_.borrow().is_fake.get()
    }

    pub fn make_fake() -> Rc<RefCell<CfgValue>> {
        let ret = Rc::new(RefCell::new(CfgValue::new("", Value::Null)));
        ret.borrow().is_fake.set(true);
        ret
    }

    pub fn set_value(self_: &Rc<RefCell<CfgValue>>, value: Value) {
        let b = self_.borrow_mut();
        *b.current_value.borrow_mut() = value.clone();
        *b.temporary_value.borrow_mut() = value;
    }

    pub fn set_temp_value(self_: &Rc<RefCell<CfgValue>>, value: Value) {
        *self_.borrow().temporary_value.borrow_mut() = value;
    }

    pub fn get_temp_value(self_: &Rc<RefCell<CfgValue>>) -> Value {
        self_.borrow().temporary_value.borrow().clone()
    }

    pub fn save_temp_value(self_: &Rc<RefCell<CfgValue>>) {
        let temp = self_.borrow().temporary_value.borrow().clone();
        *self_.borrow().current_value.borrow_mut() = temp;
    }

    pub fn reset_temp_value(self_: &Rc<RefCell<CfgValue>>) {
        let cur = self_.borrow().current_value.borrow().clone();
        *self_.borrow().temporary_value.borrow_mut() = cur;
    }

    pub fn default_to_temp(self_: &Rc<RefCell<CfgValue>>) {
        let default = self_.borrow().default_value.clone();
        *self_.borrow().temporary_value.borrow_mut() = default;
        let widget = self_.borrow().my_widget.borrow().clone();
        if let Some(widget) = widget {
            match widget {
                WidgetHandle::Slider(cs) => cs.borrow().refresh_value(),
                WidgetHandle::Button(bw) => {
                    let text = CfgValue::make_button_text(self_);
                    bw.borrow().set_message(text);
                }
                WidgetHandle::Label(_) => {}
            }
        }
    }

    pub fn recursive_save_temp_value(self_: &Rc<RefCell<CfgValue>>) {
        let a = CfgValue::get_list(self_);
        if let Some(a) = a {
            for cfg_value2 in a {
                CfgValue::recursive_save_temp_value(&cfg_value2);
            }
            return;
        }
        CfgValue::save_temp_value(self_);
    }

    pub fn generate_widget(
        self_: &Rc<RefCell<CfgValue>>,
    ) -> [Option<WidgetHandle>; 2] {
        let weak = Rc::downgrade(self_);
        let reset_button = Button::builder(
            Component::translatable("controls.reset", &[]),
            move |_button| {
                if let Some(self_) = weak.upgrade() {
                    CfgValue::default_to_temp(&self_);
                }
            },
        )
        .build();
        *self_.borrow().my_reset_button.borrow_mut() = Some(reset_button.clone());

        let value = CfgValue::get_value(self_);
        let my_widget = match value {
            Value::Num(_) => {
                let cs = CustomSlider::new(self_);
                let handle = WidgetHandle::Slider(cs.clone());
                CfgValue::assign_widget(self_, handle.clone());
                if let Some(tooltiptxt) = self_.borrow().tooltiptxt.borrow().clone() {
                    handle.set_tooltip(tooltiptxt);
                }
                Some(handle)
            }
            Value::Bool(_) => {
                let text = CfgValue::make_button_text(self_);
                let weak = Rc::downgrade(self_);
                let b = Button::builder(
                    text,
                    move |button| {
                        if let Some(self_) = weak.upgrade() {
                            let t = CfgValue::get_temp_value(&self_);
                            let toggled = match t {
                                Value::Bool(v) => Value::Bool(!v),
                                other => other,
                            };
                            CfgValue::set_temp_value(&self_, toggled);
                            button.borrow().set_message(CfgValue::make_button_text(&self_));
                        }
                    },
                )
                .build();
                let handle = WidgetHandle::Button(b);
                if let Some(tooltiptxt) = self_.borrow().tooltiptxt.borrow().clone() {
                    handle.set_tooltip(tooltiptxt);
                }
                CfgValue::assign_widget(self_, handle.clone());
                Some(handle)
            }
            _ => {
                // the fallthrough: a plain button with the current text and no action
                let b = Button::builder(CfgValue::make_button_text(self_), |_| {}).build();
                Some(WidgetHandle::Button(b))
            }
        };
        [my_widget, Some(WidgetHandle::Button(reset_button))]
    }

    fn make_button_text(self_: &Rc<RefCell<CfgValue>>) -> Component {
        let display = CfgValue::get_display_name(self_).text();
        let translated = CfgValue::try_translate(self_, &CfgValue::get_temp_value(self_));
        let format_key = self_.borrow().format_key.borrow().clone();
        Component::translatable(&format_key, &[&display, &translated.text()])
    }

    pub fn parse_json(name: &str, json_data: Option<Json>) -> Rc<RefCell<CfgValue>> {
        let a = Rc::new(RefCell::new(CfgValue::new(name, Value::Null)));
        let Some(json_data) = json_data else { return a };
        match json_data {
            Json::Object(obj_map) => {
                let mut b: Vec<Rc<RefCell<CfgValue>>> = Vec::new();
                for (element_name, element) in obj_map {
                    b.push(CfgValue::parse_json(&element_name, Some(element)));
                }
                CfgValue::set_value(&a, Value::Cfgs(b));
            }
            // TODO handle jsonarray (upstream leaves arrays unhandled too)
            Json::String(s) => CfgValue::set_value(&a, Value::Str(s)),
            Json::Bool(v) => CfgValue::set_value(&a, Value::Bool(v)),
            Json::Number(number) => {
                CfgValue::set_value(&a, Value::Num(number.as_f64().unwrap_or(0.0) as f32))
            }
            _ => {}
        }
        a
    }

    pub fn match_values(
        source: &Rc<RefCell<CfgValue>>,
        destination: &Rc<RefCell<CfgValue>>,
    ) {
        let source_value = CfgValue::get_value(source);
        if source_value == Value::Null {
            return;
        }
        let destination_value = CfgValue::get_value(destination);
        if class_of(&destination_value) != class_of(&source_value) {
            return;
        }
        let a = CfgValue::get_list(source);
        let b = CfgValue::get_list(destination);
        if a.is_none() && b.is_none() {
            CfgValue::set_value(destination, source_value);
        }

        if a.is_none() || b.is_none() {
            return;
        }
        for destval in b.unwrap() {
            for srcval in a.clone().unwrap() {
                if destval.borrow().value_name == srcval.borrow().value_name {
                    CfgValue::match_values(&srcval, &destval);
                }
            }
        }
    }

    pub fn export_json(self_: &Rc<RefCell<CfgValue>>) -> Json {
        if let Some(a) = CfgValue::get_list(self_) {
            let mut b = serde_json::Map::new();
            for cfg_value in a {
                let name = cfg_value.borrow().value_name.clone();
                b.insert(name, CfgValue::export_json(&cfg_value));
            }
            return Json::Object(b);
        }
        match CfgValue::get_value(self_) {
            Value::Null => Json::Null,
            Value::Bool(v) => Json::Bool(v),
            Value::Num(v) => serde_json::Number::from_f64(v as f64)
                .map(Json::Number)
                .unwrap_or(Json::Null),
            Value::Str(s) => Json::String(s),
            Value::Strs(list) => Json::Array(list.into_iter().map(Json::String).collect()),
            Value::Cfgs(_) => unreachable!("handled above"),
        }
    }

    pub fn to_string(self_: &Rc<RefCell<CfgValue>>) -> String {
        let b = self_.borrow();
        format!("({}:{:?})", b.value_name, *b.current_value.borrow())
    }

    pub fn get_step(self_: &Rc<RefCell<CfgValue>>) -> f64 {
        self_.borrow().step.get()
    }

    pub fn assign_widget(self_: &Rc<RefCell<CfgValue>>, cw: WidgetHandle) {
        *self_.borrow().my_widget.borrow_mut() = Some(cw);
    }

    pub fn get_max(self_: &Rc<RefCell<CfgValue>>) -> f64 {
        self_.borrow().max_val.get() as f64
    }

    pub fn get_min(self_: &Rc<RefCell<CfgValue>>) -> f64 {
        self_.borrow().min_val.get() as f64
    }

    pub fn get_format_key(self_: &Rc<RefCell<CfgValue>>) -> String {
        self_.borrow().format_key.borrow().clone()
    }

    pub fn try_translate(self_: &Rc<RefCell<CfgValue>>, value: &Value) -> Component {
        let key = match value {
            Value::Bool(v) => self_.borrow().translation_map.borrow().get(&MapKey::B(*v)).cloned(),
            // Float values cannot match the boxed Double keys upstream.
            Value::Num(_) => None,
            _ => None,
        };
        if let Some(key) = key {
            return Component::translatable(&key, &[]);
        }
        Component::literal(&format!("{}", ValueDisplay(value)))
    }

    pub fn set_translation_key(self_: &Rc<RefCell<CfgValue>>, key: &str) {
        *self_.borrow().translation_key.borrow_mut() = Some(key.to_string());
    }

    pub fn set_format_key(self_: &Rc<RefCell<CfgValue>>, key: &str) {
        *self_.borrow().format_key.borrow_mut() = key.to_string();
    }

    pub fn get_display_name(self_: &Rc<RefCell<CfgValue>>) -> Component {
        let key = self_.borrow().translation_key.borrow().clone();
        if let Some(key) = key {
            return Component::translatable(&key, &[]);
        }
        let name = self_.borrow().value_name.clone();
        Component::literal(&name)
    }

    pub fn refresh_disable_recursive(self_: &Rc<RefCell<CfgValue>>) {
        let list = CfgValue::get_list(self_);
        if list.is_none() {
            let disable_when = self_.borrow().disable_when.borrow().clone();
            let Some(disable_when) = disable_when else { return };
            let self_rc = self_.clone();
            let should_be_disabled = disable_when(&self_rc);
            let widget = self_.borrow().my_widget.borrow().clone();
            if let Some(widget) = widget {
                widget.set_active(!should_be_disabled);
            }
            let reset = self_.borrow().my_reset_button.borrow().clone();
            if let Some(reset) = reset {
                reset.borrow().base.active.set(!should_be_disabled);
            }
            return;
        }
        for cfg_value in list.unwrap() {
            CfgValue::refresh_disable_recursive(&cfg_value);
        }
    }

    pub fn en_helf_step(self_: &Rc<RefCell<CfgValue>>, val: f64) -> i64 {
        (val / CfgValue::get_step(self_)).round() as i64
    }
    pub fn en_step(self_: &Rc<RefCell<CfgValue>>, val: f64) -> f64 {
        CfgValue::en_helf_step(self_, val) as f64 * CfgValue::get_step(self_)
    }
    pub fn en_min_max(self_: &Rc<RefCell<CfgValue>>, val: f64) -> f64 {
        val * (CfgValue::get_max(self_) - CfgValue::get_min(self_)) + CfgValue::get_min(self_)
    }
    pub fn de_min_max(self_: &Rc<RefCell<CfgValue>>, val: f64) -> f64 {
        (val - CfgValue::get_min(self_)) / CfgValue::get_max(self_)
    }
}

/// Object.toString-style rendering for values inside tryTranslate.
struct ValueDisplay<'a>(&'a Value);
impl std::fmt::Display for ValueDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Value::Null => write!(f, "null"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Num(v) => write!(f, "{}", v),
            Value::Str(s) => write!(f, "{}", s),
            Value::Strs(_) | Value::Cfgs(_) => write!(f, "[list]"),
        }
    }
}
}

pub mod cfg_value_builder {
//! Upstream cfg/CfgValueBuilder.java, 1:1.

use std::cell::RefCell;
use std::rc::Rc;

use super::cfg_value::{CfgValue, MapKey, Value};
use super::seams::Component;

pub struct CfgValueBuilder {
    pub(crate) cfg_val: Rc<RefCell<CfgValue>>,
}

impl CfgValueBuilder {
    pub fn new(name: &str, default_val: Value) -> Self {
        Self {
            cfg_val: Rc::new(RefCell::new(CfgValue::new(name, default_val))),
        }
    }

    pub fn step(self, step: f64) -> Self {
        self.cfg_val.borrow().step.set(step);
        self
    }
    pub fn min_max(self, min: f32, max: f32) -> Self {
        let b = self.cfg_val.borrow();
        b.min_val.set(min);
        b.max_val.set(max);
        drop(b);
        self
    }
    pub fn tooltip(self, t: Component) -> Self {
        *self.cfg_val.borrow().tooltiptxt.borrow_mut() = Some(t);
        self
    }
    pub fn translatable(self, key: &str) -> Self {
        CfgValue::set_translation_key(&self.cfg_val, key);
        self
    }
    /// map(Object from, String to). Booleans map by Boolean key; numbers go in
    /// as Java doubles, which can never match a stored Float value on lookup.
    pub fn map(self, from: MapKey, to: &str) -> Self {
        self.cfg_val
            .borrow()
            .translation_map
            .borrow_mut()
            .insert(from, to.to_string());
        self
    }
    pub fn format(self, s: &str) -> Self {
        CfgValue::set_format_key(&self.cfg_val, s);
        self
    }
    pub fn disable_when(
        self,
        condition: impl Fn(&Rc<RefCell<CfgValue>>) -> bool + 'static,
    ) -> Self {
        *self.cfg_val.borrow().disable_when.borrow_mut() = Some(Rc::new(condition));
        self
    }

    pub fn build(self) -> Rc<RefCell<CfgValue>> {
        CfgValue::link_children(&self.cfg_val);
        self.cfg_val
    }
}

/// Convenience: the Java double literals the builder is called with.
pub fn map_f64(from: f64) -> MapKey {
    MapKey::D(from)
}
pub fn map_bool(from: bool) -> MapKey {
    MapKey::B(from)
}
}

pub mod new_config {
//! Upstream cfg/NewConfig.java, 1:1. The subclass hook methods live on the
//! ConfigHooks trait; SmScCfg supplies them exactly like the Java overrides.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value as Json};

use super::cfg_value::CfgValue;
use super::seams;

pub struct NewConfigBase {
    pub file_name: String,
    pub root: Rc<RefCell<CfgValue>>,
    pub raw_root: RefCell<Rc<RefCell<CfgValue>>>,
}

pub trait ConfigHooks {
    fn problem_writing(&self) {}
    fn problem_reading(&self) {}
    fn file_not_found(&self) {}
    /// Data corrects config values (updates config values from previous
    /// format, etc.). Called before into_variables().
    fn data_correct_permanent(&self) -> Result<(), String> {
        Ok(())
    }
    /// Inserts config values into variables.
    fn into_variables(&self) -> Result<(), String> {
        Ok(())
    }
    /// Modifies the config temporarily (values changed don't get saved to the
    /// file). Called after into_variables().
    fn data_correct_temporary(&self) {}
}

impl NewConfigBase {
    pub fn new(file: &str, template: Rc<RefCell<CfgValue>>) -> Self {
        let raw_root = CfgValue::parse_json("root", None);
        let base = Self {
            file_name: file.to_string(),
            root: template,
            raw_root: RefCell::new(raw_root),
        };
        // (Java's NewConfig constructor dispatches loadAndSave virtually to the
        // subclass; here the subclass drives the single load itself.)
        base
    }

    pub fn load_and_save(&self, hooks: &dyn ConfigHooks) {
        let dont_save = self.from_file(hooks);
        let corrected = hooks.data_correct_permanent();
        match corrected {
            Ok(()) => {
                if !dont_save {
                    self.to_file(hooks);
                }
                if hooks.into_variables().is_err() {
                    hooks.problem_reading();
                }
                hooks.data_correct_temporary();
            }
            Err(_) => {
                hooks.problem_reading();
                seams::log_info("Exception while loading config");
            }
        }
    }

    pub fn save(&self, hooks: &dyn ConfigHooks) {
        self.to_file(hooks);
        let _ = hooks.into_variables();
        hooks.data_correct_temporary();
    }

    pub(crate) fn from_file(&self, hooks: &dyn ConfigHooks) -> bool {
        let cfgfile = seams::config_dir().join(&self.file_name);
        let mut dont_save = false;
        match std::fs::read_to_string(&cfgfile) {
            Ok(data) => {
                let parsed: Result<Json, _> = serde_json::from_str(&data);
                match parsed {
                    Ok(json_data) => {
                        let raw_root = CfgValue::parse_json("root", Some(json_data));
                        *self.raw_root.borrow_mut() = raw_root.clone();
                        CfgValue::match_values(&raw_root, &self.root);
                    }
                    Err(_) => {
                        hooks.problem_reading();
                        dont_save = true;
                    }
                }
            }
            Err(_) => {
                hooks.file_not_found();
            }
        }
        dont_save
    }

    pub(crate) fn to_file(&self, hooks: &dyn ConfigHooks) {
        let cfgfile = seams::config_dir().join(&self.file_name);
        let data = serde_json::to_string_pretty(&CfgValue::export_json(&self.root))
            .unwrap_or_else(|_| "{}".to_string());
        match std::fs::write(&cfgfile, data) {
            Ok(()) => {}
            Err(_) => {
                hooks.problem_writing();
            }
        }
    }

    pub fn get_config_for_modifying(&self) -> Rc<RefCell<CfgValue>> {
        self.root.clone()
    }
}

struct NopHooks;
impl ConfigHooks for NopHooks {}

/// Gson's pretty printing emits `null` for a null value; serde_json matches.
pub fn gson_null() -> Json {
    json!(null)
}
}

pub mod sm_sc_cfg {
//! Upstream cfg/SmScCfg.java, 1:1: the config template, the static variable
//! imports (into_variables) and the format data corrections.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use super::cfg_value::{CfgValue, Value};
use super::cfg_value_builder::{map_f64, CfgValueBuilder};
use super::new_config::{ConfigHooks, NewConfigBase};
use super::smooth_sc::SmoothSc;

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
                            .tooltip(super::seams::Component::translatable(
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
                            .tooltip(super::seams::Component::translatable(
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
                            .tooltip(super::seams::Component::translatable(
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
                        .tooltip(super::seams::Component::translatable(
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
                            .tooltip(super::seams::Component::translatable(
                                "smoothscroll.config.touchpad.inertia_stop.tooltip",
                                &[],
                            ))
                            .min_max(0.0, 0.5)
                            .map(map_f64(0.0), "smoothscroll.config.value.off")
                            .step(0.001)
                            .build(),
                        CfgValue::builder("Inertia Start Threshold", Value::Num(0.05))
                            .translatable("smoothscroll.config.touchpad.inertia_start")
                            .tooltip(super::seams::Component::translatable(
                                "smoothscroll.config.touchpad.inertia_start.tooltip",
                                &[],
                            ))
                            .min_max(0.0, 1.0)
                            .map(map_f64(0.0), "smoothscroll.config.value.off")
                            .step(0.001)
                            .build(),
                        CfgValue::builder("Touchpad Threshold", Value::Num(0.25))
                            .translatable("smoothscroll.config.touchpad.threshold")
                            .tooltip(super::seams::Component::translatable(
                                "smoothscroll.config.touchpad.threshold.tooltip",
                                &[],
                            ))
                            .min_max(0.0, 5.0)
                            .map(map_f64(0.0), "smoothscroll.config.value.off")
                            .step(0.01)
                            .build(),
                        CfgValue::builder("Touchpad Time Threshold", Value::Num(0.02))
                            .translatable("smoothscroll.config.touchpad.time")
                            .tooltip(super::seams::Component::translatable(
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
                            .tooltip(super::seams::Component::translatable(
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
        if super::smooth_sc::is_smooth_scrolling_refurbished_loaded() {
            super::sm_sc_cfg::vars::set_entry_list_smoothness(0.0);
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
}

pub mod duck {
//! Upstream duck/ interfaces: capabilities mixins attach to host classes.

/// Upstream duck/ChatComponentDuck.java.
pub trait ChatComponentDuck {
    fn floaty_scroll(&mut self, scroll_y: f64);
}

/// Upstream duck/CreativeModeInventoryScreenDuck.java.
pub trait CreativeModeInventoryScreenDuck {
    fn en_mask(&self, graphics: &dyn super::seams::ExtractorSeam);
    fn de_mask(&self, graphics: &dyn super::seams::ExtractorSeam);
    fn is_selected_tab_scrollable(&self) -> bool;
    fn is_mouse_inbounds(&self) -> bool;
}

/// Upstream duck/HudDuck.java.
pub trait HudDuck {
    fn increase_rollover(&mut self);
    fn decrease_rollover(&mut self);
}
}

pub mod transformation_access {
//! Upstream TransformationAccess.java: the access-injected transformation
//! parameters on the host renderer plus its context.

use std::rc::Rc;

use super::seams::{GuiGraphicsSeam, ScreenRectangle};

/// Upstream net.minecraft.client.gui.ActiveTextCollector.Parameters: the
/// fields TransformationMixin shadows (pose, opacity, scissor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    /// Upstream Matrix3x2fc pose.
    pub pose: [f32; 6],
    pub opacity: f32,
    pub scissor: Option<ScreenRectangle>,
}

impl Parameters {
    /// Upstream new Parameters(pose, opacity, scissor).
    pub fn new(pose: [f32; 6], opacity: f32, scissor: Option<ScreenRectangle>) -> Self {
        Self { pose, opacity, scissor }
    }

    /// The host's original withScissor, the operation the mixin wraps: a copy
    /// of these parameters carrying the new scissor.
    pub fn with_scissor(&self, scissor: Option<ScreenRectangle>) -> Self {
        Parameters::new(self.pose, self.opacity, scissor)
    }
}

pub trait TransformationAccess {
    fn get_transformation(&self) -> Parameters;
    fn set_transformation(&mut self, transformation: Parameters);
    fn get_context(&self) -> Rc<dyn GuiGraphicsSeam>;
}
}

pub mod menu {
//! Upstream menu/ package, 1:1: the config screen machinery behind the
//! mod-menu entry point.


pub mod custom_slider {
//! Upstream menu/CustomSlider.java, 1:1.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use super::super::cfg_value::{CfgValue, Value, WidgetHandle};
use super::super::seams::{Component, SliderBase};

pub struct CustomSlider {
    /// AbstractSliderButton superclass state.
    pub base: SliderBase,
    pub(crate) txt: RefCell<String>,
    /// The CfgValue this slider edits (a weak ref; the value holds the widget).
    pub(crate) entry: RefCell<Weak<RefCell<CfgValue>>>,
}

impl CustomSlider {
    pub fn new(cfg_value: &Rc<RefCell<CfgValue>>) -> Rc<RefCell<CustomSlider>> {
        // super(0, 0, 150, 20, Component.literal(""), 0)
        let slider = Rc::new(RefCell::new(CustomSlider {
            base: SliderBase::new(0, 0, 150, 20, Component::literal(""), 0.0),
            txt: RefCell::new(String::new()),
            entry: RefCell::new(Rc::downgrade(cfg_value)),
        }));
        CfgValue::reset_temp_value(cfg_value);
        let value = match CfgValue::get_value(cfg_value) {
            Value::Num(v) => v as f64,
            _ => 0.0,
        };
        slider.borrow().base.value.set(CfgValue::de_min_max(cfg_value, value));
        *slider.borrow().txt.borrow_mut() = CfgValue::get_name(cfg_value);
        slider.borrow().update_message();
        slider
    }

    pub fn refresh_value(&self) {
        let entry = self.entry.borrow().upgrade().expect("slider entry dropped");
        let temp = match CfgValue::get_temp_value(&entry) {
            Value::Num(v) => v as f64,
            _ => 0.0,
        };
        self.base.value.set(CfgValue::de_min_max(&entry, temp));
        self.update_message();
    }

    fn make_text(&self) -> Component {
        let entry = self.entry.borrow().upgrade().expect("slider entry dropped");
        let display = CfgValue::get_display_name(&entry);
        let display_text = display.text();
        let shown = CfgValue::try_translate(
            &entry,
            &Value::Num(CfgValue::en_step(&entry, CfgValue::en_min_max(&entry, self.base.value.get())) as f32),
        );
        Component::translatable(
            &CfgValue::get_format_key(&entry),
            &[&display_text, &shown.text()],
        )
    }

    /// applyValue(): on a drag, write the stepped value back.
    pub fn apply_value(&self) {
        let entry = self.entry.borrow().upgrade().expect("slider entry dropped");
        let stepped = CfgValue::en_step(&entry, CfgValue::en_min_max(&entry, self.base.value.get()));
        CfgValue::set_temp_value(&entry, Value::Num(stepped as f32));
        self.base
            .value
            .set(CfgValue::de_min_max(&entry, CfgValue::en_step(&entry, CfgValue::en_min_max(&entry, self.base.value.get()))));
    }

    pub fn update_message(&self) {
        let text = self.make_text();
        self.base.set_message(text);
    }

    /// AbstractWidget.active, used by refreshDisableRecursive.
    pub fn set_active(slider: &Rc<RefCell<CustomSlider>>, active: bool) {
        slider.borrow().base.base.active.set(active);
    }
}

/// The AbstractWidget surface CfgValue sees on the slider handle.
pub fn slider_widget(slider: &Rc<RefCell<CustomSlider>>) -> WidgetHandle {
    let _ = Cell::new(0u8);
    WidgetHandle::Slider(slider.clone())
}
}

pub mod custom_tab {
//! Upstream menu/CustomTab.java, 1:1: a Tab that does not render but provides
//! inner widgets to TabNavigationBar instances.

use std::cell::RefCell;
use std::rc::Rc;

use super::super::cfg_value::WidgetHandle;
use super::super::seams::{Component, WidgetBase};

/// Host seam for net.minecraft.client.gui.components.tabs.Tab.
pub trait Tab {
    fn get_tab_title(&self) -> Component;
    /// Used to load/unload children when switching tab
    fn visit_children(&self, consumer: &mut dyn FnMut(&WidgetHandle));
    fn do_layout(&self, tab_area_x: i32, tab_area_y: i32, tab_area_width: i32, tab_area_height: i32);
    fn get_tab_extra_narration(&self) -> Component;
    /// getLayout(): upstream returns null here.
    fn get_layout(&self) -> Option<()>;
}

/// Host seam for net.minecraft.client.gui.navigation.ScreenRectangle.
pub struct ScreenRectangle {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub struct CustomTab {
    pub title: Component,
    pub children: RefCell<Vec<WidgetHandle>>,
    /// The AbstractWidget surface the tab's widgets share in the host screen.
    pub widget_base: WidgetBase,
}

impl CustomTab {
    pub fn new(title: Component, children: Vec<WidgetHandle>) -> Rc<RefCell<CustomTab>> {
        Rc::new(RefCell::new(CustomTab {
            title,
            children: RefCell::new(children),
            widget_base: WidgetBase::default(),
        }))
    }

    pub fn add_child(&self, child: WidgetHandle) {
        self.children.borrow_mut().push(child);
    }
}

impl Tab for CustomTab {
    fn get_tab_title(&self) -> Component {
        self.title.clone()
    }

    fn visit_children(&self, consumer: &mut dyn FnMut(&WidgetHandle)) {
        for child in self.children.borrow().iter() {
            consumer(child);
        }
    }

    fn do_layout(&self, _tab_area_x: i32, _tab_area_y: i32, _tab_area_width: i32, _tab_area_height: i32) {
        // Seems useless here
    }

    fn get_tab_extra_narration(&self) -> Component {
        Component::literal("Custom Tab Smooth Scrolling Settings")
    }

    fn get_layout(&self) -> Option<()> {
        super::super::smooth_sc::SmoothSc::print("TRIED TO GET LAYOUT OF CUSTOMTAB, RETURNING NULL");
        None
    }
}
}

pub mod mod_menu_impl {
//! Upstream menu/ModMenuImpl.java, 1:1: the mod-menu integration entry.

/// Host seam for com.terraformersmc.modmenu.api.ConfigScreenFactory: builds
/// the config screen with its parent screen.
pub type ConfigScreenFactory<'a> = dyn Fn(u64) -> super::smooth_scroll_config_screen::SmoothScrollConfigScreen + 'a;

pub struct ModMenuImpl;

impl ModMenuImpl {
    pub fn get_mod_config_screen_factory<'a>(
        parent: u64,
    ) -> Box<dyn Fn() -> super::smooth_scroll_config_screen::SmoothScrollConfigScreen + 'a>
    where
        Self: 'a,
    {
        Box::new(move || super::smooth_scroll_config_screen::SmoothScrollConfigScreen::new(parent))
    }
}
}

pub mod smooth_scroll_config_screen {
//! Upstream menu/SmoothScrollConfigScreen.java, 1:1. The Minecraft layout
//! classes (HeaderAndFooterLayout, GridLayout, ScrollableLayout, StringWidget)
//! are host seams; the widget generation, ordering and callbacks are the
//! translated logic.

use std::cell::RefCell;
use std::rc::Rc;

use super::super::cfg_value::{CfgValue, WidgetHandle};
use super::super::smooth_sc::SmoothSc;
use super::super::seams::{Button, Component};

/// Host seam for net.minecraft.client.gui.screens.Screen: collects renderable
/// widgets and knows the parent screen to return to.
pub trait ScreenHost {
    fn add_renderable_widget(&mut self, widget: WidgetHandle);
    /// minecraft.gui.setScreen(parent)
    fn set_screen(&mut self, parent: u64);
    /// layout.getContentHeight() as the host computed it.
    fn content_height(&self) -> i32;
    /// The two-column row helper appends pairs; the host positions them.
    fn add_row_pair(&mut self, left: WidgetHandle, right: WidgetHandle);
    fn add_wide(&mut self, widget: WidgetHandle, span: i32);
}

pub struct SmoothScrollConfigScreen {
    pub parent: u64,
    /// The buttons the init() step creates.
    pub button1: RefCell<Option<Rc<RefCell<Button>>>>,
    pub button2: RefCell<Option<Rc<RefCell<Button>>>>,
    /// The widget rows generated from the config tree.
    pub rows: RefCell<Vec<Row>>,
    /// Header title: Component.translatable("smoothscroll.config.title")
    pub title: Component,
    /// The widest tracked scrollable height for reposition_elements.
    pub scrollable_max_height: RefCell<i32>,
}

pub enum Row {
    Pair(WidgetHandle, WidgetHandle),
    Wide(WidgetHandle, i32),
}

impl SmoothScrollConfigScreen {
    pub fn new(parent: u64) -> Self {
        Self {
            parent,
            button1: RefCell::new(None),
            button2: RefCell::new(None),
            rows: RefCell::new(Vec::new()),
            title: Component::translatable("smoothscroll.config.title", &[]),
            scrollable_max_height: RefCell::new(0),
        }
    }

    pub fn init(&self, host: &mut dyn ScreenHost) {
        SmoothSc::cfg().borrow().load_and_save();

        let root = SmoothSc::cfg().borrow().get_config_for_modifying();
        let top_level = match CfgValue::get_list(&root) {
            Some(l) => l,
            None => return,
        };
        for cfg_value in top_level {
            let cfglist = CfgValue::get_list(&cfg_value);
            if let Some(cfglist) = cfglist {
                let display = CfgValue::get_display_name(&cfg_value);
                // StringWidget(display, font) spanning both columns
                host.add_wide(
                    WidgetHandle::Label(display),
                    2,
                );
                for inner_cfg_value in cfglist {
                    let a = CfgValue::generate_widget(&inner_cfg_value);
                    if let Some(a0) = &a[0] {
                        a0.set_width(150 * 2);
                        if let Some(a1) = &a[1] {
                            a1.set_width(50);
                            self.rows.borrow_mut().push(Row::Pair(a0.clone(), a1.clone()));
                            host.add_row_pair(a0.clone(), a1.clone());
                        }
                    }
                }
                host.add_wide(WidgetHandle::Label(Component::empty()), 2);
            }
        }

        let weak_cfg = Rc::downgrade(&SmoothSc::cfg());
        let button1 = Button::builder(
            Component::translatable("smoothscroll.config.save", &[]),
            move |_button| {
                if let Some(cfg) = weak_cfg.upgrade() {
                    let root = cfg.borrow().get_config_for_modifying();
                    CfgValue::recursive_save_temp_value(&root);
                    cfg.borrow().save();
                }
                // this.onClose() runs from the host after save
            },
        )
        .build();
        let button2 = Button::builder(
            Component::translatable("smoothscroll.config.exit", &[]),
            move |_button| {
                // this.onClose()
            },
        )
        .build();
        *self.button1.borrow_mut() = Some(button1.clone());
        *self.button2.borrow_mut() = Some(button2.clone());
        host.add_row_pair(WidgetHandle::Button(button1), WidgetHandle::Button(button2));
    }

    pub fn on_close(&self, host: &mut dyn ScreenHost) {
        host.set_screen(self.parent);
    }

    pub fn extract_render_state(&self, _mouse_x: i32, _mouse_y: i32, _delta: f32) {
        let root = SmoothSc::cfg().borrow().get_config_for_modifying();
        CfgValue::refresh_disable_recursive(&root);
        // super.extractRenderState(...) is the host render pass
    }

    pub fn reposition_elements(&self, content_height: i32) {
        *self.scrollable_max_height.borrow_mut() = content_height.max(0);
        // layout.arrangeElements() is the host pass
    }
}
}

}

pub mod mixins {
//! Upstream mixin/client/ package, 1:1.
//!
//! Mixin targets are Minecraft classes; each mixin's injected body ports 1:1
//! in its own module as the translation proceeds. MixinPlugin is the
//! conditional-application gate and is already translated.

//!   Chat/ChatScreenMixin.java, SuggestionsListMixin.java,
//!   ChatComponentMixin.java, CommandSuggestionsMixin.java
//!   CreativeScreen/CreativeModeInventoryScreenMixin.java,
//!   ItemPickerMenuMixin.java, AbstractContainerScreenMixin.java,
//!   ItemPickerMenuAccessor.java

/// Upstream mixin/client/MixinPlugin.java.
pub mod mixin_plugin {
    use super::super::seams::is_mod_loaded;

    pub fn should_apply_mixin(target_class_name: &str, mixin_class_name: &str) -> bool {
        let _ = target_class_name;
        if mixin_class_name.contains("CreativeScreenHandlerMixin_CC") {
            return is_mod_loaded("condensed_creative");
        }

        true
    }
}


pub mod abstract_container_screen {
//! Upstream mixin/client/CreativeScreen/AbstractContainerScreenMixin.java.

use super::super::duck::CreativeModeInventoryScreenDuck;
use super::super::seams::ExtractorSeam;

/// Upstream extractSlotHighlightBackWrap @WrapMethod. `creative` is the
/// screen viewed as CreativeModeInventoryScreenDuck (None when it is not the
/// creative screen).
pub fn extract_slot_highlight_back_wrap(
    graphics: &dyn ExtractorSeam,
    creative: Option<&dyn CreativeModeInventoryScreenDuck>,
    operation: &mut dyn FnMut(&dyn ExtractorSeam),
) {
    if let Some(a) = creative {
        if a.is_mouse_inbounds() && a.is_selected_tab_scrollable() {
            a.en_mask(graphics);
            operation(graphics);
            a.de_mask(graphics);
            return;
        }
    }
    operation(graphics);
}

/// Upstream extractSlotHighlightFrontWrap @WrapMethod.
pub fn extract_slot_highlight_front_wrap(
    graphics: &dyn ExtractorSeam,
    creative: Option<&dyn CreativeModeInventoryScreenDuck>,
    operation: &mut dyn FnMut(&dyn ExtractorSeam),
) {
    if let Some(a) = creative {
        if a.is_mouse_inbounds() && a.is_selected_tab_scrollable() {
            a.en_mask(graphics);
            operation(graphics);
            a.de_mask(graphics);
            return;
        }
    }
    operation(graphics);
}
}

pub mod abstract_scroll_area {
//! Upstream mixin/client/Widgets/AbstractScrollAreaMixin.java.
//!
//! The host's setScrollAmount dispatches the mixin's `set_scroll_amount_tail`
//! after storing the amount (Java virtual dispatch into the mixin instance),
//! so the mixin state lives in Cells: the dispatch cycle reenters it from the
//! host's shared reference.

use std::cell::Cell;

use super::super::sm_sc_cfg::vars;
use super::super::smooth_sc::touchpad_scrolled_y;

/// The AbstractScrollArea surface the mixin touches.
pub trait ScrollAreaHost {
    /// The scrollAmount shadow field: pixels scrolled down (up < down).
    fn scroll_amount(&self) -> f64;
    /// The host's setScrollAmount; after storing, it dispatches the mixin's
    /// `set_scroll_amount_tail` (the TAIL inject), which the mixin's own
    /// noSetScrollT guard silences around the updateScroll workaround.
    fn set_scroll_amount(&self, v: f64);
}

/// Upstream mixin/client/Widgets/AbstractScrollAreaMixin.java.
pub struct AbstractScrollAreaMixin {
    pub(crate) smooth_scroll_pos: Cell<f64>,
    pub(crate) target_scroll_pos: Cell<f64>,
    pub(crate) no_set_scroll_t: Cell<bool>,
}

impl Default for AbstractScrollAreaMixin {
    fn default() -> Self {
        Self {
            smooth_scroll_pos: Cell::new(0.0),
            target_scroll_pos: Cell::new(0.0),
            no_set_scroll_t: Cell::new(false),
        }
    }
}

impl AbstractScrollAreaMixin {
    /// Upstream setScrollT @Inject(method = "setScrollAmount", at = @At("TAIL")).
    pub fn set_scroll_amount_tail(&self, host: &dyn ScrollAreaHost) {
        if self.no_set_scroll_t.get() {
            return;
        }
        self.target_scroll_pos.set(host.scroll_amount());
        self.smooth_scroll_pos.set(host.scroll_amount());
    }

    /// Upstream updateScroll @Inject(method = "extractScrollbar", at = @At("HEAD")).
    pub fn update_scroll(&self, host: &dyn ScrollAreaHost) {
        if vars::entry_list_smoothness() == 0.0 {
            return;
        }

        self.smooth_scroll_pos
            .set((self.smooth_scroll_pos.get() - self.target_scroll_pos.get())
                * (vars::entry_list_smoothness() as f64)
                    .powf(super::super::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
                + self.target_scroll_pos.get());
        let rounded = self.smooth_scroll_pos.get().round();

        // TODO not so pretty workaround, might fix later
        // basically setscroll also makes the screen redraw
        self.no_set_scroll_t.set(true);
        host.set_scroll_amount(rounded);
        self.no_set_scroll_t.set(false);
    }

    /// Upstream mouseScrolledWrap @WrapMethod(method = "mouseScrolled").
    /// `operation` is the host's mouseScrolled.
    pub fn mouse_scrolled_wrap(
        &self,
        host: &dyn ScrollAreaHost,
        _mouse_x: f64,
        _mouse_y: f64,
        h_a: f64,
        v_a: f64,
        operation: &mut dyn FnMut() -> bool,
    ) -> bool {
        self.no_set_scroll_t.set(true);
        let prev_scroll_pos = host.scroll_amount();
        if vars::entry_list_smoothness() != 0.0 {
            host.set_scroll_amount(self.target_scroll_pos.get());
        }
        let ret = operation();

        if vars::entry_list_amount() != 0.0 && ret {
            host.set_scroll_amount(self.target_scroll_pos.get() - vars::entry_list_amount() * v_a);
        }
        self.target_scroll_pos.set(host.scroll_amount());

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos.set(self.target_scroll_pos.get());
        }

        if vars::entry_list_smoothness() != 0.0 {
            host.set_scroll_amount(prev_scroll_pos);
        }
        self.no_set_scroll_t.set(false);
        ret
    }
}
}

pub mod chat_component {
//! Upstream mixin/client/Chat/ChatComponentMixin.java.
//!
//! Priority >1000 upstream: bedrockify needs to move the chatBottom first, so
//! it can be smoothed out here. Unused upstream shadows (getScale as a shadow
//! method, isChatFocused) are not carried; scale arrives as a parameter.

use std::rc::Rc;

use super::super::duck::ChatComponentDuck;
use super::super::seams::ExtractorSeam;
use super::super::sm_sc_cfg::vars;
use super::super::smooth_sc::touchpad_scrolled_y;

/// Java Math.round(double): floor(x + 0.5).
fn java_round(v: f64) -> i32 {
    (v + 0.5).floor() as i64 as i32
}

/// Upstream `@Unique private double epsilon = 1d / 10`.
const EPSILON: f64 = 1.0 / 10.0;

/// The chatBottom sentinel: `@Unique private double smoothChatBottom = -69696969`.
const SMOOTH_CHAT_BOTTOM_UNSET: f64 = -69696969.0;

/// A scissor as `Parameters.withScissor(x, width, y, height)` carries it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChatScissor {
    pub x: i32,
    pub width: i32,
    pub y1: i32,
    pub y2: i32,
}

/// Upstream `ActiveTextCollector.Parameters` as the mixin sees it: an object
/// whose scissor can be set (null = no scissor). `Option<ChatParams>` itself
/// models the transformation being null.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ChatParams {
    pub scissor: Option<ChatScissor>,
}

/// Upstream TransformationAccess, which the two inner graphics-access mixins
/// implement for ChatComponent's drawing contexts.
pub trait TransformationAccess {
    fn get_transformation(&self) -> Option<ChatParams>;
    fn set_transformation(&self, transformation: Option<ChatParams>);
    fn get_context(&self) -> Option<&dyn ExtractorSeam>;
}

/// The ChatComponent surface the mixin touches (@Shadow fields and methods).
pub trait ChatComponentHost {
    /// The chatScrollbarPos shadow field.
    fn chat_scrollbar_pos(&self) -> i32;
    fn set_chat_scrollbar_pos(&self, v: i32);
    /// The shadowed getLineHeight() / getWidth() / getLinesPerPage().
    fn line_height(&self) -> i32;
    fn width(&self) -> i32;
    fn lines_per_page(&self) -> i32;
    /// The trimmedMessages shadow list: its size and each line's addedTime.
    fn trimmed_len(&self) -> usize;
    fn trimmed_added_time(&self, index: usize) -> i64;
    /// The shadowed scrollChat(int).
    fn scroll_chat(&self, dir: i32);
}

/// Upstream mixin/client/Chat/ChatComponentMixin.java (priority 1001).
pub struct ChatComponentMixin {
    host: Rc<dyn ChatComponentHost>,
    /// everything in lines for better float precision sync
    pub(crate) smooth_scroll_pos: f64,
    pub(crate) target_scroll_pos: f64,
    pub(crate) smooth_mask_height: f64,
    pub(crate) target_mask_height: f64,
    pub(crate) smooth_chat_bottom: f64,
    pub(crate) refreshing: bool,
}

impl ChatComponentMixin {
    pub fn new(host: Rc<dyn ChatComponentHost>) -> Self {
        Self {
            host,
            smooth_scroll_pos: 0.0,
            target_scroll_pos: 0.0,
            smooth_mask_height: 0.0,
            target_mask_height: 0.0,
            smooth_chat_bottom: SMOOTH_CHAT_BOTTOM_UNSET,
            refreshing: false,
        }
    }

    /// Upstream renderWrap @Inject(method = extractRenderState, at = HEAD).
    /// `foreground` is displayMode.foreground.
    pub fn render_wrap(&mut self, ticks: i64, foreground: bool) {
        let host = self.host.as_ref();
        self.smooth_scroll_pos = (self.smooth_scroll_pos - self.target_scroll_pos)
            * (vars::chat_smoothness() as f64)
                .powf(super::super::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + self.target_scroll_pos;

        // ~~snap on less than half a pixel difference~~
        // snap when there is a low enough scroll offset
        if (self.smooth_scroll_pos - self.target_scroll_pos).abs() * (host.line_height() as f64) < EPSILON {
            self.smooth_scroll_pos = self.target_scroll_pos;
        }

        host.set_chat_scrollbar_pos(self.smooth_scroll_pos.floor() as i32);

        // mask height
        let mut shown_line_count = 0usize;
        for r in 0..host.trimmed_len().min(host.lines_per_page() as usize) {
            if ticks - host.trimmed_added_time(r) < 200 || foreground {
                shown_line_count += 1;
            }
        }

        self.target_mask_height = shown_line_count as f64;
        self.smooth_mask_height = (self.smooth_mask_height - self.target_mask_height)
            * (vars::chat_opening_smoothness() as f64)
                .powf(super::super::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + self.target_mask_height;

        if (self.smooth_mask_height - self.target_mask_height).abs() * (host.line_height() as f64) < EPSILON {
            self.smooth_mask_height = self.target_mask_height;
        }
    }

    /// Upstream modifyYPos @ModifyVariable(method = extractRenderState, at =
    /// @At(value = "STORE"), name = "chatBottom"). Returns chatBottom unchanged.
    pub fn modify_y_pos(&mut self, graphics: &dyn ExtractorSeam, chat_bottom: i32, scale: f32) -> i32 {
        let host = self.host.as_ref();
        if self.smooth_chat_bottom == SMOOTH_CHAT_BOTTOM_UNSET {
            self.smooth_chat_bottom = chat_bottom as f64 / host.line_height() as f64;
        }
        self.smooth_chat_bottom = (self.smooth_chat_bottom - chat_bottom as f64 / host.line_height() as f64)
            * (vars::chat_opening_smoothness() as f64)
                .powf(super::super::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + chat_bottom as f64 / host.line_height() as f64;

        // originally i mapped the chat rendering to y = 0, but the chatting mod expects chatbottom to be unchanged
        // this means, we have to compensate for the moved chat with pose translating by -chatbottom
        graphics.pose_translate(
            0.0,
            self.smooth_chat_bottom as f32 * host.line_height() as f32 * scale - chat_bottom as f32 * scale,
        );
        chat_bottom
    }

    /// Upstream forVisibleLineWrap @WrapOperation on the forEachLine call in
    /// extractRenderState. `transformation` is the graphics viewed as
    /// TransformationAccess (None when it is not one).
    pub fn for_visible_line_wrap(
        &mut self,
        graphics: &dyn ExtractorSeam,
        transformation: Option<&dyn TransformationAccess>,
        chat_bottom: i32,
        operation: &mut dyn FnMut() -> i32,
    ) -> i32 {
        if let Some(transformation) = transformation {
            self.en_mask(transformation, chat_bottom);
        }
        graphics.pose_translate(0.0, self.get_draw_offset() as f32);

        let ret = operation();

        graphics.pose_translate(0.0, -(self.get_draw_offset() as f32));
        if let Some(transformation) = transformation {
            self.de_mask(transformation);
        }
        ret
    }

    /// Upstream enMask @Unique.
    fn en_mask(&self, transformation_access: &dyn TransformationAccess, chat_bottom: i32) {
        let host = self.host.as_ref();
        let mut mask_top = java_round((-self.smooth_mask_height) * host.line_height() as f64) + chat_bottom;
        let mut mask_bottom = 0 + chat_bottom; //(int) Math.round(smoothChatBottom * getLineHeight());

        // this lets underlined text, diacritics and stuff overflow two pixels above or under chat
        if self.smooth_scroll_pos == self.target_scroll_pos && self.get_draw_offset() == 0.0 && self.smooth_mask_height != 0.0 {
            if (self.smooth_mask_height - self.target_mask_height).abs() * (host.line_height() as f64) < EPSILON {
                mask_top -= 32;
            }
            mask_bottom += 32;
        }

        // this only affects text and the other only affects everything else... wtf mojank?
        let a = transformation_access.get_transformation();
        if a.is_some() {
            transformation_access.set_transformation(Some(ChatParams {
                scissor: Some(ChatScissor { x: -10, width: host.width() + 1000, y1: mask_top, y2: mask_bottom }),
            }));
        }

        let context = transformation_access.get_context();
        if let Some(context) = context {
            context.enable_scissor(-10, mask_top, host.width() + 1000, mask_bottom);
        }
        if vars::enable_mask_debug() {
            let context = context.expect("upstream dereferences context here unconditionally");
            context.fill(
                -10000,
                -10000,
                context.gui_width() + 999999999,
                context.gui_height() + 999999999,
                super::super::seams::argb_color(50, 255, 255, 0),
            );
        }
    }

    /// Upstream deMask @Unique.
    fn de_mask(&self, transformation_access: &dyn TransformationAccess) {
        if transformation_access.get_transformation().is_some() {
            transformation_access.set_transformation(Some(ChatParams { scissor: None })); // a.withScissor(null)
        }

        if let Some(context) = transformation_access.get_context() {
            context.disable_scissor();
        }
    }

    /// Upstream scrollWrap @WrapMethod(method = "scrollChat").
    /// this disables the vanilla scrolling but still clamps the values
    /// the method gets called in addMessageToDisplayQueue, but we replace that behaviour in onNewMessage, so that doesn't matter
    /// mod compatibility hopefully doesn't hinder
    pub fn scroll_wrap(operation: &mut dyn FnMut(i32)) {
        operation(0);
    }

    /// Upstream floatyScroll (ChatComponentDuck), with the host's scrollChat
    /// as seen through scrollWrap.
    pub fn floaty_scroll_impl(&mut self, scroll_y: f64, scroll_chat_wrapped: &mut dyn FnMut(i32)) {
        // target + mousescrollamount * lineamount
        let host = self.host.as_ref();
        let new_target = self.target_scroll_pos
            + (scroll_y / 7.0)
                * (if vars::chat_amount() != 0.0 {
                    vars::chat_amount() / host.line_height() as f32
                } else {
                    7.0
                }) as f64;
        self.target_scroll_pos = self.clamp_scroll(new_target, scroll_chat_wrapped);

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos = self.target_scroll_pos;
        }
    }

    /// Upstream clampScroll @Unique.
    fn clamp_scroll(&mut self, mut clamping: f64, scroll_chat_wrapped: &mut dyn FnMut(i32)) -> f64 {
        let host = self.host.as_ref();
        host.set_chat_scrollbar_pos(clamping.ceil() as i32);

        // we'll only use the clamp from the call
        scroll_chat_wrapped(0);

        if clamping > host.chat_scrollbar_pos() as f64 {
            clamping = host.chat_scrollbar_pos() as f64;
        }
        if clamping < 0.0 {
            clamping = 0.0;
        }

        clamping
    }

    /// Upstream opacity @ModifyVariable(method = "forEachLine", at = @At(value =
    /// "STORE"), name = "alpha").
    pub fn opacity(&self, alpha: f32) -> f32 {
        if vars::chat_opening_smoothness() == 0.0 {
            return alpha;
        }
        1.0
    }

    /// Upstream onNewMessage @ModifyVariable(method = "addMessageToDisplayQueue",
    /// at = @At("STORE"), name = "lines"). `new_lines` is lines.size(); the
    /// list itself passes through unchanged.
    pub fn on_new_message(&mut self, new_lines: usize, scroll_chat_wrapped: &mut dyn FnMut(i32)) {
        if self.refreshing {
            return;
        }
        if self.target_scroll_pos > 0.0 {
            self.target_scroll_pos += new_lines as f64;
            self.target_scroll_pos = self.clamp_scroll(self.target_scroll_pos, scroll_chat_wrapped);
        }
        self.smooth_scroll_pos += new_lines as f64;
        scroll_chat_wrapped(0);
    }

    /// Upstream addLinesAbove @ModifyVariable(method = "forEachLine", at =
    /// @At("STORE"), name = "perPage").
    pub fn add_lines_above(&self, _per_page: i32) -> i32 {
        (self.smooth_mask_height + self.get_draw_offset() / self.host.line_height() as f64).ceil() as i32
    }

    /// Upstream resetScrollWrap @WrapMethod(method = "resetChatScroll").
    pub fn reset_scroll_wrap(&mut self, operation: &mut dyn FnMut()) {
        operation();
        self.target_scroll_pos = self.host.chat_scrollbar_pos() as f64;
    }

    /// Upstream scrollbarVisibleLines @ModifyVariable(method =
    /// extractRenderState, at = @At(value = "STORE"), name = "count").
    pub fn scrollbar_visible_lines(&self, count: i32) -> i32 {
        count - (if self.get_draw_offset() == 0.0 { 0 } else { 1 })
    }

    /// Upstream scrollbarSmooth @ModifyVariable(method = extractRenderState,
    /// at = @At(value = "STORE"), name = "y").
    pub fn scrollbar_smooth(&self, _y: i32, total: i32, chat_bottom: i32, chat_height: i32) -> i32 {
        java_round(self.smooth_scroll_pos * chat_height as f64 / total as f64 - chat_bottom as f64)
    }

    /// Upstream refreshH @Inject(refreshTrimmedMessages, HEAD).
    pub fn refresh_head(&mut self) {
        self.refreshing = true;
    }

    /// Upstream refreshT @Inject(refreshTrimmedMessages, TAIL).
    pub fn refresh_tail(&mut self) {
        self.refreshing = false;
    }

    /// Upstream getDrawOffset @Unique.
    pub fn get_draw_offset(&self) -> f64 {
        -(self.host.chat_scrollbar_pos() as f64 - self.smooth_scroll_pos) * self.host.line_height() as f64
    }
}

impl ChatComponentDuck for ChatComponentMixin {
    fn floaty_scroll(&mut self, scroll_y: f64) {
        let host = self.host.clone();
        self.floaty_scroll_impl(scroll_y, &mut |_dir| {
            // the wrapped scrollChat: the wrap discards the direction and
            // calls the host's scrollChat(0)
            Self::scroll_wrap(&mut |d| host.scroll_chat(d));
        });
    }
}
}

pub mod chat_screen {
//! Upstream mixin/client/Chat/ChatScreenMixin.java.

use super::super::duck::ChatComponentDuck;

/// Upstream unclampScroll @WrapOperation on the Mth.clamp(DDD)D call in
/// ChatScreen.mouseScrolled.
pub fn unclamp_scroll(value: f64, _min: f64, _max: f64) -> f64 {
    // return value; //TODO test windows for bugs caused by unclamping this
    value
}

/// Upstream mouseScrolledWrap @WrapOperation on ChatComponent.scrollChat(I)V
/// in ChatScreen.mouseScrolled. The wrap discards the vanilla scroll call and
/// routes the mouse's scrollY into the chat component's floatyScroll.
pub fn mouse_scrolled_wrap(chat_component: &mut dyn ChatComponentDuck, _dir: i32, scroll_y: f64) {
    chat_component.floaty_scroll(scroll_y);
}
}

pub mod command_suggestions {
//! Upstream mixin/client/Chat/CommandSuggestionsMixin.java.

/// Upstream unclampScroll @WrapOperation on the Mth.clamp(DDD)D call in
/// CommandSuggestions.mouseScrolled.
pub fn unclamp_scroll(value: f64, _min: f64, _max: f64) -> f64 {
    // return value; //TODO test on windows for bugs caused by unclamping this
    value
}
}

pub mod creative_screen {
//! Upstream mixin/client/CreativeScreen/CreativeModeInventoryScreenMixin.java.
//!
//! TODO upstream: check compatibility with item borders, flow. The unused
//! upstream constructor, the commented test-item-count block, and the unused
//! ItemPickerMenu scroll wrap are carried as comments only.

use std::rc::Rc;

use super::super::duck::CreativeModeInventoryScreenDuck;
use super::super::seams::ExtractorSeam;
use super::super::seams::java_round_f32;
use super::super::sm_sc_cfg::vars;
use super::super::smooth_sc::SmoothSc;
use super::super::smooth_sc::touchpad_scrolled_y;

/// Upstream `@Unique private int slotSize = 18` (never mutated).
const SLOT_SIZE: i32 = 18;

/// Upstream `@Unique private int scrollItemCount = 45`.
const SCROLL_ITEM_COUNT: i32 = 45;

/// Upstream MouseButtonEvent as the wraps rebuild it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseButtonEvent {
    pub x: f64,
    pub y: f64,
    /// The event's buttonInfo.
    pub button: i32,
}

/// The blit arguments the extractBackground wrap intercepts and replays.
#[derive(Clone, Debug)]
pub struct BlitArgs {
    pub pipeline: String,
    pub texture: String,
    pub x: i32,
    pub y: i32,
    pub u: f32,
    pub v: f32,
    pub width: i32,
    pub height: i32,
    pub texture_width: i32,
    pub texture_height: i32,
}

/// The CreativeModeInventoryScreen surface the mixin touches.
pub trait CreativeScreenHost {
    /// The scrollOffs shadow field (the scroll target).
    fn scroll_offs(&self) -> f32;
    fn set_scroll_offs(&self, v: f32);
    /// The static selectedTab's canScroll().
    fn selected_tab_can_scroll(&self) -> bool;
    /// menu.items: its size and each entry.
    fn menu_items_len(&self) -> usize;
    fn menu_item(&self, index: usize) -> String;
    /// menu.scrollTo(f)
    fn menu_scroll_to(&self, offs: f32);
    /// ((ItemPickerMenuAccessor) menu).getCalculatedRowCount()
    fn calculated_row_count(&self) -> i32;
    /// FabricLoader object share "flow:is_caching_screen" instanceof Boolean b && b.
    fn flow_is_caching_screen(&self) -> bool;
}

/// Upstream mixin/client/CreativeScreen/CreativeModeInventoryScreenMixin.java.
pub struct CreativeModeInventoryScreenMixin {
    host: Rc<dyn CreativeScreenHost>,
    pub(crate) mouse_in_bounds: std::cell::Cell<bool>,
    pub(crate) row_offset: std::cell::Cell<i32>,
    pub(crate) smooth_scroll_offs: std::cell::Cell<f32>,
    item_counter: std::cell::Cell<i32>,
}

impl CreativeModeInventoryScreenMixin {
    pub fn new(host: Rc<dyn CreativeScreenHost>) -> Self {
        Self {
            host,
            mouse_in_bounds: std::cell::Cell::new(true),
            row_offset: std::cell::Cell::new(0),
            smooth_scroll_offs: std::cell::Cell::new(0.0),
            item_counter: std::cell::Cell::new(SCROLL_ITEM_COUNT),
        }
    }

    fn slot_size(&self) -> i32 {
        SLOT_SIZE
    }

    /// Upstream drawBackgroundWrap @WrapOperation on the extractBackground blit.
    /// `operation` re-invokes the blit with the given args on the graphics.
    pub fn draw_background_wrap(
        &self,
        graphics: &dyn ExtractorSeam,
        a: &BlitArgs,
        operation: &mut dyn FnMut(&BlitArgs),
        mouse_x: i32,
        mouse_y: i32,
    ) {
        let host = self.host.as_ref();
        let x = a.x;
        let y = a.y;
        let ix = x + 8;
        let iy = y + 17;
        let iu = a.u + 8.0;
        let iv = a.v + 17.0;
        let iwidth = 162;
        let iheight = 90;

        // transparent background compatibility: up, down, left, right
        graphics.enable_scissor(-10000, -10000, 10000, iy + 1);
        operation(a);
        graphics.disable_scissor();
        graphics.enable_scissor(-10000, iy + iheight - 1, 10000, 10000);
        operation(a);
        graphics.disable_scissor();
        graphics.enable_scissor(-10000, iy + 1, ix, iy + iheight - 1);
        operation(a);
        graphics.disable_scissor();
        graphics.enable_scissor(ix + iwidth, iy + 1, 10000, iy + iheight - 1);
        operation(a);
        graphics.disable_scissor();

        if host.flow_is_caching_screen() {
            return;
        }

        self.smooth_scroll_offs.set(
            (self.smooth_scroll_offs.get() - host.scroll_offs())
                * vars::creative_screen_smoothness().powf(SmoothSc::get_last_frame_duration())
                + host.scroll_offs(),
        );

        let mut row_count = self.calculate_row_count();
        if row_count != 0 {
            if mouse_y - y > 60 && self.smooth_scroll_offs.get() != 0.0 {
                self.row_offset.set(1);
            } else {
                self.row_offset.set(0);
            }
            row_count = self.row_offset.get() / row_count;
        }
        host.menu_scroll_to(self.smooth_scroll_offs.get() + row_count as f32);

        self.mouse_in_bounds.set(
            mouse_x >= ix && mouse_x < ix + iwidth && mouse_y >= iy && mouse_y < iy + iheight,
        );

        let y_offset = -(self.scroll_offs_to_pixels(self.smooth_scroll_offs.get())
            % (self.slot_size() * 5) as f32);

        graphics.enable_scissor(ix, iy + 1, ix + iwidth, iy + iheight - 1);

        graphics.pose_push();
        graphics.pose_translate(0.0, y_offset);

        if vars::creative_use_scissor_texture() {
            graphics.enable_scissor(ix, iy, ix + iwidth, iy + iheight + 1);
            operation(a);
            graphics.disable_scissor();

            graphics.pose_translate(0.0, (self.slot_size() * 5) as f32);

            graphics.enable_scissor(ix, iy + 1, ix + iwidth, iy + iheight);
            operation(a);
            graphics.disable_scissor();
        } else {
            let inner = BlitArgs {
                x: ix,
                y: iy,
                u: iu,
                v: iv,
                width: iwidth,
                height: iheight,
                ..a.clone()
            };
            operation(&inner);

            graphics.pose_translate(0.0, (self.slot_size() * 5) as f32);

            operation(&inner);
        }
        graphics.pose_pop();

        graphics.pose_push();
        graphics.pose_translate(0.0, self.get_draw_offset());
        let curr_row = (self.smooth_scroll_offs.get() * self.calculate_row_count() as f32) as f64 as i32;
        let from_index = (curr_row + 5 - self.row_offset.get() * 5) * 9;
        let mut i = from_index;
        while i >= 0 && (i as usize) < host.menu_items_len() && i < from_index + 9 {
            let item = host.menu_item(i as usize);
            graphics.item(
                &item,
                x + 9 + i % 9 * self.slot_size(),
                y + 6 * self.slot_size() - self.row_offset.get() * 6 * self.slot_size(),
            );
            i += 1;
        }
        graphics.pose_pop();

        graphics.disable_scissor();
    }

    /// Upstream extractSlots override. `run_super` is the host's own
    /// AbstractContainerScreen.extractSlots.
    pub fn extract_slots(&self, graphics: &dyn ExtractorSeam, run_super: &mut dyn FnMut()) {
        if self.host.selected_tab_can_scroll() {
            self.item_counter.set(0);
            self.en_mask(graphics);
        }
        run_super();
    }

    /// Upstream extractSlot override. `run_super` is the host's own
    /// AbstractContainerScreen.extractSlot.
    pub fn extract_slot(&self, graphics: &dyn ExtractorSeam, run_super: &mut dyn FnMut()) {
        run_super();

        self.item_counter.set(self.item_counter.get() + 1);
        if self.item_counter.get() == SCROLL_ITEM_COUNT {
            self.de_mask(graphics);
        }
    }

    /// Upstream mouseScrolledWrap @WrapMethod.
    pub fn mouse_scrolled_wrap(
        &self,
        x: f64,
        y: f64,
        scroll_x: f64,
        scroll_y: f64,
        operation: &mut dyn FnMut(f64, f64, f64, f64) -> bool,
    ) -> bool {
        let host = self.host.as_ref();
        let prev_scroll = host.scroll_offs();
        let ret = operation(x, y, scroll_x, scroll_y);

        // if unscrollable
        if !ret {
            return ret;
        }

        let mut pixelscroll = self.scroll_offs_to_pixels(prev_scroll);
        pixelscroll -= vars::creative_screen_amount() * scroll_y as f32;
        host.set_scroll_offs(self.pixels_to_scroll_offs(pixelscroll).clamp(0.0, 1.0));
        if touchpad_scrolled_y() {
            self.smooth_scroll_offs.set(host.scroll_offs());
        }

        ret
    }

    /// Upstream extractContents override.
    pub fn extract_contents(
        &self,
        graphics: &dyn ExtractorSeam,
        mouse_x: i32,
        mouse_y: i32,
        a: f32,
        run_super: &mut dyn FnMut(i32, i32),
    ) {
        let offset = if self.mouse_in_bounds.get() {
            -java_round_f32(self.get_draw_offset())
        } else {
            0
        };
        let _ = (graphics, a);
        run_super(mouse_x, mouse_y + offset);
    }

    /// Upstream mouseClickedWrap @WrapMethod.
    pub fn mouse_clicked_wrap(
        &self,
        event: &MouseButtonEvent,
        double_click: bool,
        operation: &mut dyn FnMut(&MouseButtonEvent, bool) -> bool,
    ) -> bool {
        let offset = if self.mouse_in_bounds.get() {
            -java_round_f32(self.get_draw_offset())
        } else {
            0
        };
        operation(
            &MouseButtonEvent { x: event.x, y: event.y + offset as f64, button: event.button },
            double_click,
        )
    }

    /// Upstream mouseReleasedWrap @WrapMethod.
    pub fn mouse_released_wrap(
        &self,
        event: &MouseButtonEvent,
        operation: &mut dyn FnMut(&MouseButtonEvent) -> bool,
    ) -> bool {
        let offset = if self.mouse_in_bounds.get() {
            -java_round_f32(self.get_draw_offset())
        } else {
            0
        };
        operation(&MouseButtonEvent { x: event.x, y: event.y + offset as f64, button: event.button })
    }

    /// Upstream selectTabWrap @WrapMethod (the tab argument only passes
    /// through to the operation).
    pub fn select_tab_wrap(&self, operation: &mut dyn FnMut()) {
        self.smooth_scroll_offs.set(0.0);
        operation();
    }

    /// Upstream getDrawOffset @Unique.
    pub fn get_draw_offset(&self) -> f32 {
        if !self.host.selected_tab_can_scroll() {
            return 0.0;
        }
        -self.scroll_offs_to_pixels(self.smooth_scroll_offs.get()) % self.slot_size() as f32
            + self.slot_size() as f32 * self.row_offset.get() as f32
    }

    /// Upstream scrollOffsToPixels @Unique.
    fn scroll_offs_to_pixels(&self, scroll_offs: f32) -> f32 {
        scroll_offs * self.calculate_row_count() as f32 * self.slot_size() as f32
    }

    /// Upstream pixelsToScrollOffs @Unique.
    fn pixels_to_scroll_offs(&self, pixels: f32) -> f32 {
        pixels / self.slot_size() as f32 / self.calculate_row_count() as f32
    }

    /// Upstream calculateRowCount @Unique (through the accessor).
    fn calculate_row_count(&self) -> i32 {
        self.host.calculated_row_count()
    }
}

impl CreativeModeInventoryScreenDuck for CreativeModeInventoryScreenMixin {
    /// Upstream enMask @Override (duck).
    fn en_mask(&self, graphics: &dyn ExtractorSeam) {
        graphics.enable_scissor(8, 18, 170, 106);
        graphics.pose_push();
        graphics.pose_translate(0.0, self.get_draw_offset());
    }

    /// Upstream deMask @Override (duck).
    fn de_mask(&self, graphics: &dyn ExtractorSeam) {
        graphics.pose_pop();

        graphics.disable_scissor();
    }

    fn is_mouse_inbounds(&self) -> bool {
        self.mouse_in_bounds.get()
    }

    fn is_selected_tab_scrollable(&self) -> bool {
        self.host.selected_tab_can_scroll()
    }
}
}

pub mod edit_box {
//! Upstream mixin/client/Widgets/EditBoxMixin.java.
//!
//! Java String.length() counts UTF-16 units; the port works in chars, which
//! is the same for the ASCII text these paths were written against.

use super::super::seams::extractor;
use super::super::seams::java_round_f32;
use super::super::sm_sc_cfg::vars;
use super::super::smooth_sc::touchpad_scrolled_y;

/// The EditBox surface the mixin touches (shadow fields + Font).
pub trait EditBoxHost {
    fn is_hovered(&self) -> bool;
    /// The value shadow field.
    fn value(&self) -> String;
    /// Font.width(s)
    fn font_width(&self, s: &str) -> i32;
    /// Font.plainSubstrByWidth(s, width): s trimmed to fit width.
    fn plain_substr(&self, s: &str, width: i32) -> String;
    /// The displayPos shadow field.
    fn display_pos(&self) -> i32;
    fn set_display_pos(&self, v: i32);
    /// The textX / textY shadow fields and the shadowed getInnerWidth().
    fn text_x(&self) -> i32;
    fn text_y(&self) -> i32;
    fn inner_width(&self) -> i32;
}

/// Upstream mixin/client/Widgets/EditBoxMixin.java.
pub struct EditBoxMixin {
    /// in pixels (left < right)
    pub(crate) smooth_scroll_pos: f32,
    pub(crate) target_scroll_pos: f32,
    pub(crate) prev_cursor_pixel: f32,
}

impl Default for EditBoxMixin {
    fn default() -> Self {
        Self { smooth_scroll_pos: 0.0, target_scroll_pos: 0.0, prev_cursor_pixel: 0.0 }
    }
}

impl EditBoxMixin {
    /// Upstream mouseScrolled override.
    /// safety net - vanilla checks for hovering before calling this function,
    /// mods occasionally do not
    pub fn mouse_scrolled(&mut self, host: &dyn EditBoxHost, h_a: f64, v_a: f64) -> bool {
        if !host.is_hovered() {
            return false;
        }
        self.target_scroll_pos = ((self.target_scroll_pos as f64
            - (v_a - h_a) * vars::text_amount() as f64)
            .clamp(0.0, host.font_width(&host.value()) as f64)) as f32;

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos = self.target_scroll_pos;
        }
        true
    }

    /// Upstream renderWidgetWrap @WrapMethod(method = "extractWidgetRenderState").
    pub fn render_widget_wrap(&mut self, host: &dyn EditBoxHost, operation: &mut dyn FnMut()) {
        self.smooth_scroll_pos = (self.smooth_scroll_pos - self.target_scroll_pos)
            * (vars::text_smoothness() as f64)
                .powf(super::super::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
                as f32
            + self.target_scroll_pos;

        let shown = host.plain_substr(&host.value(), java_round_f32(self.smooth_scroll_pos));
        host.set_display_pos(shown.chars().count() as i32);

        let g = extractor();
        g.enable_scissor(host.text_x(), host.text_y() - 10, host.text_x() + host.inner_width(), host.text_y() + 10);
        g.pose_push();
        g.pose_translate(self.get_draw_offset(host), 0.0);
        operation();
        g.pose_pop();
        if vars::enable_mask_debug() {
            g.fill(-100, -100, g.gui_width(), g.gui_height(), super::super::seams::argb_color(50, 0, 255, 255));
        }
        g.disable_scissor();
    }

    /// Upstream drawBackgroundWrap @WrapOperation on drawBackground's
    /// blitSprite. Runs after the host popped its pose and disabled the
    /// scissor; the wrap re-arms both around the wrapped call.
    pub fn draw_background_wrap(&mut self, host: &dyn EditBoxHost, operation: &mut dyn FnMut()) {
        let g = extractor();
        g.pose_pop(); // TODO somehow remove this code duplication
        g.disable_scissor();
        operation();
        g.enable_scissor(host.text_x(), host.text_y() - 10, host.text_x() + host.inner_width(), host.text_y() + 10);
        g.pose_push();
        g.pose_translate(self.get_draw_offset(host), 0.0);
    }

    /// Upstream addCharacters @ModifyVariable(method = "extractWidgetRenderState",
    /// at = @At(value = "STORE"), name = "displayed").
    pub fn add_characters(&self, host: &dyn EditBoxHost, displayed: &str) -> String {
        let value = host.value();
        let dp = host.display_pos() as usize;
        let dlen = displayed.chars().count();
        if dp + dlen >= value.chars().count() || dlen == 0 {
            return displayed.to_string();
        }

        let first_char_width = host.font_width(&displayed.chars().next().unwrap().to_string());
        let tail: String = value.chars().skip(dp + dlen).collect();
        let mut additional_chars = host.plain_substr(&tail, first_char_width);
        let total_len = dp + dlen + additional_chars.chars().count();
        if value.chars().count() > total_len {
            additional_chars.push(value.chars().nth(total_len).unwrap());
        }

        format!("{}{}", displayed, additional_chars)
    }

    /// Upstream findClickedPositionInTextWrap @WrapMethod(method =
    /// "findClickedPositionInText"). `button_info` is event.buttonInfo().
    pub fn find_clicked_position_in_text_wrap(
        &self,
        host: &dyn EditBoxHost,
        event_x: f32,
        event_y: f32,
        button_info: &str,
        operation: &mut dyn FnMut(f32, f32, &str) -> i32,
    ) -> i32 {
        operation(event_x - self.get_draw_offset(host), event_y, button_info)
    }

    /// Upstream scrollToWrap @WrapMethod(method = "scrollTo").
    pub fn scroll_to_wrap(&mut self, host: &dyn EditBoxHost, pos: i32, operation: &mut dyn FnMut(i32)) {
        if !vars::text_custom_update() {
            operation(pos);
            let prefix: String = host.value().chars().take(host.display_pos() as usize).collect();
            self.target_scroll_pos = host.font_width(&prefix) as f32;
            return;
        }

        let value = host.value();
        let prefix: String = value.chars().take(pos as usize).collect();
        let cursor_pixel = host.font_width(&prefix) as f32;
        let margin = vars::text_margin() / 100.0 * host.inner_width() as f32;
        let inner = host.inner_width() as f32;

        if vars::text_margin() > 50.0 {
            if cursor_pixel < self.prev_cursor_pixel && cursor_pixel < self.target_scroll_pos + margin {
                self.target_scroll_pos = cursor_pixel - margin;
            } else if cursor_pixel > self.prev_cursor_pixel
                && cursor_pixel > self.target_scroll_pos + inner - margin
            {
                self.target_scroll_pos = cursor_pixel - inner + margin;
            }
            self.target_scroll_pos = self.target_scroll_pos.clamp(cursor_pixel - inner, cursor_pixel);
        } else if cursor_pixel < self.target_scroll_pos + margin {
            self.target_scroll_pos = cursor_pixel - margin;
        } else if cursor_pixel > self.target_scroll_pos + inner - margin {
            self.target_scroll_pos = cursor_pixel - inner + margin;
        }
        self.target_scroll_pos = self.target_scroll_pos.clamp(0.0, host.font_width(&value) as f32);
        host.set_display_pos(0);
        self.prev_cursor_pixel = cursor_pixel;
    }

    /// Upstream getDrawOffset @Unique.
    fn get_draw_offset(&self, host: &dyn EditBoxHost) -> f32 {
        let shown = host.plain_substr(&host.value(), java_round_f32(self.smooth_scroll_pos));
        host.font_width(&shown) as f32 - self.smooth_scroll_pos
    }
}
}

pub mod hud {
//! Upstream mixin/client/Hotbar/HudMixin.java.
//!
//! Mixin priority 999 (upstream comment: <1000 so bedrockify applying first
//! doesn't crash modifyarg; >= -999999999 so the wrapoperation applies after
//! the raised mod fixing raised hotbar selectors).

use super::super::duck::HudDuck;
use super::super::sm_sc_cfg::vars;
use super::super::seams::{argb_color, extractor};

/// Upstream mixin/client/Hotbar/HudMixin.java.
pub struct HudMixin {
    pub(crate) slot_width: i32,
    pub(crate) slot_count: i32,
    pub(crate) rollover_space: i32,
    pub(crate) masked: bool,
    /// pixels to the right of hotbar start (upstream comment)
    pub(crate) smooth_selector_pos: f32,
    pub(crate) rollover: i32,
}

impl Default for HudMixin {
    fn default() -> Self {
        Self {
            slot_width: 20,
            slot_count: 9,
            rollover_space: 4,
            masked: false,
            smooth_selector_pos: 0.0,
            rollover: 0,
        }
    }
}

impl HudMixin {
    /// Upstream extractHotbarWrap @WrapMethod(method = "extractItemHotbar").
    /// `operation` is the host's extractItemHotbar; the graphics extractor and
    /// delta tracker ride the seams.
    pub fn extract_hotbar_wrap(&mut self, operation: &mut dyn FnMut()) {
        let selected = super::super::seams::player_inventory().selected_slot();

        self.rollover_space = if vars::static_selector() { 1 } else { 4 };
        let target = (selected - self.rollover * self.slot_count) * self.slot_width
            - self.rollover * self.rollover_space;
        self.smooth_selector_pos = (((self.smooth_selector_pos - target as f32) as f64
            * (vars::hotbar_smoothness() as f64).powf(super::super::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + target as f64)) as f32;

        if self.smooth_selector_pos < 0.0 {
            self.smooth_selector_pos += (self.slot_count * self.slot_width + self.rollover_space) as f32;
            self.rollover -= 1;
        } else if self.smooth_selector_pos
            > (self.rollover_space * 2 + self.slot_width * self.slot_count) as f32
        {
            self.smooth_selector_pos -= (self.slot_count * self.slot_width + self.rollover_space) as f32;
            self.rollover += 1;
        }

        operation();
    }

    /// Upstream moveSelector @WrapOperation on the second blitSprite call in
    /// extractItemHotbar. `x` is the call's x argument; `operation` is the
    /// wrapped blitSprite with those arguments.
    pub fn move_selector(&mut self, x: i32, operation: &mut dyn FnMut()) {
        let selected = super::super::seams::player_inventory().selected_slot();
        let g = extractor();
        let hotbar_start = x - selected * self.slot_width;

        if vars::static_selector() {
            g.pose_push();
            g.pose_translate(-((x - hotbar_start) as f32) + (self.slot_count / 2 * self.slot_width) as f32, 0.0);

            operation();

            g.pose_pop();
            return;
        }

        if self.smooth_selector_pos.round() > (self.slot_width * 8) as f32 {
            self.enable_mask();
            g.pose_push();
            g.pose_translate(
                self.smooth_selector_pos - (x - hotbar_start) as f32
                    - (self.slot_count * self.slot_width + self.rollover_space) as f32,
                0.0,
            );

            operation();

            g.pose_pop();
        }

        g.pose_push();
        g.pose_translate(self.smooth_selector_pos - (x - hotbar_start) as f32, 0.0);
        operation();
        g.pose_pop();

        if self.masked {
            if vars::enable_mask_debug() {
                g.fill(-100, -100, g.gui_width(), g.gui_height(), argb_color(50, 0, 255, 255));
            }
            g.disable_scissor();
            self.masked = false;
        }
    }

    /// Upstream enableMask @Unique.
    fn enable_mask(&mut self) {
        let g = extractor();
        let x2 = g.gui_width() / 2 - 91;
        let y2 = g.gui_height() - 22;
        if !vars::static_selector() {
            g.enable_scissor(x2 - 1, y2 - 1, x2 + 182 + 1, y2 + 22 + 1);
        } else {
            g.enable_scissor(x2, y2 - 1, x2 + 182, y2 + 22 + 1);
        }
        self.masked = true;
    }

    /// Upstream moveHotbar @WrapOperation on the first blitSprite call in
    /// extractItemHotbar.
    pub fn move_hotbar(&mut self, operation: &mut dyn FnMut()) {
        self.move_whole_hotbar(operation);
    }

    /// Upstream moveItems @WrapOperation on the first extractSlot call in
    /// extractItemHotbar. `x`/`y`/`seed` are the call's arguments.
    pub fn move_items(&mut self, x: i32, y: i32, seed: i32, operation: &mut dyn FnMut()) {
        self.move_whole_hotbar(&mut || {
            extractor().extract_slot(x, y, seed);
            operation();
        });
    }

    /// Upstream moveWholeHotbar @Unique.
    fn move_whole_hotbar(&mut self, draw: &mut dyn FnMut()) {
        let g = extractor();
        if !vars::static_selector() {
            draw();
            return;
        }
        self.enable_mask();
        g.pose_push();
        g.pose_translate(
            -self.smooth_selector_pos + (self.slot_count / 2 * self.slot_width) as f32,
            0.0,
        );

        draw();

        if self.smooth_selector_pos > (self.slot_count / 2 * self.slot_width) as f32 {
            g.pose_translate(((self.slot_count * self.slot_width + self.rollover_space)) as f32, 0.0);
        } else {
            g.pose_translate(-((self.slot_count * self.slot_width + self.rollover_space)) as f32, 0.0);
        }

        draw();

        g.pose_pop();

        if self.masked {
            if vars::enable_mask_debug() {
                g.fill(-100, -100, g.gui_width(), g.gui_height(), argb_color(50, 0, 255, 255));
            }
            g.disable_scissor();
            self.masked = false;
        }
    }

    /// Test visibility for the wrap checks.
    #[cfg(test)]
    pub(crate) fn state(&self) -> (f32, i32, bool) {
        (self.smooth_selector_pos, self.rollover, self.masked)
    }
}

impl HudDuck for HudMixin {
    fn increase_rollover(&mut self) {
        self.rollover += 1;
    }

    fn decrease_rollover(&mut self) {
        self.rollover -= 1;
    }
}
}

pub mod item_picker_menu {
//! Upstream mixin/client/CreativeScreen/ItemPickerMenuMixin.java and
//! ItemPickerMenuAccessor.java.

/// The ItemPickerMenu surface: the shadowed abstract calculateRowCount()
/// (the accessor's @Invoker("calculateRowCount") is the same call).
pub trait ItemPickerMenuHost {
    fn calculate_row_count(&self) -> i32;
}

/// Upstream mixin/client/CreativeScreen/ItemPickerMenuMixin.java.
pub struct ItemPickerMenuMixin;

impl ItemPickerMenuMixin {
    /// Upstream scrollToWrap @WrapMethod: based on mouse position, use items
    /// from a row before or not (upstream keeps the vanilla call).
    pub fn scroll_to_wrap(&self, scroll_offs: f32, operation: &mut dyn FnMut(f32)) {
        operation(scroll_offs);
    }

    /// Upstream getRowIndexForScroll @WrapMethod (the vanilla body replaced).
    pub fn get_row_index_for_scroll(&self, host: &dyn ItemPickerMenuHost, scroll_offs: f32) -> i32 {
        ((scroll_offs * host.calculate_row_count() as f32) as f64 as i32).max(0)
    }
}
}

pub mod miscellaneous {
//! Upstream mixin/client/Miscellaneous package, 1:1.

pub mod sdl_event_handler_mixin {
    //! Upstream SDLEventHandlerMixin.java.
    //!
    //! The @WrapOperation'd SDL_PollEventWrap is dead code upstream (its
    //! annotation is commented out, the method body remains); it is ported 1:1
    //! with the same note. The live members are handleMouseWheelEvent
    //! (@Inject HEAD) and PollEventsTail (@WrapMethod tail).
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::super::super::seams::{self, SdlEvent};
    use super::super::super::smooth_sc::SmoothSc;

    /// Upstream mixin instance fields: lastScrollY, lastScrollYTime, inertia.
    pub struct SdlEventHandlerMixin {
        pub last_scroll_y: f64,
        pub last_scroll_y_time: i64,
        pub inertia: f64,
    }

    impl Default for SdlEventHandlerMixin {
        fn default() -> Self {
            Self { last_scroll_y: 0.0, last_scroll_y_time: 0, inertia: 0.0 }
        }
    }

    impl SdlEventHandlerMixin {
        /// Upstream SDL_PollEventWrap: dead code upstream (the @WrapOperation
        /// annotation is commented out), ported anyway.
        #[allow(dead_code)]
        pub fn sdl_poll_event_wrap(
            &mut self,
            event: &SdlEvent,
            operation: &mut dyn FnMut(&SdlEvent) -> bool,
        ) -> bool {
            let ret = operation(event);
            if event.event_type != 32512 {
                SmoothSc::printt(&[
                    String::from("poll: "),
                    event.event_type.to_string(),
                    event.wheel_which.to_string(),
                ]);
            }
            ret
        }

        /// Upstream handleMouseWheelEvent @Inject(method =
        /// "handleMouseWheelEvent", at = @At("HEAD")).
        pub fn handle_mouse_wheel_event(&mut self, event: &SdlEvent) {
            self.inertia = 0.0;
            // checks, whether it should consider this event touchpad-scroll
            // if the previous event was considered as such and was within
            // touchpadTimeThreshold nanoseconds, it is also considered as
            // touchpad-scroll; if the scroll amount is low enough, it is
            // considered as touchpad-scroll; if the previous event was within
            // touchpadTimeThreshold nanoseconds and is of different absolute
            // amount, it is considered as touchpad-scroll (upstream comment)
            use super::super::super::sm_sc_cfg::vars;
            let y = event.wheel_y;
            let time_delta = (seams::util_nanos() as i64).wrapping_sub(self.last_scroll_y_time);
            if y.abs() < vars::touchpad_threshold()
                || ((time_delta as f32) < vars::touchpad_time_threshold()
                    && (super::super::super::smooth_sc::touchpad_scrolled_y()
                        || (!vars::expect_merged() && self.last_scroll_y.abs() != (y as f64).abs())))
            {
                super::super::super::smooth_sc::set_touchpad_scrolled_y(true);
                if y.abs() > vars::inertia_threshold() {
                    self.inertia = y as f64;
                }
            } else {
                super::super::super::smooth_sc::set_touchpad_scrolled_y(false);
            }
            self.last_scroll_y = y as f64;
            self.last_scroll_y_time = seams::util_nanos() as i64;
        }

        /// Upstream PollEventsTail @WrapMethod(method = "pollEvents"): the
        /// wrapped operation runs first, then the tail. Takes the mixin as an
        /// Rc so the deferred execute lambda reads this.inertia when it runs,
        /// exactly as the upstream closure captures `this`.
        pub fn poll_events_tail(this: &Rc<RefCell<Self>>, operation: &mut dyn FnMut()) {
            operation();
            let time_delta =
                (seams::util_nanos() as i64).wrapping_sub(this.borrow().last_scroll_y_time);
            if time_delta == 0 {
                return;
            }
            if this.borrow().inertia.abs() < super::super::super::sm_sc_cfg::vars::inertia_stop() as f64 {
                return;
            }
            // TODO inertia will be applied every frame, it needs to account for
            // that (upstream comment)
            this.borrow_mut().inertia *= (super::super::super::sm_sc_cfg::vars::inertia_deceleration() as f64)
                .powf(SmoothSc::get_last_frame_duration() as f64);
            // SmoothSc.mc.execute(() -> SmoothSc.mc.mouseHandler.onScroll(
            //     SmoothSc.mc.getWindow().handle(), 0, inertia * getLastFrameDuration()))
            let mixin = Rc::clone(this);
            seams::mouse_host().execute(Box::new(move || {
                let host = seams::mouse_host();
                let window = host.window_handle();
                let frame = SmoothSc::get_last_frame_duration() as f64;
                host.on_scroll(window, 0.0, mixin.borrow().inertia * frame);
            }));
        }
    }
}

pub mod transformation_mixin {
    //! Upstream TransformationMixin.java.
    use super::super::super::seams::ScreenRectangle;
    use super::super::super::transformation_access::Parameters;

    /// Upstream mixin state: the @Shadow fields (scissor, pose, opacity).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct TransformationMixin {
        pub scissor: Option<ScreenRectangle>,
        /// Upstream Matrix3x2fc pose.
        pub pose: [f32; 6],
        pub opacity: f32,
    }

    impl TransformationMixin {
        /// Upstream withScissorWrap @WrapMethod(method = "withScissor").
        /// TODO document why this is needed (upstream comment). `operation` is
        /// the host's original withScissor.
        pub fn with_scissor_wrap(
            &self,
            scissor: Option<ScreenRectangle>,
            operation: &mut dyn FnMut(Option<ScreenRectangle>) -> Parameters,
        ) -> Parameters {
            if scissor.is_none() {
                if self.scissor.is_some() {
                    return Parameters::new(self.pose, self.opacity, None);
                }
                if self.scissor.is_none() {
                    // upstream: return (Parameters) (Object) this
                    return Parameters::new(self.pose, self.opacity, self.scissor);
                }
            }
            operation(scissor)
        }
    }
}

pub mod minecraft_client_mixin {
    //! Upstream MinecraftClientMixin.java.
    //!
    //! Upstream setScreenT (@Inject setScreen TAIL, catching the ItemPickerMenu
    //! and resetting creativeScreenScrollOffset) is commented out; ported as a
    //! note only. The live member is onResReload.

    /// Upstream onResReload @Inject(method = "reloadResourcePacks", at = @At("HEAD")).
    pub fn on_res_reload() {
        super::super::super::smooth_sc::SmoothSc::read_config();
    }
}

pub mod render_system_mixin {
    //! Upstream RenderSystemMixin.java: its only injection
    //! (initBackendSystemTail) is commented out upstream, so the class
    //! contributes no behavior. Kept as an empty module for the 1:1 file map.
}

pub mod render_pass_mixin {
    //! Upstream RenderPassMixin.java.
    use super::super::super::seams::RenderArea;

    /// Upstream @Shadow private @Final RenderArea renderArea.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct RenderPassMixin {
        pub render_area: RenderArea,
    }

    impl RenderPassMixin {
        /// Upstream ignoreArgumentException @WrapMethod(method =
        /// "enableScissor"). The scissor values here are almost always
        /// calculated and have safety margins, so the area can be invalid
        /// (out of bounds); upstream clamps the values so the invalid-argument
        /// exception never occurs instead of clamping its own scissors
        /// everywhere (upstream comment, abridged).
        pub fn ignore_argument_exception(
            &self,
            mut x: i32,
            mut y: i32,
            mut width: i32,
            mut height: i32,
            operation: &mut dyn FnMut(i32, i32, i32, i32),
        ) {
            let ra = &self.render_area;
            x = x.clamp(ra.x, ra.x + ra.width);
            y = y.clamp(ra.y, ra.y + ra.height);
            width = (width + x).clamp(ra.x, ra.x + ra.width) - x;
            height = (height + y).clamp(ra.y, ra.y + ra.height) - y;
            if width == 0 || height == 0 {
                width = 1;
                height = 1;
                x = ra.x;
                y = ra.y;
            }
            operation(x, y, width, height);
        }
    }
}
}

pub mod mouse_handler {
//! Upstream mixin/client/Hotbar/MouseHandlerMixin.java.
//!
//! The mouse handler only drives the hotbar rollover counter; the Hud cast to
//! HudDuck rides the seams.

use super::super::seams::player_inventory;
use super::super::sm_sc_cfg::vars;

/// Upstream onScrollWrap @WrapMethod(method = "onScroll"). `yoffset` is the
/// wrapped call's yoffset parameter; `operation` is the host's onScroll.
pub fn on_scroll_wrap(yoffset: f64, operation: &mut dyn FnMut()) {
    if !player_inventory().player_present() {
        operation();
        return;
    }

    let old_slot = player_inventory().selected_slot();
    let hud_duck = super::super::seams::hud_duck();

    operation();

    if !vars::hotbar_rollover() || vars::hotbar_smoothness() == 0.0 || !player_inventory().player_present() {
        return;
    }

    let new_slot = player_inventory().selected_slot();
    if new_slot == old_slot {
        return;
    }

    // vertical > 0 -> Slot Decreases (Left)
    // vertical < 0 -> Slot Increases (Right)
    if yoffset > 0.0 {
        // Scrolling Left
        if new_slot > old_slot {
            // Wrapped (e.g. 0 -> 8)
            hud_duck.borrow_mut().increase_rollover();
        }
    } else if yoffset < 0.0 {
        // Scrolling Right
        if new_slot < old_slot {
            // Wrapped (e.g. 8 -> 0)
            hud_duck.borrow_mut().decrease_rollover();
        }
    }
}
}

pub mod precise_scissor {
//! Upstream mixin/client/PreciseScissor package, 1:1.

pub mod draw_context_mixin {
    //! Upstream DrawContextMixin.java: both injections (the windowScale
    //! redirect on setScissor and the containsPointInScissor wrap) are
    //! commented out upstream, so the class contributes no behavior. Kept as
    //! an empty module for the 1:1 file map.
}

pub mod screen_rect_mixin {
    //! Upstream ScreenRectMixin.java: its only injection (the windowScale
    //! redirect on transform) is commented out upstream, so the class
    //! contributes no behavior. Kept as an empty module for the 1:1 file map.
}
}

pub mod suggestions_list {
//! Upstream mixin/client/Chat/SuggestionsListMixin.java.

use std::cell::Cell;

use super::super::seams::extractor;
use super::super::seams::ExtractorSeam;
use super::super::sm_sc_cfg::vars;
use super::super::smooth_sc::touchpad_scrolled_y;

/// The SuggestionsList surface the mixin touches (@Shadow fields).
pub trait SuggestionsListHost {
    /// The offset shadow field: scroll position, index of the uppermost line
    /// (up < down).
    fn offset(&self) -> i32;
    fn set_offset(&self, v: i32);
    fn rect_y(&self) -> i32;
    fn rect_height(&self) -> i32;
    fn rect_contains(&self, x: i32, y: i32) -> bool;
}

/// The inject points the host's extractRenderState body drives: the
/// translate inject fires at the first List.get and the unTranslate
/// ModifyVariable fires at each Message STORE.
pub struct TranslateHooks<'a> {
    mixin: &'a SuggestionsListMixin,
    g: &'a dyn ExtractorSeam,
}

impl TranslateHooks<'_> {
    /// Upstream translate @Inject(method = "extractRenderState", at = @At(value
    /// = "INVOKE", target = "Ljava/util/List;get(I)Ljava/lang/Object;", ordinal = 0)).
    pub fn translate(&self, host: &dyn SuggestionsListHost) {
        if self.mixin.translated.get() {
            return;
        }
        self.g.enable_scissor(0, host.rect_y(), self.g.gui_width(), host.rect_y() + host.rect_height());
        self.g.pose_push();
        self.g.pose_translate(0.0, self.mixin.get_draw_offset(host));
        self.mixin.translated.set(true);
    }

    /// Upstream unTranslate @ModifyVariable(method = "extractRenderState",
    /// at = @At(value = "STORE"), ordinal = 0).
    pub fn try_untranslate(&self) {
        self.mixin.try_untranslate(self.g);
    }
}

/// Upstream mixin/client/Chat/SuggestionsListMixin.java.
pub struct SuggestionsListMixin {
    pub(crate) line_height: Cell<i32>,
    pub(crate) smooth_scroll_pos: Cell<f32>,
    pub(crate) target_scroll_pos: Cell<f32>,
    pub(crate) translated: Cell<bool>,
    /// Package-private upstream: `boolean align_pixels = true`.
    pub align_pixels: Cell<bool>,
}

impl Default for SuggestionsListMixin {
    fn default() -> Self {
        Self {
            line_height: Cell::new(12),
            smooth_scroll_pos: Cell::new(0.0),
            target_scroll_pos: Cell::new(0.0),
            translated: Cell::new(false),
            align_pixels: Cell::new(true),
        }
    }
}

impl SuggestionsListMixin {
    /// The wrap head of extractRenderStateWrap: smoothing, half-pixel snap,
    /// offset = floor(smooth).
    pub fn render_head(&self, host: &dyn SuggestionsListHost) {
        self.smooth_scroll_pos.set((self.smooth_scroll_pos.get() - self.target_scroll_pos.get())
            * (vars::suggestion_window_smoothness() as f64)
                .powf(super::super::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
                as f32
            + self.target_scroll_pos.get());

        // snap on less than half a pixel difference
        if (self.smooth_scroll_pos.get() - self.target_scroll_pos.get()).abs() < 1.0 / self.line_height.get() as f32 / 2.0 {
            self.smooth_scroll_pos.set(self.target_scroll_pos.get());
        }

        host.set_offset(self.smooth_scroll_pos.get().floor() as i32);
    }

    /// Upstream extractRenderStateWrap @WrapMethod(method =
    /// "extractRenderState"). The body receives the adjusted mouse coords and
    /// the inject-point hooks.
    pub fn extract_render_state_wrap(
        &self,
        g: &dyn ExtractorSeam,
        host: &dyn SuggestionsListHost,
        mouse_x: i32,
        mouse_y: i32,
        body: &mut dyn FnMut(i32, i32, &TranslateHooks),
    ) {
        self.render_head(host);

        let mx = if host.rect_contains(mouse_x, mouse_y) { mouse_x } else { -1 };
        let my = (mouse_y as f32 - self.get_draw_offset(host)).floor() as i32;

        body(mx, my, &TranslateHooks { mixin: self, g });

        self.try_untranslate(g);
    }

    /// Upstream tryUntranslate @Unique.
    pub fn try_untranslate(&self, g: &dyn ExtractorSeam) {
        if self.translated.get() {
            g.pose_pop();
            if vars::enable_mask_debug() {
                g.fill(-100, -100, g.gui_width(), g.gui_height(), super::super::seams::argb_color(50, 255, 255, 0));
            }
            g.disable_scissor();
        }
        self.translated.set(false);
    }

    /// Upstream mouseScrolledWrap @WrapMethod(method = "mouseScrolled").
    pub fn mouse_scrolled_wrap(
        &self,
        host: &dyn SuggestionsListHost,
        scroll: f64,
        operation: &mut dyn FnMut(f64) -> bool,
    ) -> bool {
        if vars::suggestion_window_amount() == 0.0 {
            return operation(scroll);
        }

        let index_before = host.offset();

        let new_target = self.target_scroll_pos.get() as f64
            - scroll
                * (if vars::suggestion_window_amount() != 0.0 {
                    vars::suggestion_window_amount() / self.line_height.get() as f32
                } else {
                    1.0
                }) as f64;
        host.set_offset(new_target.ceil() as i32);

        // we'll only use the clamp from the call and also the return value
        let ret = operation(0.0);

        if !ret {
            host.set_offset(index_before);
            return ret;
        }

        let mut new_target = new_target;
        if new_target > host.offset() as f64 {
            new_target = host.offset() as f64;
        }
        if new_target < 0.0 {
            new_target = 0.0;
        }

        self.target_scroll_pos.set(new_target as f32);

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos.set(self.target_scroll_pos.get());
        }

        ret
    }

    /// Upstream cycleWrap @WrapMethod(method = "cycle").
    /// this shouldn't touch the scroll amount, it is called when scrolling by
    /// keyboard-selecting
    pub fn cycle_wrap(&self, host: &dyn SuggestionsListHost, off: i32, operation: &mut dyn FnMut(i32)) {
        let index_before = host.offset();
        host.set_offset(self.target_scroll_pos.get().floor() as i32);

        operation(off);

        self.target_scroll_pos.set(host.offset() as f32);
        host.set_offset(index_before);
    }

    /// Upstream mouseClickedWrap @WrapMethod(method = "mouseClicked"). The
    /// mouseClickedRedirectCondition redirect makes the host's own
    /// Rect2i.contains always true, so only this outer check gates the call.
    pub fn mouse_clicked_wrap(
        &self,
        host: &dyn SuggestionsListHost,
        x: i32,
        y: i32,
        operation: &mut dyn FnMut(i32, i32) -> bool,
    ) -> bool {
        if !host.rect_contains(x, y) {
            return false;
        }
        operation(x, y - super::super::seams::java_round_f32(self.get_draw_offset(host)))
    }

    /// Upstream addLineUnder @ModifyVariable(method = "extractRenderState",
    /// at = @At("STORE"), ordinal = 2).
    pub fn add_line_under(&self, host: &dyn SuggestionsListHost, limit: i32) -> i32 {
        if self.get_draw_offset(host) == 0.0 {
            limit
        } else {
            limit + 1
        }
    }

    /// Upstream requestCursorRectCondition @WrapOperation on the
    /// Rect2i.contains call in extractRenderState: the y argument gets the
    /// draw offset folded in.
    pub fn request_cursor_rect_y(&self, host: &dyn SuggestionsListHost, y: i32) -> i32 {
        (y as f32 + self.get_draw_offset(host)).floor() as i32
    }

    /// Upstream getDrawOffset @Unique.
    pub fn get_draw_offset(&self, host: &dyn SuggestionsListHost) -> f32 {
        let ret = (host.offset() as f32 - self.smooth_scroll_pos.get()) * self.line_height.get() as f32;
        if self.align_pixels.get() {
            super::super::seams::java_round_f32(ret) as f32
        } else {
            ret
        }
    }
}
}

}

