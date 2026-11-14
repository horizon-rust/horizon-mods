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
    static HUD_DUCK: RefCell<Option<Rc<std::cell::RefCell<dyn crate::duck::HudDuck>>>> =
        const { RefCell::new(None) };
}

pub fn set_player_inventory(p: Rc<dyn PlayerSeam>) {
    PLAYER_SEAM.with(|c| *c.borrow_mut() = Some(p));
}

pub fn player_inventory() -> Rc<dyn PlayerSeam> {
    PLAYER_SEAM.with(|c| c.borrow().clone()).expect("PlayerSeam not set")
}

pub fn set_hud_duck(h: Rc<std::cell::RefCell<dyn crate::duck::HudDuck>>) {
    HUD_DUCK.with(|c| *c.borrow_mut() = Some(h));
}

pub fn hud_duck() -> Rc<std::cell::RefCell<dyn crate::duck::HudDuck>> {
    HUD_DUCK.with(|c| c.borrow().clone()).expect("HudDuck not set")
}

/// Java Math.round(float): floor(x + 0.5), not Rust's round-half-away.
pub fn java_round_f32(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}
