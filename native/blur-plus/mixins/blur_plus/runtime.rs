// blur+ (Motschen/Blur v6) ported to Horizon as a native Rust mod.
// Upstream: github.com/Motschen/Blur, commit 7f57397, MIT (c) 2017 tterrag.
// 1:1 logic core: config + v3 migration, the ten easing curves, the
// animation state machine (blur radius, background alpha, gradient), the
// HSB/hex color math, the render-pass flow (onRender / renderBackground /
// onRenderEnd), the screen gates from every upstream mixin, the options
// coefficient, and the debug HUD renderer. The drawing itself (strata,
// gradients, text) is a seam: draw specs are returned for the client.

use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Colors (java.awt.Color semantics)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub const fn from_rgba_f(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    // Java int getters round back with +0.5.
    pub fn red(&self) -> i32 {
        (self.r * 255.0 + 0.5) as i32
    }
    pub fn green(&self) -> i32 {
        (self.g * 255.0 + 0.5) as i32
    }
    pub fn blue(&self) -> i32 {
        (self.b * 255.0 + 0.5) as i32
    }
    pub fn alpha(&self) -> i32 {
        (self.a * 255.0 + 0.5) as i32
    }

    /// getRGB(): 0xAARRGGBB
    pub fn rgb(&self) -> i32 {
        (self.alpha() << 24) | (self.red() << 16) | (self.green() << 8) | self.blue()
    }

    pub fn black() -> Color {
        Color::from_rgba8(0, 0, 0, 255)
    }
}

/// java.awt.Color.HSBtoRGB, exact JDK semantics.
pub fn hsb_to_rgb(hue: f32, saturation: f32, brightness: f32) -> i32 {
    let h = (hue - hue.floor()) * 6.0;
    let f = h - h.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - f * saturation);
    let t = brightness * (1.0 - (1.0 - f) * saturation);
    let i = h as i64;
    let (r, g, b) = match i % 6 {
        0 => (brightness, t, p),
        1 => (q, brightness, p),
        2 => (p, brightness, t),
        3 => (p, q, brightness),
        4 => (t, p, brightness),
        _ => (brightness, p, q),
    };
    ((r * 255.0 + 0.5) as i32) << 16 | ((g * 255.0 + 0.5) as i32) << 8 | ((b * 255.0 + 0.5) as i32)
}

pub fn hsb_color(hue: f32, saturation: f32, brightness: f32) -> Color {
    let rgb = hsb_to_rgb(hue, saturation, brightness) as u32;
    Color::from_rgba8(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
        255,
    )
}

/// MidnightColorUtil.hex2Rgb: Color.decode("#" + str minus '#'), Color.BLACK
/// on any parse failure.
pub fn hex2_rgb(color_str: &str) -> Color {
    let cleaned = color_str.replace('#', "");
    match decode_hex(&cleaned) {
        Some(c) => c,
        None => Color::black(),
    }
}

fn decode_hex(hex: &str) -> Option<Color> {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    let val = u32::from_str_radix(digits, 16).ok()?;
    let (a, rgb) = match digits.len() {
        6 => (255u8, val),
        8 => (((val >> 24) & 0xff) as u8, val & 0x00ff_ffff),
        _ => return None,
    };
    Some(Color::from_rgba8(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
        a,
    ))
}

// ---------------------------------------------------------------------------
// Config (eu.midnightdust.blur.config.BlurConfig, minus the config screen UI)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Easing {
    Flat,
    Sine,
    Quad,
    Cubic,
    Quart,
    Quint,
    Expo,
    Circ,
    Back,
    Elastic,
}

impl Easing {
    /// BlurConfig.Easing.apply, verbatim.
    pub fn apply(&self, x: f64) -> f64 {
        match self {
            Easing::Flat => x,
            Easing::Sine => 0.5 - (x * std::f64::consts::PI).cos() / 2.0,
            Easing::Quad => x * x,
            Easing::Cubic => x * x * x,
            Easing::Quart => x * x * x * x,
            Easing::Quint => x * x * x * x * x,
            Easing::Expo => {
                if x == 0.0 {
                    0.0
                } else {
                    2f64.powf(10.0 * x - 10.0)
                }
            }
            Easing::Circ => 1.0 - (1.0 - x.powi(2)).sqrt(),
            Easing::Back => 2.70158 * x * x * x - 1.70158 * x * x,
            Easing::Elastic => {
                if x == 0.0 {
                    0.0
                } else if x == 1.0 {
                    1.0
                } else {
                    -2f64.powf(10.0 * x - 10.0)
                        * ((x * 10.0 - 10.75) * (2.0 * std::f64::consts::PI / 3.0)).sin()
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlurConfig {
    pub config_version: i32,
    // screens
    pub blur_containers: bool,
    pub blur_books: bool,
    pub blur_signs: bool,
    pub blur_command_blocks: bool,
    pub blur_death_screen: bool,
    pub blur_title_screen: bool,
    pub darken_title_screen: bool,
    pub reduce_in_game_blur: bool,
    pub show_screen_id: bool,
    pub force_enabled_screens: Vec<String>,
    pub force_disabled_screens: Vec<String>,
    // style
    pub use_gradient: bool,
    pub gradient_start: String,
    pub gradient_start_alpha: i32,
    pub gradient_end: String,
    pub gradient_end_alpha: i32,
    pub gradient_rotation: i32,
    pub rainbow_mode: bool,
    // animations
    pub fade_time_millis: i32,
    pub fade_out_time_millis: i32,
    pub blur_animation_curve: Easing,
    pub background_animation_curve: Easing,
}

impl Default for BlurConfig {
    fn default() -> Self {
        BlurConfig {
            config_version: 3,
            blur_containers: true,
            blur_books: true,
            blur_signs: true,
            blur_command_blocks: true,
            blur_death_screen: false,
            blur_title_screen: false,
            darken_title_screen: false,
            reduce_in_game_blur: false,
            show_screen_id: false,
            force_enabled_screens: vec![
                "dev.emi.emi.screen.RecipeScreen".to_string(),
                "mezz.jei.gui.recipes.RecipesGui".to_string(),
                "me.shedaniel.rei.impl.client.gui.screen.DefaultDisplayViewingScreen".to_string(),
            ],
            force_disabled_screens: vec![
                "net.irisshaders.iris.gui.screen.ShaderPackScreen".to_string()
            ],
            use_gradient: true,
            gradient_start: "#000000".to_string(),
            gradient_start_alpha: 75,
            gradient_end: "#000000".to_string(),
            gradient_end_alpha: 75,
            gradient_rotation: 0,
            rainbow_mode: false,
            fade_time_millis: 300,
            fade_out_time_millis: 300,
            blur_animation_curve: Easing::Flat,
            background_animation_curve: Easing::Flat,
        }
    }
}

impl BlurConfig {
    /// Blur.init(): the v3 config migration (adds the JEI/REI recipe screens
    /// once, then writes back; persistence is the client's seam).
    pub fn init(&mut self) -> bool {
        if self.config_version < 3 {
            self.force_enabled_screens
                .push("mezz.jei.gui.recipes.RecipesGui".to_string());
            self.force_enabled_screens.push(
                "me.shedaniel.rei.impl.client.gui.screen.DefaultDisplayViewingScreen".to_string(),
            );
            self.config_version = 3;
            return true;
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Animation machinery (animations/*)
// ---------------------------------------------------------------------------

/// org.joml.Math.lerp(a, b, t) = a + (b - a) * t
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub trait AnimationTarget: Copy + PartialEq {
    fn animation_target(&self) -> f32;
    fn animation_time_millis(&self, config: &BlurConfig) -> i32;
}

#[derive(Clone, Copy, Debug)]
pub struct AnimationState<E: Copy> {
    pub time_state: f32,
    pub start_value: f32,
    pub current_value: f32,
    pub target: Option<E>,
}

pub struct AnimationHandler<E: Copy + PartialEq + AnimationTarget> {
    state: AnimationState<E>,
    new_target: Option<E>,
}

impl<E: Copy + PartialEq + AnimationTarget> AnimationHandler<E> {
    pub fn new() -> Self {
        AnimationHandler {
            state: AnimationState {
                time_state: 0.0,
                start_value: 0.0,
                current_value: 0.0,
                target: None,
            },
            new_target: None,
        }
    }

    pub fn time_state(&self) -> f32 {
        self.state.time_state
    }
    pub fn current_value(&self) -> f32 {
        self.state.current_value
    }
    pub fn target(&self) -> Option<E> {
        self.state.target
    }

    fn animation_time_millis(&self, config: &BlurConfig) -> i32 {
        match self.target() {
            Some(t) => t.animation_time_millis(config),
            None => config.fade_time_millis,
        }
    }

    pub fn step_animation(
        &self,
        delta_seconds: f32,
        easing: Easing,
        config: &BlurConfig,
    ) -> AnimationState<E> {
        let new_time_state = (self.time_state()
            + (1000.0 * delta_seconds / self.animation_time_millis(config) as f32))
            .clamp(0.0, 1.0);
        let target_value = self.target().map(|t| t.animation_target()).unwrap_or(0.0);
        let new_value = lerp(
            self.state.start_value,
            target_value,
            easing.apply(new_time_state as f64) as f32,
        );
        AnimationState {
            time_state: new_time_state,
            start_value: self.state.start_value,
            current_value: new_value,
            target: self.state.target,
        }
    }

    pub fn update_animation(&mut self, delta_seconds: f32, easing: Easing, config: &BlurConfig) {
        if self.animation_time_millis(config) == 0 || delta_seconds <= 0.0 {
            return;
        }
        if let Some(t) = self.new_target.take() {
            self.reset_target(Some(t));
        }
        if self.target().is_none() {
            return;
        }
        let next = self.step_animation(delta_seconds, easing, config);
        self.state = next;
    }

    pub fn set_target(&mut self, target: E) {
        if self.new_target == Some(target) {
            return;
        }
        if self.target() == Some(target) {
            self.new_target = None;
        } else {
            self.new_target = Some(target);
        }
    }

    pub fn reset_target(&mut self, target: Option<E>) {
        let current = self.current_value();
        self.state = AnimationState {
            time_state: 0.0,
            start_value: current,
            current_value: current,
            target,
        };
    }

    // Accessors the GradientAnimationHandler override needs (upstream calls
    // super.stepAnimation from its overridden stepAnimation).
    pub fn take_new_target(&mut self) -> Option<E> {
        self.new_target.take()
    }
    pub fn set_state(&mut self, s: AnimationState<E>) {
        self.state = s;
    }
}

impl<E: Copy + PartialEq + AnimationTarget> Default for AnimationHandler<E> {
    fn default() -> Self {
        Self::new()
    }
}

// The upstream static Blur.reducedBlur, shared by the radius target and the
// screen gates.
static REDUCED_BLUR: AtomicBool = AtomicBool::new(false);

pub fn set_reduced_blur(v: bool) {
    REDUCED_BLUR.store(v, Ordering::Relaxed);
}
pub fn reduced_blur() -> bool {
    REDUCED_BLUR.load(Ordering::Relaxed)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlurRadiusTarget {
    FadeIn,
    FadeOut,
}

impl AnimationTarget for BlurRadiusTarget {
    fn animation_target(&self) -> f32 {
        let target = match self {
            BlurRadiusTarget::FadeIn => 1.0,
            BlurRadiusTarget::FadeOut => 0.0,
        };
        if reduced_blur() {
            return target * 0.5;
        }
        target
    }
    fn animation_time_millis(&self, config: &BlurConfig) -> i32 {
        if *self == BlurRadiusTarget::FadeIn {
            config.fade_time_millis
        } else {
            config.fade_out_time_millis
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackgroundAlphaTarget {
    FadeIn,
    FadeOut,
}

impl AnimationTarget for BackgroundAlphaTarget {
    fn animation_target(&self) -> f32 {
        match self {
            BackgroundAlphaTarget::FadeIn => 1.0,
            BackgroundAlphaTarget::FadeOut => 0.0,
        }
    }
    fn animation_time_millis(&self, config: &BlurConfig) -> i32 {
        if *self == BackgroundAlphaTarget::FadeIn {
            config.fade_time_millis
        } else {
            config.fade_out_time_millis
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GradientTarget {
    Fixed,
    Rainbow,
}

impl AnimationTarget for GradientTarget {
    fn animation_target(&self) -> f32 {
        match self {
            GradientTarget::Fixed => 0.0,
            GradientTarget::Rainbow => 1.0,
        }
    }
    fn animation_time_millis(&self, config: &BlurConfig) -> i32 {
        if *self == GradientTarget::Rainbow {
            config.fade_time_millis
        } else {
            config.fade_out_time_millis
        }
    }
}

/// BlurRadiusAnimationHandler: resets the animation onto the current value
/// whenever the global reducedBlur flag flips (upstream captures
/// prevReducedBlur at construction, false by default).
pub struct BlurRadiusAnimationHandler {
    pub anim: AnimationHandler<BlurRadiusTarget>,
    prev_reduced_blur: bool,
}

impl BlurRadiusAnimationHandler {
    pub fn new() -> Self {
        BlurRadiusAnimationHandler {
            anim: AnimationHandler::new(),
            prev_reduced_blur: reduced_blur(),
        }
    }

    pub fn update_animation(&mut self, delta_seconds: f32, easing: Easing, config: &BlurConfig) {
        if reduced_blur() != self.prev_reduced_blur {
            let t = self.anim.target();
            self.anim.reset_target(t);
            self.prev_reduced_blur = reduced_blur();
        }
        self.anim.update_animation(delta_seconds, easing, config);
    }

    pub fn current_value(&self) -> f32 {
        self.anim.current_value()
    }
    pub fn set_target(&mut self, t: BlurRadiusTarget) {
        self.anim.set_target(t)
    }
    pub fn target(&self) -> Option<BlurRadiusTarget> {
        self.anim.target()
    }
}

impl Default for BlurRadiusAnimationHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// GradientAnimationHandler: carries the rainbow drift state upstream kept
/// in statics (rotation, hue1, hue2), with its stepAnimation override.
pub struct GradientAnimationHandler {
    pub anim: AnimationHandler<GradientTarget>,
    rotation: f32,
    hue1: f32,
    hue2: f32,
}

impl GradientAnimationHandler {
    pub fn new() -> Self {
        GradientAnimationHandler {
            anim: AnimationHandler::new(),
            rotation: 0.0, // (float) BlurConfig.gradientRotation, default 0
            hue1: 0.0,
            hue2: 0.35,
        }
    }

    pub fn step_animation(&mut self, delta_seconds: f32, easing: Easing, config: &BlurConfig) {
        if self.anim.target() == Some(GradientTarget::Rainbow) {
            // 20 degrees of rotation per second
            self.rotation = (self.rotation + delta_seconds * 20.0) % 360.0;
            // 0.2 of the hue wheel per second
            self.hue1 = (self.hue1 + delta_seconds * 0.2) % 1.0;
            self.hue2 = (self.hue2 + delta_seconds * 0.2) % 1.0;
        }
        let next = self.anim.step_animation(delta_seconds, easing, config);
        self.anim.set_state(next);
    }

    pub fn update_animation(&mut self, delta_seconds: f32, easing: Easing, config: &BlurConfig) {
        let millis = self
            .anim
            .target()
            .map(|t| t.animation_time_millis(config))
            .unwrap_or(config.fade_time_millis);
        if millis == 0 || delta_seconds <= 0.0 {
            return;
        }
        if let Some(t) = self.anim.take_new_target() {
            self.anim.reset_target(Some(t));
        }
        if self.anim.target().is_none() {
            return;
        }
        self.step_animation(delta_seconds, easing, config);
    }

    pub fn current_value(&self) -> f32 {
        self.anim.current_value()
    }
    pub fn set_target(&mut self, target: GradientTarget) {
        self.anim.set_target(target)
    }
    pub fn target(&self) -> Option<GradientTarget> {
        self.anim.target()
    }

    pub fn first_color(&self, config: &BlurConfig) -> Color {
        let rainbow = hsb_color(self.hue1, 1.0, 1.0);
        let fixed = hex2_rgb(&config.gradient_start);
        let cur = self.current_value();
        Color::from_rgba_f(
            rainbow.r * cur + fixed.r * (1.0 - cur),
            rainbow.g * cur + fixed.g * (1.0 - cur),
            rainbow.b * cur + fixed.b * (1.0 - cur),
            config.gradient_start_alpha as f32 / 255.0,
        )
    }

    pub fn second_color(&self, config: &BlurConfig) -> Color {
        let rainbow = hsb_color(self.hue2, 1.0, 1.0);
        let fixed = hex2_rgb(&config.gradient_end);
        let cur = self.current_value();
        Color::from_rgba_f(
            rainbow.r * cur + fixed.r * (1.0 - cur),
            rainbow.g * cur + fixed.g * (1.0 - cur),
            rainbow.b * cur + fixed.b * (1.0 - cur),
            config.gradient_end_alpha as f32 / 255.0,
        )
    }

    pub fn rotation(&self, config: &BlurConfig) -> f32 {
        self.rotation * self.current_value()
            + config.gradient_rotation as f32 * (1.0 - self.current_value())
    }
}

impl Default for GradientAnimationHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// The core flow (Blur.java)
// ---------------------------------------------------------------------------

/// What the client should draw for renderRotatedGradient: the rotated,
/// scaled fill-gradient rect (the engine draws it inside its pose stack).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientDraw {
    pub rotation_radians: f32,
    pub scale: f32,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub first_color: i32,
    pub second_color: i32,
}

/// What the client should draw for DebugHudRenderer.renderLine.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugLineDraw {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub bg_color: i32,
    pub text_x: i32,
    pub text_y: i32,
    pub text_color: i32,
    pub text: String,
}

pub struct Blur {
    pub config: BlurConfig,
    pub blur_radius_animation: BlurRadiusAnimationHandler,
    pub background_alpha_animation: AnimationHandler<BackgroundAlphaTarget>,
    pub gradient_animation: GradientAnimationHandler,
    pub is_processing_render_pass: bool,
    pub force_rendered_background: bool,
    // Stand-ins for the client state upstream reads off Minecraft.getInstance().
    pub current_screen_name: Option<String>,
    pub in_level: bool,
    pub gui_width: i32,
    pub gui_height: i32,
}

impl Blur {
    pub const DEBUG_DEFAULT_COLOR: i32 = 0x90E0E0E0u32 as i32;
    pub const DEBUG_BACKGROUND_ALPHA: i32 = 0x90;
    pub const DEBUG_LINE_HEIGHT: i32 = 9;
    pub const MENU_BLURRINESS_MAX: i32 = 20;

    pub fn new(config: BlurConfig) -> Blur {
        Blur {
            config,
            blur_radius_animation: BlurRadiusAnimationHandler::new(),
            background_alpha_animation: AnimationHandler::new(),
            gradient_animation: GradientAnimationHandler::new(),
            is_processing_render_pass: false,
            force_rendered_background: false,
            current_screen_name: None,
            in_level: false,
            gui_width: 0,
            gui_height: 0,
        }
    }

    /// canBlur(): true when the render state has no stratum queued after the
    /// blur boundary (Integer.MAX_VALUE sentinel).
    pub fn can_blur(first_stratum_after_blur: i32) -> bool {
        first_stratum_after_blur == i32::MAX
    }

    /// onRender(), start of a render pass: nothing knows yet whether the
    /// screen has a background, so fade everything out.
    pub fn on_render(&mut self) {
        if !self.is_processing_render_pass {
            self.is_processing_render_pass = true;
            self.blur_radius_animation
                .set_target(BlurRadiusTarget::FadeOut);
            self.background_alpha_animation
                .set_target(BackgroundAlphaTarget::FadeOut);
            self.force_rendered_background = false;
            set_reduced_blur(false);
        }
        // else: upstream logs "called multiple times in one render pass".
    }

    /// renderBlurredBackground(): true when the engine should call
    /// blurBeforeThisStratum (the < 0.001 skip and the canBlur gate are the
    /// logic; the blur itself is the seam).
    pub fn render_blurred_background(&self, first_stratum_after_blur: i32) -> bool {
        if self.blur_radius_animation.current_value() < 0.001 {
            return false; // there's no blur to apply
        }
        Blur::can_blur(first_stratum_after_blur)
    }

    /// renderBackground(): nextStratum() first, then blur + gradient exactly
    /// once per render pass. Returns (next_stratum, blur, gradient).
    pub fn render_background(&mut self, first_stratum_after_blur: i32) -> (bool, bool, bool) {
        let next_stratum = true;
        if !self.force_rendered_background {
            let blur = self.render_blurred_background(first_stratum_after_blur);
            let gradient = self.render_rotated_gradient().is_some();
            self.force_rendered_background = true;
            (next_stratum, blur, gradient)
        } else {
            // upstream logs "renderBackground has been called multiple times"
            (next_stratum, false, false)
        }
    }

    pub fn get_background_gradient_color(&self, second: bool) -> i32 {
        let color = if second {
            self.gradient_animation.second_color(&self.config)
        } else {
            self.gradient_animation.first_color(&self.config)
        };
        let alpha = (self.background_alpha_animation.current_value() * color.alpha() as f32) as i32;
        (alpha << 24) | (color.red() << 16) | (color.green() << 8) | color.blue()
    }

    pub fn get_background_gradient_rotation(&self) -> i32 {
        self.gradient_animation.rotation(&self.config) as i32
    }

    /// renderRotatedGradient(): the math behind the rotated gradient; None
    /// when there's nothing to draw.
    pub fn render_rotated_gradient(&self) -> Option<GradientDraw> {
        if !self.config.use_gradient || self.background_alpha_animation.current_value() < 0.001 {
            return None; // there's no gradient to draw
        }
        let (width, height) = (self.gui_width, self.gui_height);
        let diagonal = (width as f32 * width as f32 + height as f32 * height as f32).sqrt();
        let smallest_dimension = width.min(height) as f32;
        let rotation = (self.get_background_gradient_rotation() as f32).to_radians();
        let first_color = self.get_background_gradient_color(false);
        let second_color = self.get_background_gradient_color(true);
        Some(GradientDraw {
            rotation_radians: rotation,
            scale: diagonal / smallest_dimension,
            x1: -width / 2,
            y1: -height / 2,
            x2: width / 2,
            y2: height / 2,
            first_color,
            second_color,
        })
    }

    pub fn update_animations(&mut self, realtime_delta_ticks: f32) {
        let delta_seconds = realtime_delta_ticks / 20.0;
        self.blur_radius_animation.update_animation(
            delta_seconds,
            self.config.blur_animation_curve,
            &self.config,
        );
        self.background_alpha_animation.update_animation(
            delta_seconds,
            self.config.background_animation_curve,
            &self.config,
        );
        self.gradient_animation.update_animation(
            delta_seconds,
            self.config.background_animation_curve,
            &self.config,
        );
    }

    /// onRenderEnd(), end of a render pass: the screen has now told us
    /// whether it had a background, so resolve the fade targets.
    pub fn on_render_end(&mut self, realtime_delta_ticks: f32) {
        if self.is_processing_render_pass {
            let screen_name = self.current_screen_name.clone();

            // force a background fade-in for forceEnabledScreens
            if let Some(name) = &screen_name {
                if self.config.force_enabled_screens.contains(name) {
                    self.blur_radius_animation
                        .set_target(BlurRadiusTarget::FadeIn);
                    self.background_alpha_animation
                        .set_target(BackgroundAlphaTarget::FadeIn);
                }
            }

            // force a background fade-out for forceDisabledScreens
            if let Some(name) = &screen_name {
                if self.config.force_disabled_screens.contains(name) {
                    self.blur_radius_animation
                        .set_target(BlurRadiusTarget::FadeOut);
                    self.background_alpha_animation
                        .set_target(BackgroundAlphaTarget::FadeOut);
                }
            }

            // update gradientAnimation target from config
            self.gradient_animation
                .set_target(if self.config.rainbow_mode {
                    GradientTarget::Rainbow
                } else {
                    GradientTarget::Fixed
                });

            self.update_animations(realtime_delta_ticks);

            self.is_processing_render_pass = false;
        }
        // else: upstream logs "onRenderEnd has been called multiple times".
    }

    // ----- screen gates (upstream mixins) -----

    /// MixinScreen.blur$onRenderBlurredBackground: the screen drew its
    /// blurred background, so fade the blur in.
    pub fn on_blurred_background_extracted(&mut self) {
        self.blur_radius_animation
            .set_target(BlurRadiusTarget::FadeIn);
    }

    /// MixinScreen.blur$onRenderBackground: the screen drew any background,
    /// so fade the background alpha in; forceEnabledScreens (minus
    /// forceDisabledScreens) also get their blurred background drawn.
    pub fn on_screen_background(
        &mut self,
        screen_name: &str,
        first_stratum_after_blur: i32,
    ) -> bool {
        self.background_alpha_animation
            .set_target(BackgroundAlphaTarget::FadeIn);
        self.config
            .force_enabled_screens
            .contains(&screen_name.to_string())
            && !self
                .config
                .force_disabled_screens
                .contains(&screen_name.to_string())
            && Blur::can_blur(first_stratum_after_blur)
    }

    /// MixinScreen menu-background wrap: useGradient swaps the vanilla menu
    /// background texture for our gradient.
    pub fn menu_background_uses_gradient(&self) -> bool {
        self.config.use_gradient
    }

    /// MixinGui.blur$beforeRenderScreen1: in a level with no screen open,
    /// draw our background.
    pub fn before_render_screen_in_level(
        &mut self,
        first_stratum_after_blur: i32,
    ) -> (bool, bool, bool) {
        if self.current_screen_name.is_none() && self.in_level {
            self.render_background(first_stratum_after_blur)
        } else {
            (false, false, false)
        }
    }

    /// MixinAbstractContainerScreen: blur gate + isInGameUi override.
    pub fn container_gate(
        &mut self,
        first_stratum_after_blur: i32,
        original_is_in_game_ui: bool,
    ) -> (bool, bool) {
        if self.config.reduce_in_game_blur {
            set_reduced_blur(true);
        }
        let render_blurred =
            self.config.blur_containers && Blur::can_blur(first_stratum_after_blur);
        let in_game_ui = if self.config.blur_containers {
            false
        } else {
            original_is_in_game_ui
        };
        (render_blurred, in_game_ui)
    }

    /// BookViewScreen / BookEditScreen / BookSignScreen: blurBooks gate.
    pub fn book_gate(&mut self, first_stratum_after_blur: i32) -> bool {
        if self.config.reduce_in_game_blur {
            set_reduced_blur(true);
        }
        self.config.blur_books && Blur::can_blur(first_stratum_after_blur)
    }

    /// AbstractSignEditScreen: blurSigns gate.
    pub fn sign_gate(&mut self, first_stratum_after_blur: i32) -> bool {
        if self.config.reduce_in_game_blur {
            set_reduced_blur(true);
        }
        self.config.blur_signs && Blur::can_blur(first_stratum_after_blur)
    }

    /// AbstractCommandBlockEditScreen: blurCommandBlocks gate.
    pub fn command_block_gate(&mut self, first_stratum_after_blur: i32) -> bool {
        if self.config.reduce_in_game_blur {
            set_reduced_blur(true);
        }
        self.config.blur_command_blocks && Blur::can_blur(first_stratum_after_blur)
    }

    /// DeathScreen: blurDeathScreen gate.
    pub fn death_gate(&mut self, first_stratum_after_blur: i32) -> bool {
        if self.config.reduce_in_game_blur {
            set_reduced_blur(true);
        }
        self.config.blur_death_screen && Blur::can_blur(first_stratum_after_blur)
    }

    /// MixinTitleScreen.extractBackground: HEAD blur, then the radius target
    /// follows blurTitleScreen, then the gradient replaces the menu
    /// background when useGradient, then the alpha target follows
    /// darkenTitleScreen.
    pub fn title_gate(&mut self, first_stratum_after_blur: i32) -> (bool, bool, bool) {
        let blur = Blur::can_blur(first_stratum_after_blur);
        self.blur_radius_animation
            .set_target(if self.config.blur_title_screen {
                BlurRadiusTarget::FadeIn
            } else {
                BlurRadiusTarget::FadeOut
            });
        let menu_bg_gradient = self.config.use_gradient;
        self.background_alpha_animation
            .set_target(if self.config.darken_title_screen {
                BackgroundAlphaTarget::FadeIn
            } else {
                BackgroundAlphaTarget::FadeOut
            });
        (blur, menu_bg_gradient, true)
    }

    /// MixinOptions.blur$applyMenuBackgroundBlurCoefficient: the vanilla
    /// menu blurriness setting is scaled by the radius animation value.
    pub fn apply_menu_background_blur_coefficient(&self, original: i32) -> i32 {
        (original as f32 * self.blur_radius_animation.current_value()) as i32
    }

    /// MixinScreen.blur$renderDebugHud: showScreenID draws the screen's
    /// class name in the top-left corner.
    pub fn debug_line_for_screen(&self, text_width: i32) -> Option<DebugLineDraw> {
        if self.config.show_screen_id {
            if let Some(name) = &self.current_screen_name {
                return Blur::render_debug_line(
                    name,
                    self.gui_width,
                    2,
                    text_width,
                    Some(0xff80dfffu32 as i32),
                    true,
                );
            }
        }
        None
    }

    // ----- debug HUD (util/DebugHudRenderer.java) -----

    pub fn render_debug_line(
        text: &str,
        gui_width: i32,
        line_y: i32,
        text_width: i32,
        color: Option<i32>,
        left: bool,
    ) -> Option<DebugLineDraw> {
        if text.is_empty() {
            return None;
        }
        let line_x = if left { 2 } else { gui_width - 2 - text_width };
        let line_color = color.unwrap_or(Blur::DEBUG_DEFAULT_COLOR);
        let r = (((line_color >> 16) & 0xff) as f32 * 0.25) as i32;
        let g = (((line_color >> 8) & 0xff) as f32 * 0.25) as i32;
        let b = ((line_color & 0xff) as f32 * 0.25) as i32;
        let bg_color = (Blur::DEBUG_BACKGROUND_ALPHA << 24) | (r << 16) | (g << 8) | b;
        Some(DebugLineDraw {
            x1: line_x - 1,
            y1: line_y - 1,
            x2: line_x + text_width + 1,
            y2: line_y + Blur::DEBUG_LINE_HEIGHT - 1,
            bg_color,
            text_x: line_x,
            text_y: line_y,
            text_color: line_color,
            text: text.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests share the global REDUCED_BLUR flag; run single-threaded.
    fn reset_globals() {
        set_reduced_blur(false);
    }

    fn plain_config() -> BlurConfig {
        let mut c = BlurConfig::default();
        c.gradient_start = "#FF0000".to_string();
        c.gradient_end = "#0000FF".to_string();
        c
    }

    fn drive_fade_in(b: &mut Blur, seconds_per_step: f32, steps: usize) {
        for _ in 0..steps {
            b.update_animations(seconds_per_step * 20.0);
        }
    }

    #[test]
    fn easing_curves_match_upstream() {
        assert_eq!(Easing::Flat.apply(0.5), 0.5);
        assert!((Easing::Sine.apply(0.5) - 0.5).abs() < 1e-9);
        assert_eq!(Easing::Quad.apply(0.5), 0.25);
        assert_eq!(Easing::Cubic.apply(0.5), 0.125);
        assert_eq!(Easing::Quart.apply(0.5), 0.0625);
        assert_eq!(Easing::Quint.apply(0.5), 0.03125);
        assert_eq!(Easing::Expo.apply(0.0), 0.0);
        assert!((Easing::Expo.apply(1.0) - 1.0).abs() < 1e-9);
        assert!((Easing::Circ.apply(1.0) - 1.0).abs() < 1e-9);
        assert!((Easing::Circ.apply(0.5) - (1.0 - (1.0 - 0.25f64).sqrt())).abs() < 1e-9);
        assert!((Easing::Back.apply(0.5) - (-0.0876975)).abs() < 1e-9);
        assert_eq!(Easing::Elastic.apply(0.0), 0.0);
        assert_eq!(Easing::Elastic.apply(1.0), 1.0);
        // -2^-5 * sin(-12.042772) = -0.03125 * 0.5
        assert!((Easing::Elastic.apply(0.5) - (-0.015625)).abs() < 1e-6);
    }

    #[test]
    fn hsb_primary_colors() {
        assert_eq!(hsb_to_rgb(0.0, 1.0, 1.0), 0xff0000);
        assert_eq!(hsb_to_rgb(1.0 / 3.0, 1.0, 1.0), 0x00ff00);
        assert_eq!(hsb_to_rgb(2.0 / 3.0, 1.0, 1.0), 0x0000ff);
        // hue wraps at 1.0
        assert_eq!(hsb_to_rgb(1.0, 1.0, 1.0), 0xff0000);
    }

    #[test]
    fn hex2_rgb_matches_midnightlib() {
        assert_eq!(hex2_rgb("#000000"), Color::black());
        let red = hex2_rgb("FF0000");
        assert_eq!(
            (red.red(), red.green(), red.blue(), red.alpha()),
            (255, 0, 0, 255)
        );
        let translucent = hex2_rgb("#80FF0000");
        assert_eq!(translucent.alpha(), 128);
        assert_eq!(translucent.red(), 255);
        // garbage parses to black like Color.decode's fallback
        assert_eq!(hex2_rgb("not-a-color"), Color::black());
    }

    #[test]
    fn config_defaults_match_upstream() {
        let c = BlurConfig::default();
        assert_eq!(c.config_version, 3);
        assert!(c.blur_containers && c.blur_books && c.blur_signs && c.blur_command_blocks);
        assert!(!c.blur_death_screen && !c.blur_title_screen && !c.darken_title_screen);
        assert!(c.use_gradient);
        assert_eq!(c.gradient_start, "#000000");
        assert_eq!(c.gradient_start_alpha, 75);
        assert_eq!(c.gradient_rotation, 0);
        assert!(!c.rainbow_mode);
        assert_eq!(c.fade_time_millis, 300);
        assert_eq!(c.fade_out_time_millis, 300);
        assert_eq!(c.blur_animation_curve, Easing::Flat);
        assert_eq!(c.force_enabled_screens.len(), 3);
        assert_eq!(c.force_disabled_screens.len(), 1);
    }

    #[test]
    fn v3_config_migration() {
        let mut c = plain_config();
        assert!(!c.init());

        let mut old = plain_config();
        old.config_version = 2;
        assert!(old.init());
        assert_eq!(old.config_version, 3);
        assert_eq!(old.force_enabled_screens.len(), 5);
        assert!(old
            .force_enabled_screens
            .contains(&"mezz.jei.gui.recipes.RecipesGui".to_string()));
        assert!(old.force_enabled_screens.contains(
            &"me.shedaniel.rei.impl.client.gui.screen.DefaultDisplayViewingScreen".to_string()
        ));
        assert!(!old.init()); // runs once
    }

    #[test]
    fn fade_in_advances_linearly_with_flat_easing() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        // 300ms fade, 0.1s steps: time_state 1/3 per step
        b.update_animations(0.1 * 20.0);
        assert!((b.blur_radius_animation.current_value() - 1.0 / 3.0).abs() < 1e-4);
        assert!((b.blur_radius_animation.anim.time_state() - 1.0 / 3.0).abs() < 1e-4);
        b.update_animations(0.1 * 20.0);
        b.update_animations(0.1 * 20.0);
        assert!((b.blur_radius_animation.current_value() - 1.0).abs() < 1e-3);
        // time state clamps at 1
        b.update_animations(0.1 * 20.0);
        assert_eq!(b.blur_radius_animation.anim.time_state(), 1.0);
    }

    #[test]
    fn fade_out_uses_its_own_time() {
        reset_globals();
        let mut c = plain_config();
        c.fade_out_time_millis = 100;
        let mut b = Blur::new(c);
        b.blur_radius_animation
            .set_target(BlurRadiusTarget::FadeOut);
        b.update_animations(0.1 * 20.0); // 1000*0.1/100 = 1.0
        assert_eq!(b.blur_radius_animation.anim.time_state(), 1.0);
        assert!((b.blur_radius_animation.current_value() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn no_target_means_no_animation() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.update_animations(20.0); // 1 full second
        assert_eq!(b.blur_radius_animation.current_value(), 0.0);
        assert_eq!(b.blur_radius_animation.anim.time_state(), 0.0);
        // zero/negative delta is ignored even with a target
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        b.update_animations(0.0);
        b.update_animations(-1.0);
        assert_eq!(b.blur_radius_animation.anim.time_state(), 0.0);
    }

    #[test]
    fn set_target_dedupes_like_upstream() {
        reset_globals();
        let mut h: AnimationHandler<BlurRadiusTarget> = AnimationHandler::new();
        h.set_target(BlurRadiusTarget::FadeIn);
        assert_eq!(h.target(), None); // still pending
        h.set_target(BlurRadiusTarget::FadeIn); // same pending target: no-op
        h.update_animation(0.1, Easing::Flat, &BlurConfig::default());
        assert_eq!(h.target(), Some(BlurRadiusTarget::FadeIn));
        // target == current target clears the pending new target
        h.set_target(BlurRadiusTarget::FadeIn);
        h.update_animation(0.1, Easing::Flat, &BlurConfig::default());
        let before = h.current_value();
        // switching targets resets onto the current value
        h.set_target(BlurRadiusTarget::FadeOut);
        assert_eq!(h.target(), Some(BlurRadiusTarget::FadeIn)); // pending swap
        h.update_animation(0.001, Easing::Flat, &BlurConfig::default()); // applies the reset, tiny step
        assert_eq!(h.target(), Some(BlurRadiusTarget::FadeOut));
        assert!((h.current_value() - before).abs() < 0.02);
        assert!(h.time_state() < 0.01); // the timer restarted at 0
    }

    #[test]
    fn reduced_blur_halves_the_radius_target() {
        reset_globals();
        set_reduced_blur(true);
        let mut b = Blur::new(plain_config());
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        b.update_animations(0.1 * 20.0);
        // target is 0.5, t = 1/3
        assert!((b.blur_radius_animation.current_value() - 1.0 / 6.0).abs() < 1e-4);
        set_reduced_blur(false);
    }

    #[test]
    fn radius_handler_resets_when_reduced_blur_flips() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        drive_fade_in(&mut b, 0.09, 3); // t = 0.9 after three 1/3 steps
        let cur = b.blur_radius_animation.current_value();
        assert!((cur - 0.9).abs() < 1e-3);
        set_reduced_blur(true); // target becomes 0.5
        b.update_animations(0.1 * 20.0);
        // reset onto 0.9 then lerp toward 0.5 by t=1/3
        let expected = 0.9 + (0.5 - 0.9) / 3.0;
        assert!((b.blur_radius_animation.current_value() - expected).abs() < 1e-3);
        set_reduced_blur(false);
    }

    #[test]
    fn gradient_blends_fixed_and_rainbow_by_current_value() {
        reset_globals();
        let mut c = plain_config();
        c.gradient_start = "#FF0000".to_string();
        c.gradient_end = "#0000FF".to_string();
        let mut b = Blur::new(c);
        // cur = 0: pure fixed colors
        let first = b.gradient_animation.first_color(&b.config);
        assert_eq!((first.red(), first.green(), first.blue()), (255, 0, 0));
        let second = b.gradient_animation.second_color(&b.config);
        assert_eq!((second.red(), second.green(), second.blue()), (0, 0, 255));
        // cur = 1: pure rainbow. The hue drifts 0.2/s while Rainbow is the
        // target, so derive the expected hue from the elapsed time.
        b.gradient_animation.set_target(GradientTarget::Rainbow);
        for _ in 0..4 {
            b.gradient_animation
                .update_animation(0.1, Easing::Flat, &b.config);
        } // 0.4s total: t clamps to 1
        assert!((b.gradient_animation.current_value() - 1.0).abs() < 1e-4);
        let first = b.gradient_animation.first_color(&b.config);
        assert_eq!(first.red(), 255);
        let second = b.gradient_animation.second_color(&b.config);
        let expected = hsb_to_rgb(0.35 + 0.4 * 0.2, 1.0, 1.0) as u32;
        assert!((second.red() - (((expected >> 16) & 0xff) as i32)).abs() <= 1);
        assert!((second.green() - (((expected >> 8) & 0xff) as i32)).abs() <= 1);
        assert!((second.blue() - ((expected & 0xff) as i32)).abs() <= 1);
    }

    #[test]
    fn rainbow_drift_advances_rotation_and_hues() {
        reset_globals();
        let mut c = plain_config();
        c.gradient_rotation = 90;
        let mut b = Blur::new(c);
        b.gradient_animation.set_target(GradientTarget::Rainbow);
        b.gradient_animation
            .update_animation(0.1, Easing::Flat, &b.config);
        let cur = b.gradient_animation.current_value(); // 1/3
                                                        // rotation drifted 0.1s * 20 deg/s = 2 deg, blended with config's 90
        let rot = b.gradient_animation.rotation(&b.config);
        assert!((rot - (2.0 * cur + 90.0 * (1.0 - cur))).abs() < 1e-4);
        assert_eq!(b.get_background_gradient_rotation(), 60); // 60.67 truncates
    }

    #[test]
    fn packed_gradient_color_follows_alpha_animation() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        // alpha animation at 0 packs an invisible color
        assert_eq!(b.get_background_gradient_color(false), 0x00FF0000);
        // fade the background alpha all the way in
        b.background_alpha_animation
            .set_target(BackgroundAlphaTarget::FadeIn);
        drive_fade_in(&mut b, 0.1, 4);
        let packed = b.get_background_gradient_color(false) as u32;
        assert_eq!(packed, 0x4BFF0000); // 75/255 alpha channel
    }

    #[test]
    fn can_blur_uses_the_max_sentinel() {
        assert!(Blur::can_blur(i32::MAX));
        assert!(!Blur::can_blur(0));
        assert!(!Blur::can_blur(5));
    }

    #[test]
    fn render_blurred_background_gates() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        // no blur radius yet: nothing to apply
        assert!(!b.render_blurred_background(i32::MAX));
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        drive_fade_in(&mut b, 0.1, 4);
        assert!(b.render_blurred_background(i32::MAX));
        assert!(!b.render_blurred_background(5));
    }

    #[test]
    fn render_background_draws_once_per_pass() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.background_alpha_animation
            .set_target(BackgroundAlphaTarget::FadeIn);
        drive_fade_in(&mut b, 0.1, 4);
        let (next, blur, gradient) = b.render_background(i32::MAX);
        assert!(next);
        assert!(!blur);
        assert!(gradient); // useGradient on, alpha faded in
        assert!(b.force_rendered_background);
        let (next, blur, gradient) = b.render_background(i32::MAX);
        assert!(next);
        assert!(!blur);
        assert!(!gradient); // latch: no second draw
    }

    #[test]
    fn render_rotated_gradient_math() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.gui_width = 427;
        b.gui_height = 240;
        // alpha 0: nothing to draw
        assert!(b.render_rotated_gradient().is_none());
        b.background_alpha_animation
            .set_target(BackgroundAlphaTarget::FadeIn);
        drive_fade_in(&mut b, 0.1, 4);
        let g = b.render_rotated_gradient().unwrap();
        let diagonal = ((427f32 * 427.0) + (240f32 * 240.0)).sqrt();
        assert!((g.scale - diagonal / 240.0).abs() < 1e-3);
        assert_eq!(g.rotation_radians, 0.0);
        assert_eq!((g.x1, g.y1, g.x2, g.y2), (-213, -120, 213, 120));
        // useGradient off kills the gradient
        b.config.use_gradient = false;
        assert!(b.render_rotated_gradient().is_none());
    }

    #[test]
    fn on_render_latches_and_on_render_end_resolves() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.current_screen_name = Some("mezz.jei.gui.recipes.RecipesGui".to_string());
        b.on_render();
        assert!(b.is_processing_render_pass);
        assert_eq!(b.blur_radius_animation.target(), None); // FadeOut pending
        b.on_render(); // second call in the same pass: no-op
        assert!(b.is_processing_render_pass);
        b.on_render_end(0.1 * 20.0);
        assert!(!b.is_processing_render_pass);
        assert_eq!(
            b.blur_radius_animation.target(),
            Some(BlurRadiusTarget::FadeIn)
        );
        assert_eq!(
            b.background_alpha_animation.target(),
            Some(BackgroundAlphaTarget::FadeIn)
        );
        assert_eq!(b.gradient_animation.target(), Some(GradientTarget::Fixed));
        // after the pass, onRenderEnd is a no-op
        let cur = b.blur_radius_animation.current_value();
        b.on_render_end(20.0);
        assert!((b.blur_radius_animation.current_value() - cur).abs() < 1e-6);
    }

    #[test]
    fn on_render_end_force_disabled_screens_fade_out() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.current_screen_name =
            Some("net.irisshaders.iris.gui.screen.ShaderPackScreen".to_string());
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        b.on_render();
        b.on_render_end(0.1 * 20.0);
        assert_eq!(
            b.blur_radius_animation.target(),
            Some(BlurRadiusTarget::FadeOut)
        );
        assert_eq!(
            b.background_alpha_animation.target(),
            Some(BackgroundAlphaTarget::FadeOut)
        );
    }

    #[test]
    fn on_render_end_picks_gradient_target_from_config() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.config.rainbow_mode = true;
        b.on_render();
        b.on_render_end(0.1 * 20.0);
        assert_eq!(b.gradient_animation.target(), Some(GradientTarget::Rainbow));
    }

    #[test]
    fn screen_background_gates() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        // a screen drawing any background fades the alpha in
        let draw_blurred = b.on_screen_background("dev.emi.emi.screen.RecipeScreen", i32::MAX);
        assert!(draw_blurred); // forceEnabled, not forceDisabled, canBlur
        b.update_animations(0.02); // applies the pending FadeIn
        assert_eq!(
            b.background_alpha_animation.target(),
            Some(BackgroundAlphaTarget::FadeIn)
        );
        // forceDisabledScreens never get the extra blurred background
        let mut b2 = Blur::new(plain_config());
        assert!(
            !b2.on_screen_background("net.irisshaders.iris.gui.screen.ShaderPackScreen", i32::MAX)
        );
        // a screen not on either list: nothing forced
        let mut b3 = Blur::new(plain_config());
        assert!(!b3.on_screen_background("net.minecraft.client.gui.screens.TitleScreen", i32::MAX));
        // blur unavailable: no forced blurred background
        let mut b4 = Blur::new(plain_config());
        assert!(!b4.on_screen_background("dev.emi.emi.screen.RecipeScreen", 7));
        // onBlurredBackgroundExtracted fades the radius in
        let mut b5 = Blur::new(plain_config());
        b5.on_blurred_background_extracted();
        b5.update_animations(0.02); // applies the pending FadeIn
        assert_eq!(
            b5.blur_radius_animation.target(),
            Some(BlurRadiusTarget::FadeIn)
        );
    }

    #[test]
    fn in_level_background_only_without_a_screen() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.in_level = true;
        b.gui_width = 427;
        b.gui_height = 240;
        b.background_alpha_animation
            .set_target(BackgroundAlphaTarget::FadeIn);
        drive_fade_in(&mut b, 0.1, 4);
        let (next, _blur, gradient) = b.before_render_screen_in_level(i32::MAX);
        assert!(next && gradient);
        // with a screen open, vanilla handles it
        b.force_rendered_background = false;
        b.current_screen_name = Some("net.minecraft.client.gui.screens.TitleScreen".to_string());
        let r = b.before_render_screen_in_level(i32::MAX);
        assert_eq!(r, (false, false, false));
        // not in a level: nothing either
        b.current_screen_name = None;
        b.in_level = false;
        b.force_rendered_background = false;
        assert_eq!(
            b.before_render_screen_in_level(i32::MAX),
            (false, false, false)
        );
    }

    #[test]
    fn container_gate_matches_mixin() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        let (render_blurred, in_game_ui) = b.container_gate(i32::MAX, true);
        assert!(render_blurred);
        assert!(!in_game_ui); // containers never count as in-game UI
                              // blurContainers off: passthrough, no blur
        set_reduced_blur(false);
        let mut b2 = Blur::new(plain_config());
        b2.config.blur_containers = false;
        let (render_blurred, in_game_ui) = b2.container_gate(i32::MAX, true);
        assert!(!render_blurred);
        assert!(in_game_ui);
        // reduceInGameBlur flips the global flag
        let mut b3 = Blur::new(plain_config());
        b3.config.reduce_in_game_blur = true;
        let _ = b3.container_gate(i32::MAX, true);
        assert!(reduced_blur());
        set_reduced_blur(false);
    }

    #[test]
    fn screen_category_gates_match_mixins() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        assert!(b.book_gate(i32::MAX));
        assert!(b.sign_gate(i32::MAX));
        assert!(b.command_block_gate(i32::MAX));
        assert!(!b.death_gate(i32::MAX)); // blurDeathScreen defaults false
        b.config.blur_death_screen = true;
        assert!(b.death_gate(i32::MAX));
        // canBlur gate kills all of them
        set_reduced_blur(false);
        let mut b2 = Blur::new(plain_config());
        assert!(!b2.book_gate(1));
        assert!(!b2.sign_gate(1));
        assert!(!b2.command_block_gate(1));
        assert!(!b2.death_gate(1));
    }

    #[test]
    fn title_gate_matches_mixin() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        let (blur, menu_gradient, _) = b.title_gate(i32::MAX);
        assert!(blur);
        assert!(menu_gradient); // useGradient default
        b.update_animations(0.02); // applies the pending FadeOut
        assert_eq!(
            b.blur_radius_animation.target(),
            Some(BlurRadiusTarget::FadeOut)
        );
        assert_eq!(
            b.background_alpha_animation.target(),
            Some(BackgroundAlphaTarget::FadeOut)
        );
        // with the title screen options on, both fade in
        let mut b2 = Blur::new(plain_config());
        b2.config.blur_title_screen = true;
        b2.config.darken_title_screen = true;
        let _ = b2.title_gate(i32::MAX);
        b2.update_animations(0.02); // applies the pending FadeIn
        assert_eq!(
            b2.blur_radius_animation.target(),
            Some(BlurRadiusTarget::FadeIn)
        );
        assert_eq!(
            b2.background_alpha_animation.target(),
            Some(BackgroundAlphaTarget::FadeIn)
        );
        // useGradient off keeps the vanilla menu background
        let mut b3 = Blur::new(plain_config());
        b3.config.use_gradient = false;
        let (_, menu_gradient, _) = b3.title_gate(i32::MAX);
        assert!(!menu_gradient);
    }

    #[test]
    fn menu_blurriness_coefficient_scales_the_setting() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        assert_eq!(b.apply_menu_background_blur_coefficient(10), 0);
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        drive_fade_in(&mut b, 0.05, 3); // cur = 0.5
        assert_eq!(b.apply_menu_background_blur_coefficient(10), 5);
        assert_eq!(Blur::MENU_BLURRINESS_MAX, 20);
    }

    #[test]
    fn update_animations_converts_ticks_to_seconds() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        b.blur_radius_animation.set_target(BlurRadiusTarget::FadeIn);
        // 20 ticks = 1 second = 1000ms: the 300ms fade completes
        b.update_animations(20.0);
        assert!((b.blur_radius_animation.current_value() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn debug_line_layout_and_colors() {
        let line = Blur::render_debug_line("Hi", 100, 2, 10, None, true).unwrap();
        assert_eq!((line.x1, line.y1, line.x2, line.y2), (1, 1, 13, 10));
        assert_eq!(line.text_color, 0x90E0E0E0u32 as i32);
        assert_eq!(line.bg_color as u32, 0x90383838); // 0xE0 * 0.25 = 0x38
        assert_eq!((line.text_x, line.text_y), (2, 2));
        assert_eq!(line.text, "Hi");
        // right-aligned
        let line = Blur::render_debug_line("Hi", 100, 2, 10, None, false).unwrap();
        assert_eq!(line.text_x, 88);
        // custom color keeps its channels at 25% for the background
        let line =
            Blur::render_debug_line("x", 100, 2, 4, Some(0xFF102030u32 as i32), true).unwrap();
        assert_eq!(line.bg_color as u32, 0x9004080c); // 0x10*0.25=4, 0x20*0.25=8, 0x30*0.25=12
                                                      // empty text draws nothing
        assert!(Blur::render_debug_line("", 100, 2, 0, None, true).is_none());
    }

    #[test]
    fn show_screen_id_draws_the_class_name() {
        reset_globals();
        let mut b = Blur::new(plain_config());
        assert!(b.debug_line_for_screen(10).is_none()); // off by default
        b.config.show_screen_id = true;
        b.current_screen_name = Some("net.minecraft.client.gui.screens.TitleScreen".to_string());
        let line = b.debug_line_for_screen(10).unwrap();
        assert_eq!(line.text, "net.minecraft.client.gui.screens.TitleScreen");
        assert_eq!(line.text_color, 0xff80dfffu32 as i32);
    }

    #[test]
    fn menu_background_gradient_swap() {
        let mut b = Blur::new(plain_config());
        assert!(b.menu_background_uses_gradient());
        b.config.use_gradient = false;
        assert!(!b.menu_background_uses_gradient());
    }
}
