/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of the `mixins` package (core mixins, duck interface and
//! accessor). Mixin injection points (`@Inject`, `@WrapOperation`, ...) are
//! declared in `mixins/rrls/horizon-mixin.toml` at the packaging site; what
//! lives here is the injected logic itself, translated method by method. The
//! Minecraft classes behind the injections (Gui, Minecraft, LoadingOverlay,
//! Screen) are host seams.

use crate::config::{DoubleLoad, RrlsConfig, Type};
use crate::rrls::ReloadersFuture;
use crate::utils::overlay_helper::{self, config, ScreenKind, State};
use crate::utils::rainbow::rainbow_color;

/// The config values the mixins read, cloned out of the shared store so each
/// translated method reads one coherent snapshot (`RrlsConfig.INSTANCE`).
pub fn current_config() -> RrlsConfig {
    config().read().expect("rrls config").clone()
}

/// Host seam for `Rrls.LOGGER.error(...)`.
pub fn log_error(message: &str) {
    let _ = (crate::rrls::LOGGER, message);
}

/// Upstream: `net.minecraft.util.Util.getMillis` (host clock seam).
pub fn util_get_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64
}

/// Host clock seam in nanoseconds for `RainbowUtils.rainbowColor`.
pub fn util_get_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64
}

/// `net.minecraft.util.Mth.lerp`.
pub fn mth_lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}

/// `net.minecraft.util.Mth.clamp` / `Mth.ceil`.
pub fn mth_clamp(value: f32, min: f32, max: f32) -> f32 {
    value.clamp(min, max)
}

pub fn mth_ceil(value: f32) -> i32 {
    value.ceil() as i32
}

/// `net.minecraft.util.ARGB.color(alpha, red, green, blue)`.
pub fn argb_color(alpha: i32, red: i32, green: i32, blue: i32) -> u32 {
    (((alpha & 0xFF) << 24) | ((red & 0xFF) << 16) | ((green & 0xFF) << 8) | (blue & 0xFF)) as u32
}

// ---------------------------------------------------------------------------
// duck/OverlayExtender.java
// ---------------------------------------------------------------------------

/// The graphics surface the mini-render draws through (the subset
/// `LoadingOverlayMixin.rrls$miniRender` uses; production passes
/// `DummyGuiGraphics.INSTANCE`).
pub trait MiniRenderSink {
    fn gui_width(&self) -> i32;
    fn gui_height(&self) -> i32;
    fn extract_progress_bar(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, fade: f32);
    fn centered_text(&mut self, text: &str, x: i32, y: i32, color: u32);
    fn extract_widget(&mut self, widget: &FocusableTextWidget, partial_tick: f32);
}

/// The pose/scissor operations `GuiMixin.rrls$enableScissor` wraps (host seam).
pub trait GraphicsOps {
    fn push_matrix(&mut self);
    fn pop_matrix(&mut self);
    fn enable_scissor(&mut self, x0: i32, y0: i32, x1: i32, y1: i32);
    fn disable_scissor(&mut self);
}

/// Upstream: `FocusableTextWidget` (host seam), narrowed to the setters
/// `rrls$miniRender` drives.
pub struct FocusableTextWidget {
    pub message: String,
    pub message_mutable: bool,
    pub width: i32,
    pub x: i32,
    pub y: i32,
    pub alpha: f32,
    pub color: Option<u32>,
}

impl FocusableTextWidget {
    pub fn new(message: &str, mutable: bool) -> Self {
        Self {
            message: message.to_string(),
            message_mutable: mutable,
            width: 0,
            x: 0,
            y: 0,
            alpha: 1.0,
            color: None,
        }
    }

    pub fn get_width(&self) -> i32 {
        self.width
    }
}

/// Upstream: `duck/OverlayExtender.java`.
pub trait OverlayExtender {
    fn rrls_get_state(&self) -> State;
    fn rrls_set_state(&mut self, state: State);

    fn rrls_mini_render(&mut self, _sink: &mut dyn MiniRenderSink, _partial_tick: f32) {
        panic!(
            "The '{}' overlay doesn't have a mini-render!",
            std::any::type_name::<Self>()
        );
    }

    fn rrls_reset_progress(&mut self) {
        // NO-OP
    }
}

/// Upstream: `net.minecraft.client.gui.screens.Overlay`, narrowed to the
/// render-state extraction the mixins call.
pub trait Overlay: OverlayExtender {
    /// `overlay.extractRenderState(DummyGuiGraphics.INSTANCE, 0, 0, delta)`.
    fn extract_render_state(&mut self, sink: &mut dyn MiniRenderSink, x: i32, y: i32, delta: f32);
}

/// Upstream: `net.minecraft.client.gui.Gui`, narrowed to its `overlay` field.
pub struct Gui {
    pub overlay: Option<Box<dyn Overlay>>,
    /// The live overlay's duck state, what `OverlayHelper.isRenderingState`
    /// reads through the object; `None` is "no overlay".
    pub overlay_state: Option<State>,
}

// ---------------------------------------------------------------------------
// mixins/GuiMixin.java
// ---------------------------------------------------------------------------

/// Upstream: `GuiMixin` (@Mixin(Gui.class)).
pub struct GuiMixin;

impl GuiMixin {
    /// `rrls$miniRender`: while an overlay is in a rendering state, its render
    /// state is re-extracted into the dummy graphics under the optional
    /// scissor, and the mini-render runs outside the scissor. `Err` carries
    /// what upstream catches as RuntimeException and logs.
    pub fn rrls_mini_render(
        gui: &mut Gui,
        sink: &mut dyn MiniRenderSink,
        ops: &mut dyn GraphicsOps,
        delta_ticks: f32,
    ) -> Result<(), String> {
        let config = current_config();
        if overlay_helper::is_rendering_state(gui.overlay_state) {
            let inner = Self::rrls_enable_scissor(ops, |ops| {
                if let Some(overlay) = gui.overlay.as_mut() {
                    overlay.extract_render_state(sink, 0, 0, delta_ticks);
                }
                let _ = ops;
                Ok(())
            });
            if let Err(ex) = inner {
                // Rrls.LOGGER.error("Failed to draw overlay!", ex)
                log_error("Failed to draw overlay!");
                return Err(ex);
            }

            if gui.overlay.is_some() && config.mini_render {
                if let Some(overlay) = gui.overlay.as_mut() {
                    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        overlay.rrls_mini_render(sink, delta_ticks);
                    }));
                    if res.is_err() {
                        log_error("Failed to draw overlay!");
                        return Err("mini render failed".to_string());
                    }
                }
            }
        }
        Ok(())
    }

    /// `rrls$blockOverlay(original)`: a rendering-state overlay is hidden from
    /// callers while `blockOverlay` is on. `None` is upstream's `null`.
    pub fn rrls_block_overlay(original_state: Option<State>) -> Option<State> {
        let config = current_config();
        if config.block_overlay && overlay_helper::is_rendering_state(original_state) {
            return None;
        }
        original_state
    }

    /// `rrls$enableScissor(graphics, runnable)`: scissor only when configured.
    pub fn rrls_enable_scissor<R>(
        graphics: &mut dyn GraphicsOps,
        runnable: impl FnOnce(&mut dyn GraphicsOps) -> R,
    ) -> R {
        if current_config().enable_scissor {
            graphics.push_matrix();
            graphics.enable_scissor(0, 0, 0, 0);

            let result = runnable(graphics);

            graphics.disable_scissor();
            graphics.pop_matrix();
            result
        } else {
            runnable(graphics)
        }
    }

    /// `rrls$miniRender` GETFIELD wrap (`handleKeybinds`, `extractRenderState`):
    /// a rendering-state overlay reads back as absent.
    pub fn rrls_mini_render_getfield(overlay_state: Option<State>) -> Option<State> {
        if overlay_helper::is_rendering_state(overlay_state) {
            return None;
        }
        overlay_state
    }
}

// ---------------------------------------------------------------------------
// mixins/MinecraftClientMixin.java
// ---------------------------------------------------------------------------

/// Upstream: `MinecraftClientMixin` (@Mixin(Minecraft.class)), narrowed to the
/// fields its injections touch (`gameLoadFinished`, `gui.overlay`).
pub struct MinecraftClientMixin {
    pub game_load_finished: bool,
    pub gui_overlay_state: Option<State>,
    pub mod_reloaders: ReloadersFuture,
}

impl MinecraftClientMixin {
    pub fn new() -> Self {
        Self {
            game_load_finished: false,
            gui_overlay_state: None,
            mod_reloaders: ReloadersFuture::new(),
        }
    }

    /// `rrls$screen(component)`: while rrls is rendering, the vanilla
    /// GenericMessageScreen is never created (`null` upstream).
    pub fn rrls_screen(&self) -> Option<()> {
        if overlay_helper::is_current_rendering_state(self.gui_overlay_state) {
            return None;
        }
        Some(())
    }

    /// `rrls$init`: finish the load early once the mod reloaders future has a
    /// live value (`getAndSet(null)` + `managedBlock(isDone)`), then let the
    /// vanilla `onResourceLoadFinished` run. `Err` is what upstream catches
    /// and turns into `gameLoadFinished = false`.
    pub fn rrls_init(&mut self) -> Result<(), String> {
        if !overlay_helper::is_current_rendering_state(self.gui_overlay_state) {
            return Ok(());
        }

        log_error("Trying to complete load early..."); // upstream: LOGGER.info

        let live = self.mod_reloaders.get_and_set_null();
        if live {
            managed_block(&|| {
                matches!(self.mod_reloaders.current().1, Some(crate::rrls::SharedState::Done))
            });
        }

        self.on_resource_load_finished();
        Ok(())
    }

    /// `Minecraft.onResourceLoadFinished` (shadowed host call).
    pub fn on_resource_load_finished(&mut self) {}

    /// `rrls$fixDH(instance)`: the runnable still runs, but the wrap never
    /// lets the original continue (`return false`).
    pub fn rrls_fix_dh(&self, instance: &mut dyn FnMut()) -> bool {
        instance();
        false
    }

    /// `rrls$onResourceReloadFailure`: when `resetResources` is off, the
    /// vanilla reset is cancelled, the error is logged, and a double load is
    /// scheduled only when `doubleLoad` is a load variant. Returns whether the
    /// vanilla method continues (upstream `ci.cancel()`).
    pub fn rrls_on_resource_reload_failure(
        &mut self,
        reload_scheduled: &mut bool,
        force_load: &mut bool,
    ) -> bool {
        if !current_config().reset_resources {
            // Rrls.LOGGER.error("Caught error loading resource packs!", throwable)
            log_error("Caught error loading resource packs!");

            let config = current_config();
            if config.double_load.is_load() {
                *force_load = config.double_load == DoubleLoad::ForceLoad;
                *reload_scheduled = true;
            }
            return false;
        }
        true
    }

    /// `rrls$doubleLoad`: without a double-load variant the vanilla reload
    /// call is cancelled.
    pub fn rrls_double_load(&self) -> bool {
        !current_config().double_load.is_load()
    }

    /// `rrls$doubleLoad(boolean error)`: the forced-reload flag is always the
    /// config comparison (`true` upstream's comment).
    pub fn rrls_double_load_arg(&self, _error: bool) -> bool {
        current_config().double_load == DoubleLoad::ForceLoad
    }

    /// `rrls$miniRenderTick` (tick) and `rrls$miniRenderWorldLoad`
    /// (doWorldLoad, ordinal 0, which delegates to it).
    pub fn rrls_mini_render_tick(&self, original_state: Option<State>) -> Option<State> {
        if overlay_helper::is_rendering_state(original_state) {
            return None;
        }
        original_state
    }

    /// `rrls$removeTick`: the frame render inside `setScreenAndShow` is
    /// skipped while an overlay is rendering.
    pub fn rrls_remove_tick(&self) -> bool {
        !overlay_helper::is_rendering_state(self.gui_overlay_state)
    }

    /// `rrls$fixPanorama` (condition form): panorama registration is skipped
    /// while rrls is rendering.
    pub fn rrls_fix_panorama(&self) -> bool {
        !overlay_helper::is_current_rendering_state(self.gui_overlay_state)
    }

    /// `rrls$fixPanorama` (injection form): when rrls is rendering, the
    /// panorama registration is replayed here instead.
    pub fn rrls_fix_panorama_init(&self, registered: &mut bool) {
        if !overlay_helper::is_current_rendering_state(self.gui_overlay_state) {
            return;
        }
        *registered = true; // gameRenderer.registerPanoramaTextures(textureManager)
    }
}

/// Upstream: `ForkJoinPool.managedBlock` around `future.isDone()`. The host
/// executor drives the future; this seam re-checks completion and lets the
/// caller's scheduler interleave.
pub fn managed_block(is_done: &dyn Fn() -> bool) {
    let _ = is_done();
}

// ---------------------------------------------------------------------------
// mixins/LoadingOverlayMixin.java
// ---------------------------------------------------------------------------

/// Upstream: `LoadingOverlayMixin` (@Mixin(LoadingOverlay.class)): the overlay
/// that replaces the vanilla loading screen with the configured progress,
/// text or text-with-background render, and carries the rrls duck state.
pub struct LoadingOverlayMixin {
    pub state: State,
    pub current_progress: f32,
    pub fade_out_start: i64,
    pub fade_in_start: i64,
    pub text_widget: Option<FocusableTextWidget>,
    pub at_end_start: i64,
    pub is_finished: bool,
    /// `minecraft.gui.screen()` for `lookupState`.
    pub screen: Option<ScreenKind>,
}

impl LoadingOverlayMixin {
    pub fn new() -> Self {
        Self {
            state: State::Default,
            current_progress: 0.0,
            fade_out_start: -1,
            fade_in_start: -1,
            text_widget: None,
            at_end_start: -1,
            is_finished: false,
            screen: None,
        }
    }

    /// `rrls$init` (constructor TAIL): the duck state is looked up from the
    /// current screen and the fade-in flag, and the text widget is built for
    /// the TEXT_WITH_BACKGROUND type.
    pub fn rrls_init(&mut self, fade_in: bool) {
        self.rrls_set_state(overlay_helper::lookup_state(self.screen, fade_in));

        if current_config().r#type == Type::TextWithBackground {
            self.text_widget = Some(FocusableTextWidget::new(&current_config().reload_text, false));
        }
    }

    /// `rrls$resetProgress`: every fade timer and the progress reset.
    pub fn rrls_reset_progress(&mut self) {
        self.is_finished = false;
        self.current_progress = 0.0;
        self.at_end_start = -1;
        self.fade_out_start = -1;
        self.fade_in_start = -1;
    }

    /// `rrls$interpolateAtEnd` (tick injection): once, when the progress
    /// reaches the end, stamp the at-end fade start.
    pub fn rrls_interpolate_at_end(&mut self) {
        if self.at_end_start == -1 && self.current_progress >= 0.99999 {
            self.at_end_start = util_get_millis();
        }
    }

    /// `rrls$screenrender`: the screen render state extraction is skipped into
    /// the dummy graphics (`!(graphics instanceof DummyGuiGraphics)`).
    pub fn rrls_screenrender(&self, graphics_is_dummy: bool) -> bool {
        !graphics_is_dummy
    }

    /// `rrls$reinitScreen` (setOverlay wrap): the overlay is replaced only
    /// when the duck state is back to DEFAULT or the mini-render finished.
    pub fn rrls_reinit_screen(&mut self) -> bool {
        let is_removed = self.state == State::Default || self.is_finished;
        if is_removed {
            log_error("Overlay is removed!"); // upstream: LOGGER.info
        }
        is_removed
    }

    /// `rrls$reinitScreen` (Screen.init wrap).
    pub fn rrls_reinit_screen_init(&self) -> bool {
        current_config().re_init_screen
    }

    /// `rrls$_clearColor`: the clear color call is skipped into the dummy
    /// graphics (the destination vector is returned untouched).
    pub fn rrls_clear_color(&self, graphics_is_dummy: bool) -> bool {
        !graphics_is_dummy
    }

    /// `rrls$lerp` (extractProgressBar Math.round wrap): outside DEFAULT the
    /// alpha is lerped by the fade instead.
    pub fn rrls_lerp(&self, i: f32, fade: f32) -> f32 {
        if self.state != State::Default {
            return mth_lerp(fade, 0.0, 255.0);
        }
        i
    }

    /// `rrls$rainbowProgress` (ARGB.color wrap): rainbow wins when enabled and
    /// the state is not DEFAULT.
    pub fn rrls_rainbow_progress(&self, alpha: i32, red: i32, green: i32, blue: i32, fade: f32) -> u32 {
        if current_config().rgb_progress && self.state != State::Default {
            return rainbow_color(fade, util_get_nanos());
        }
        argb_color(alpha, red, green, blue)
    }
}

/// `net.minecraft.util.Mth.clamp` alias for the mini-render's ease branch.
fn mth_clamp_f(value: f32, min: f32, max: f32) -> f32 {
    mth_clamp(value, min, max)
}

impl OverlayExtender for LoadingOverlayMixin {
    fn rrls_get_state(&self) -> State {
        self.state
    }

    fn rrls_set_state(&mut self, state: State) {
        self.state = state;
    }

    /// `rrls$miniRender`: the configured overlay render. Ease comes either
    /// from the interpolated progress or from the fade animation, the alpha is
    /// lerped 4..255 and ceiled (the upstream `Fuck Font#adjustColor` note is
    /// kept), and the type switch drives the progress bar, the centered text
    /// or the background text widget.
    fn rrls_mini_render(&mut self, sink: &mut dyn MiniRenderSink, partial_tick: f32) {
        let config = current_config();
        let i = sink.gui_width();
        let j = sink.gui_height();

        let fade_time = if config.interpolate_at_end {
            self.at_end_start
        } else {
            self.fade_out_start
        };

        let mut ease = 1.0_f32;
        if config.interpolate_progress {
            ease -= config.easing.invoke(self.current_progress, config.easing_arg);
        } else if fade_time > -1 {
            let f = (util_get_millis() - fade_time) as f32 / config.animation_speed;
            ease -= config
                .easing
                .invoke(mth_clamp_f(f, 0.0, 1.0), config.easing_arg);
        }

        if ease <= 0.0 {
            self.is_finished = true;
        }

        let ease_alpha = mth_ceil(mth_lerp(ease, 4.0, 255.0)); // Fuck Font#adjustColor
        let ease_color = argb_color(ease_alpha, 255, 255, 255);

        match config.r#type {
            Type::Progress => {
                let s = ((j as f64) * 0.8325) as i32;
                let r = (((i as f64) * 0.75).min(j as f64) * 0.5) as i32;

                sink.extract_progress_bar(i / 2 - r, s - 5, i / 2 + r, s + 5, ease);
            }

            Type::Text => {
                let color = if config.rgb_progress {
                    rainbow_color(ease_alpha as f32, util_get_nanos())
                } else {
                    ease_color
                };
                sink.centered_text(&config.reload_text, i / 2, 70, color);
            }

            Type::TextWithBackground => {
                if let Some(widget) = self.text_widget.as_mut() {
                    widget.width = i;
                    widget.x = i / 2 - widget.get_width() / 2;
                    widget.y = j - j / 3;
                    widget.alpha = crate::utils::rainbow::from_8bit_channel(ease_alpha);

                    if widget.message_mutable && config.rgb_progress {
                        widget.color = Some(rainbow_color(ease_alpha as f32, util_get_nanos()));
                    }

                    sink.extract_widget(widget, partial_tick);
                }
            }

            Type::None => {}
        }
    }

    fn rrls_reset_progress(&mut self) {
        LoadingOverlayMixin::rrls_reset_progress(self);
    }
}

impl Overlay for LoadingOverlayMixin {
    fn extract_render_state(&mut self, _sink: &mut dyn MiniRenderSink, _x: i32, _y: i32, _delta: f32) {
        // Host seam: the vanilla LoadingOverlay render-state extraction.
    }
}

// ---------------------------------------------------------------------------
// mixins/TranslatableEnumMixin.java + accessors
// ---------------------------------------------------------------------------

/// Upstream: `net.neoforged.neoforge.common.TranslatableEnum`, applied to
/// `DoubleLoad`, `HideType` and `Type` by `TranslatableEnumMixin`. The enums
/// already carry their translation keys, so the marker holds; the NeoForge
/// interface itself is the host seam.
pub trait TranslatableEnum {}

impl TranslatableEnum for DoubleLoad {}
impl TranslatableEnum for crate::config::HideType {}
impl TranslatableEnum for Type {}

/// Upstream: `mixins/accessors/ResourceLoaderImplAccessor.java`, a Fabric
/// internals accessor (`ResourceLoaderImpl.addedReloaders`). Horizon has no
/// Fabric resource-loader implementation; the added-reloader map is a host
/// seam read at the mixin site.
pub mod accessors {
    /// `getAddedReloaders()` returns the mod-added reload listeners.
    pub fn get_added_reloaders() -> std::collections::HashMap<String, ()> {
        std::collections::HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::RwLock;

    static CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Holds the shared config store for the whole test body: the mixins read
    /// the live store, so parallel tests would race each other's writes.
    fn config_guard() -> std::sync::MutexGuard<'static, ()> {
        CONFIG_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn with_config(f: impl FnOnce(&mut RrlsConfig)) {
        let mut guard = config().write().expect("rrls config");
        f(&mut guard);
    }

    /// Counter sink recording the calls the mini-render makes.
    #[derive(Default)]
    struct CountingSink {
        width: i32,
        height: i32,
        progress_bars: Vec<(i32, i32, i32, i32, f32)>,
        texts: Vec<(String, i32, i32, u32)>,
        widgets: Vec<f32>,
    }

    impl MiniRenderSink for CountingSink {
        fn gui_width(&self) -> i32 {
            self.width
        }
        fn gui_height(&self) -> i32 {
            self.height
        }
        fn extract_progress_bar(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, fade: f32) {
            self.progress_bars.push((x0, y0, x1, y1, fade));
        }
        fn centered_text(&mut self, text: &str, x: i32, y: i32, color: u32) {
            self.texts.push((text.to_string(), x, y, color));
        }
        fn extract_widget(&mut self, _widget: &FocusableTextWidget, partial_tick: f32) {
            self.widgets.push(partial_tick);
        }
    }

    #[derive(Default)]
    struct NoOps;

    impl GraphicsOps for NoOps {
        fn push_matrix(&mut self) {}
        fn pop_matrix(&mut self) {}
        fn enable_scissor(&mut self, _x0: i32, _y0: i32, _x1: i32, _y1: i32) {}
        fn disable_scissor(&mut self) {}
    }

    #[test]
    fn block_overlay_hides_only_rendering_states_when_enabled() {
        let _g = config_guard();
        with_config(|c| c.block_overlay = true);
        assert_eq!(GuiMixin::rrls_block_overlay(Some(State::Default)), Some(State::Default));
        assert_eq!(GuiMixin::rrls_block_overlay(Some(State::Hide)), None);
        assert_eq!(GuiMixin::rrls_block_overlay(None), None);
        with_config(|c| c.block_overlay = false);
        assert_eq!(GuiMixin::rrls_block_overlay(Some(State::Hide)), Some(State::Hide));
    }

    #[test]
    fn enable_scissor_gates_on_config() {
        let _g = config_guard();
        with_config(|c| c.enable_scissor = true);
        let mut ops = NoOps;
        assert_eq!(GuiMixin::rrls_enable_scissor(&mut ops, |_| 7), 7);
        with_config(|c| c.enable_scissor = false);
        assert_eq!(GuiMixin::rrls_enable_scissor(&mut ops, |_| 3), 3);
    }

    #[test]
    fn getfield_wrap_hides_rendering_overlays() {
        let _g = config_guard();
        assert_eq!(GuiMixin::rrls_mini_render_getfield(Some(State::Hide)), None);
        assert_eq!(GuiMixin::rrls_mini_render_getfield(Some(State::Default)), Some(State::Default));
    }

    #[test]
    fn mini_render_runs_only_while_rendering() {
        let _g = config_guard();
        let mut sink = CountingSink { width: 320, height: 240, ..Default::default() };
        let mut ops = NoOps;
        let mut overlay = LoadingOverlayMixin::new();
        let mut gui = Gui { overlay: Some(Box::new(overlay)), overlay_state: Some(State::Default) };

        with_config(|c| c.mini_render = true);
        assert!(GuiMixin::rrls_mini_render(&mut gui, &mut sink, &mut ops, 0.5).is_ok());
        assert!(sink.progress_bars.is_empty() && sink.texts.is_empty());

        gui.overlay_state = Some(State::Hide);
        let _ = GuiMixin::rrls_mini_render(&mut gui, &mut sink, &mut ops, 0.5);
        assert_eq!(sink.progress_bars.len(), 1, "DEFAULT config type draws the progress bar");
    }

    #[test]
    fn screen_creation_skipped_while_rendering() {
        let _g = config_guard();
        let mut m = MinecraftClientMixin::new();
        m.gui_overlay_state = Some(State::Hide);
        assert!(m.rrls_screen().is_none());
        m.gui_overlay_state = Some(State::Default);
        assert!(m.rrls_screen().is_some());
    }

    #[test]
    fn resource_reload_failure_cancelled_without_reset() {
        let _g = config_guard();
        let mut m = MinecraftClientMixin::new();
        with_config(|c| c.reset_resources = false);
        with_config(|c| c.double_load = DoubleLoad::ForceLoad);
        let mut reload_scheduled = false;
        let mut force_load = false;
        assert!(!m.rrls_on_resource_reload_failure(&mut reload_scheduled, &mut force_load));
        assert!(reload_scheduled && force_load);

        with_config(|c| c.double_load = DoubleLoad::None);
        let mut reload_scheduled = false;
        let mut force_load = false;
        assert!(!m.rrls_on_resource_reload_failure(&mut reload_scheduled, &mut force_load));
        assert!(!reload_scheduled);

        with_config(|c| c.reset_resources = true);
        assert!(m.rrls_on_resource_reload_failure(&mut reload_scheduled, &mut force_load));
    }

    #[test]
    fn double_load_flags_follow_config() {
        let _g = config_guard();
        with_config(|c| c.double_load = DoubleLoad::None);
        // rrls$doubleLoad cancels the vanilla reload when doubleLoad is NOT a load variant
        assert!(MinecraftClientMixin::new().rrls_double_load());
        assert!(!MinecraftClientMixin::new().rrls_double_load_arg(false));
        with_config(|c| c.double_load = DoubleLoad::Load);
        assert!(!MinecraftClientMixin::new().rrls_double_load());
        assert!(!MinecraftClientMixin::new().rrls_double_load_arg(false));
    }

    #[test]
    fn overlay_hides_while_rendering_and_frame_render_gated() {
        let _g = config_guard();
        let mut m = MinecraftClientMixin::new();
        m.gui_overlay_state = Some(State::Hide);
        assert!(m.rrls_mini_render_tick(Some(State::Hide)).is_none());
        assert!(!m.rrls_remove_tick());
        assert!(!m.rrls_fix_panorama());
        let mut registered = false;
        m.rrls_fix_panorama_init(&mut registered);
        assert!(registered, "registration replays through the fix while rendering");
    }

    #[test]
    fn init_completes_load_early_once() {
        let _g = config_guard();
        let mut m = MinecraftClientMixin::new();
        m.gui_overlay_state = Some(State::Hide);
        m.mod_reloaders.set_new(None);
        m.mod_reloaders.complete();
        assert!(m.rrls_init().is_ok());
        assert!(!m.game_load_finished, "gameLoadFinished is not flipped on success");
    }

    #[test]
    fn loading_overlay_init_looks_up_state_and_builds_widget() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        overlay.screen = None;
        with_config(|c| c.r#type = Type::TextWithBackground);
        with_config(|c| c.reload_text = "Reloading...".into());
        overlay.rrls_init(true);
        assert!(overlay.text_widget.is_some());
        assert_eq!(overlay.text_widget.as_ref().unwrap().message, "Reloading...");
        with_config(|c| c.r#type = Type::Progress);
        let mut plain = LoadingOverlayMixin::new();
        plain.rrls_init(false);
        assert!(plain.text_widget.is_none());
    }

    #[test]
    fn mini_render_progress_type_scales_the_bar() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        let mut sink = CountingSink { width: 640, height: 480, ..Default::default() };
        with_config(|c| c.r#type = Type::Progress);
        with_config(|c| c.interpolate_progress = true);
        overlay.rrls_mini_render(&mut sink, 0.0);
        let (x0, y0, x1, y1, _) = sink.progress_bars[0];
        // s = 480 * 0.8325 = 399 (int cast), r = min(640*0.75, 480) * 0.5 = 240
        assert_eq!((x0, y0, x1, y1), (320 - 240, 399 - 5, 320 + 240, 399 + 5));
    }

    #[test]
    fn mini_render_text_type_centers_the_label() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        let mut sink = CountingSink { width: 640, height: 480, ..Default::default() };
        with_config(|c| c.r#type = Type::Text);
        with_config(|c| c.rgb_progress = false);
        with_config(|c| c.reload_text = "hold on".into());
        with_config(|c| c.interpolate_progress = true);
        with_config(|c| c.easing = crate::utils::ease::Ease::Linear);
        overlay.rrls_mini_render(&mut sink, 0.0);
        let (text, x, y, color) = sink.texts[0].clone();
        assert_eq!(text, "hold on");
        assert_eq!(x, 320);
        assert_eq!(y, 70);
        // progress 0 with linear easing: ease = 1, alpha = ceil(lerp(1, 4, 255)) = 255
        assert_eq!(color, argb_color(255, 255, 255, 255));
    }

    #[test]
    fn ease_finishing_marks_the_overlay_done() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        let mut sink = CountingSink { width: 100, height: 100, ..Default::default() };
        with_config(|c| c.r#type = Type::Progress);
        with_config(|c| c.interpolate_progress = false);
        with_config(|c| c.animation_speed = 1.0);
        overlay.fade_out_start = util_get_millis() - 10_000;
        overlay.rrls_mini_render(&mut sink, 0.0);
        assert!(overlay.is_finished, "ease <= 0 marks finished");
    }

    #[test]
    fn text_with_background_positions_the_widget() {
        let _g = config_guard();
        with_config(|c| c.r#type = Type::TextWithBackground);
        with_config(|c| c.reload_text = "Reloading...".into());
        let mut overlay = LoadingOverlayMixin::new();
        overlay.rrls_init(false);
        let mut sink = CountingSink { width: 300, height: 300, ..Default::default() };
        with_config(|c| c.interpolate_progress = true);
        overlay.rrls_mini_render(&mut sink, 0.25);
        let widget = overlay.text_widget.as_ref().unwrap();
        assert_eq!(widget.width, 300);
        assert_eq!(widget.x, 0, "x = i/2 - width/2, width modeled as maxWidth");
        assert_eq!(widget.y, 200);
        assert_eq!(sink.widgets.len(), 1);
    }

    #[test]
    fn interpolate_at_end_stamps_once() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        overlay.rrls_interpolate_at_end();
        assert_eq!(overlay.at_end_start, -1, "low progress never stamps");

        overlay.current_progress = 1.0;
        overlay.rrls_interpolate_at_end();
        assert!(overlay.at_end_start > 0);
        let stamped = overlay.at_end_start;
        overlay.rrls_interpolate_at_end();
        assert_eq!(overlay.at_end_start, stamped, "stamps only once");
    }

    #[test]
    fn reset_progress_clears_every_timer() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        overlay.current_progress = 0.7;
        overlay.at_end_start = 5;
        overlay.fade_out_start = 6;
        overlay.fade_in_start = 7;
        overlay.is_finished = true;
        overlay.rrls_reset_progress();
        assert_eq!(overlay.current_progress, 0.0);
        assert_eq!(overlay.at_end_start, -1);
        assert_eq!(overlay.fade_out_start, -1);
        assert_eq!(overlay.fade_in_start, -1);
        assert!(!overlay.is_finished);
    }

    #[test]
    fn reinit_and_lerp_gates() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        with_config(|c| c.re_init_screen = false);
        assert!(!overlay.rrls_reinit_screen_init());
        overlay.is_finished = false;
        overlay.state = State::Hide;
        assert!(!overlay.rrls_reinit_screen());
        overlay.state = State::Default;
        assert!(overlay.rrls_reinit_screen());

        overlay.state = State::Hide;
        assert!((overlay.rrls_lerp(200.0, 0.5) - 127.5).abs() < 1e-4);
        overlay.state = State::Default;
        assert_eq!(overlay.rrls_lerp(42.0, 0.0), 42.0);
    }

    #[test]
    fn rainbow_progress_gates_on_state() {
        let _g = config_guard();
        let mut overlay = LoadingOverlayMixin::new();
        overlay.state = State::Default;
        assert_eq!(overlay.rrls_rainbow_progress(255, 10, 20, 30, 0.5), argb_color(255, 10, 20, 30));
        overlay.state = State::Hide;
        with_config(|c| c.rgb_progress = false);
        assert_eq!(overlay.rrls_rainbow_progress(255, 10, 20, 30, 0.5), argb_color(255, 10, 20, 30));
    }

    #[test]
    fn dummy_graphics_skips_screen_and_clear_color() {
        let _g = config_guard();
        let overlay = LoadingOverlayMixin::new();
        assert!(!overlay.rrls_screenrender(true));
        assert!(overlay.rrls_screenrender(false));
        assert!(!overlay.rrls_clear_color(true));
        assert!(overlay.rrls_clear_color(false));
    }

    #[test]
    #[should_panic(expected = "doesn't have a mini-render!")]
    fn default_overlay_extender_has_no_mini_render() {
        let _g = config_guard();
        struct Bare(Option<State>);
        impl OverlayExtender for Bare {
            fn rrls_get_state(&self) -> State {
                self.0.unwrap_or(State::Default)
            }
            fn rrls_set_state(&mut self, _s: State) {}
        }
        let mut sink = CountingSink::default();
        Bare(None).rrls_mini_render(&mut sink, 0.0);
    }

    #[test]
    fn fix_dh_runs_then_blocks() {
        let _g = config_guard();
        let m = MinecraftClientMixin::new();
        let mut ran = 0;
        assert!(!m.rrls_fix_dh(&mut || ran += 1));
        assert_eq!(ran, 1);
    }

    // Silence the RwLock import warning when tests change shape.
    #[allow(dead_code)]
    fn assert_lock_type(lock: &RwLock<RrlsConfig>) {
        let _ = lock;
    }
}

// ---------------------------------------------------------------------------
// mixins/animation/ (both files, priority 999)
// ---------------------------------------------------------------------------

/// Mixins under `mixins/animation/`: they retune the vanilla fade animations
/// while rrls owns the screen.
pub mod animation {
    use super::*;

    /// Upstream vanilla `LoadingOverlay` fade constants. Host seam: the exact
    /// values come from the host's `LoadingOverlay`; the mixin matches on them
    /// to tell the fade-out constant from the fade-in one.
    pub const FADE_OUT_TIME: f32 = 1000.0;
    pub const FADE_IN_TIME: f32 = 500.0;

    /// Upstream: `animation/LoadingOverlayMixin.java` (@Mixin(LoadingOverlay,
    /// priority 999)).
    pub struct AnimationLoadingOverlayMixin {
        pub state: State,
    }

    impl AnimationLoadingOverlayMixin {
        /// `rrls$changeAnimationSpeed`: outside a rendering state the vanilla
        /// fade constants are replaced by the configured speed, the fade-in
        /// one halved.
        pub fn rrls_change_animation_speed(&self, instance: f32) -> f32 {
            if !self.state.is_rendering() {
                let config = current_config();
                return if instance == FADE_OUT_TIME {
                    config.animation_speed
                } else {
                    config.animation_speed / 2.0
                };
            }
            instance
        }
    }

    /// Upstream: `animation/TitleScreenMixin.java` (@Mixin(TitleScreen,
    /// priority 999)).
    pub struct TitleScreenMixin {
        pub fading: bool,
    }

    impl TitleScreenMixin {
        pub fn new() -> Self {
            Self { fading: false }
        }

        /// `rrls$removeFade` (constructor RETURN): while rrls is rendering,
        /// the title screen's fade is suppressed.
        pub fn rrls_remove_fade(&mut self, live_state: Option<State>) {
            if self.fading && overlay_helper::is_current_rendering_state(live_state) {
                self.fading = false;
            }
        }

        /// `rrls$changeAnimationSpeed`: the title screen's fade-in runs at
        /// double the configured speed.
        pub fn rrls_change_animation_speed(&self, _instance: f32) -> f32 {
            current_config().animation_speed * 2.0
        }
    }
}

// ---------------------------------------------------------------------------
// mixins/compat/ (five files)
// ---------------------------------------------------------------------------

/// Mixins under `mixins/compat/`: compat shims that keep the vanilla client
/// coherent while rrls holds the screen.
pub mod compat {
    use super::*;
    use crate::utils::texture_utils;

    /// Upstream: `net.minecraft.client.renderer.item.EmptyModel` (host seam).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct EmptyModel;

    pub static EMPTY_MODEL: EmptyModel = EmptyModel;

    /// Upstream: `compat/CubeMapMixin.java`. After the vanilla register call,
    /// a reloadable texture registered while rrls is rendering is reloaded
    /// immediately through the texture utils.
    pub struct CubeMapMixin;

    impl CubeMapMixin {
        pub fn rrls_early_register(
            registered: bool,
            texture_reloadable: bool,
            live_state: Option<State>,
            pending: &texture_utils::PendingTextures,
            path: &texture_utils::Identifier,
        ) {
            let _ = registered; // original.call(instance, path, texture)
            if overlay_helper::is_current_rendering_state(live_state) && texture_reloadable {
                let _ = pending.reload_texture(path.clone());
            }
        }
    }

    /// Upstream: `compat/ItemModelResolverMixin.java`. A model lookup failing
    /// during the menu render falls back to the empty model.
    pub struct ItemModelResolverMixin;

    impl ItemModelResolverMixin {
        pub fn rrls_fix_items_in_menu(
            lookup: impl FnOnce() -> Result<(), String>,
        ) -> Result<(), EmptyModel> {
            match lookup() {
                Ok(()) => Ok(()),
                Err(_) => Err(EMPTY_MODEL),
            }
        }
    }

    /// Upstream: `compat/OverlayMixin.java` (@Mixin(Overlay.class)): the base
    /// duck state every overlay carries, plus the pause gate.
    pub struct OverlayMixin {
        pub rrls_state: State,
    }

    impl Default for OverlayMixin {
        fn default() -> Self {
            Self {
                rrls_state: State::Default,
            }
        }
    }

    impl OverlayExtender for OverlayMixin {
        fn rrls_get_state(&self) -> State {
            self.rrls_state
        }

        fn rrls_set_state(&mut self, state: State) {
            self.rrls_state = state;
        }
    }

    impl OverlayMixin {
        /// `rrls$pausesGame(original)`: a rendering-state overlay never pauses
        /// the game.
        pub fn rrls_pauses_game(&self, original: bool) -> bool {
            !self.rrls_state.is_rendering() && original
        }
    }

    /// Upstream: `compat/RendererKeyboardMouseMixin.java`
    /// (@Mixin({KeyboardHandler, MouseHandler, FramerateLimitTracker})): the
    /// `Gui.overlay()` lookup reads back as absent while a rendering-state
    /// overlay is live.
    pub struct RendererKeyboardMouseMixin;

    impl RendererKeyboardMouseMixin {
        pub fn rrls_check_rendering_state(
            original_state: Option<State>,
        ) -> Option<State> {
            if overlay_helper::is_rendering_state(original_state) {
                return None;
            }
            original_state
        }
    }

    /// Upstream: `compat/ServerPackManagerMixin.java`. With
    /// `earlyPackStatusSend` on, the SUCCESSFULLY_LOADED resource-pack packet
    /// is sent as soon as the pack is pushed, before the vanilla flow.
    pub struct ServerPackManagerMixin;

    impl ServerPackManagerMixin {
        pub fn early_resource_pack_status_send(
            early_send: bool,
            connection_connected: bool,
            sent: &mut bool,
        ) {
            if early_send && connection_connected {
                // handler.getConnection().send(new
                // ServerboundResourcePackPacket(id, SUCCESSFULLY_LOADED))
                *sent = true;
            }
        }
    }
}

#[cfg(test)]
mod animation_compat_tests {
    use super::animation::*;
    use super::compat::*;
    use super::*;
    use crate::utils::texture_utils;

    #[test]
    fn animation_speed_replaces_fade_constants_outside_rendering() {
        let m = AnimationLoadingOverlayMixin { state: State::Default };
        assert_eq!(m.rrls_change_animation_speed(FADE_OUT_TIME), 1.0);
        assert_eq!(m.rrls_change_animation_speed(FADE_IN_TIME), 0.5);

        let rendering = AnimationLoadingOverlayMixin { state: State::Hide };
        assert_eq!(rendering.rrls_change_animation_speed(FADE_OUT_TIME), FADE_OUT_TIME);
        assert_eq!(rendering.rrls_change_animation_speed(3.0), 3.0);
    }

    #[test]
    fn title_screen_fade_removed_while_rendering_and_doubled_speed() {
        let mut m = TitleScreenMixin::new();
        m.fading = true;
        m.rrls_remove_fade(Some(State::Hide));
        assert!(!m.fading);

        let mut m2 = TitleScreenMixin::new();
        m2.fading = true;
        m2.rrls_remove_fade(Some(State::Default));
        assert!(m2.fading, "fade stays outside a rendering state");

        assert_eq!(m2.rrls_change_animation_speed(FADE_IN_TIME), 2.0);
    }

    #[test]
    fn cubemap_early_register_gates_on_rendering_and_reloadable() {
        let pending = texture_utils::PendingTextures::default();
        let path: crate::utils::texture_utils::Identifier = "test:cube".to_string();

        CubeMapMixin::rrls_early_register(true, true, Some(State::Hide), &pending, &path);
        assert!(pending.is_pending("test:cube"));

        let pending2 = texture_utils::PendingTextures::default();
        CubeMapMixin::rrls_early_register(true, true, Some(State::Default), &pending2, &path);
        assert!(!pending2.is_pending("test:cube"));
    }

    #[test]
    fn item_model_lookup_falls_back_to_empty_model() {
        assert!(ItemModelResolverMixin::rrls_fix_items_in_menu(|| Ok(())).is_ok());
        assert_eq!(
            ItemModelResolverMixin::rrls_fix_items_in_menu(|| Err("boom".to_string())),
            Err(EMPTY_MODEL)
        );
    }

    #[test]
    fn overlay_base_duck_state_and_pause_gate() {
        let mut m = OverlayMixin::default();
        assert_eq!(m.rrls_get_state(), State::Default);
        m.rrls_set_state(State::Hide);
        assert_eq!(m.rrls_get_state(), State::Hide);
        assert!(!m.rrls_pauses_game(true));
        m.rrls_set_state(State::Default);
        assert!(m.rrls_pauses_game(true));
        assert!(!m.rrls_pauses_game(false));
    }

    #[test]
    fn keyboard_mouse_overlay_lookup_hides_rendering_states() {
        assert_eq!(RendererKeyboardMouseMixin::rrls_check_rendering_state(Some(State::Hide)), None);
        assert_eq!(
            RendererKeyboardMouseMixin::rrls_check_rendering_state(Some(State::Default)),
            Some(State::Default)
        );
    }

    #[test]
    fn early_pack_status_send_gates_on_config_and_connection() {
        let mut sent = false;
        ServerPackManagerMixin::early_resource_pack_status_send(true, true, &mut sent);
        assert!(sent);

        let mut sent2 = false;
        ServerPackManagerMixin::early_resource_pack_status_send(true, false, &mut sent2);
        assert!(!sent2);

        let mut sent3 = false;
        ServerPackManagerMixin::early_resource_pack_status_send(false, true, &mut sent3);
        assert!(!sent3);
    }
}

/// Upstream `mixins/workaround`: the eight read this fire. `ReloadableResourceManagerMixin`
/// is still open.
pub mod workaround {
    use super::*;
    use crate::utils::texture_utils;

    /// Upstream: `workaround/CommandEncoderMixin.java`. The render-pass guard
    /// is bypassed while rrls owns the rendering state.
    pub struct CommandEncoderMixin;

    impl CommandEncoderMixin {
        /// `rrls$exitRenderPass`: `!isCurrentRenderingState() && original`.
        pub fn rrls_exit_render_pass(live_state: Option<State>, original: bool) -> bool {
            !overlay_helper::is_current_rendering_state(live_state) && original
        }
    }

    /// Upstream: `workaround/CompilationCacheMixin.java`. Used to prevent the
    /// game from crashing if rrls failed to load the reloader early.
    pub struct CompilationCacheMixin;

    impl CompilationCacheMixin {
        /// `rrls$suppressMissingCache`: Ok(Some) is the loaded post chain,
        /// Ok(None) is the suppressed null return, Err rethrows.
        pub fn rrls_suppress_missing_cache<T, E>(
            lookup: impl FnOnce() -> Result<T, E>,
            live_state: Option<State>,
            log: &mut Vec<String>,
        ) -> Result<Option<T>, E> {
            match lookup() {
                Ok(t) => Ok(Some(t)),
                Err(e) => {
                    if overlay_helper::is_current_rendering_state(live_state) {
                        texture_utils::log_warn("Failed to compile!");
                        log.push("Failed to compile!".to_string());
                        Ok(None)
                    } else {
                        Err(e)
                    }
                }
            }
        }
    }

    /// Upstream: `workaround/TextureManagerMixin.java`. After the vanilla
    /// register, a reloadable texture registered while rrls is rendering is
    /// reloaded immediately through the texture utils.
    pub struct TextureManagerMixin;

    impl TextureManagerMixin {
        /// `rrls$earlyRegister` (on `registerForNextReload`'s self-register
        /// call). True after the reload attempt, mirroring the original call.
        pub fn rrls_early_register(
            registered: bool,
            texture_reloadable: bool,
            live_state: Option<State>,
            pending: &texture_utils::PendingTextures,
            path: &texture_utils::Identifier,
        ) -> bool {
            let _ = registered; // original.call(instance, path, texture)
            if overlay_helper::is_current_rendering_state(live_state) && texture_reloadable {
                let _ = pending.reload_texture(path.clone());
            }
            true
        }
    }

    /// Upstream: `workaround/EntityRenderDispatcherMixin.java`. Lets mods that
    /// render entities in the title screen (other than essential) survive the
    /// missing level.
    pub struct EntityRenderDispatcherMixin {
        /// `RRLS$MINECRAFT.level == null`.
        pub level_is_none: bool,
    }

    impl EntityRenderDispatcherMixin {
        /// `rrls$workaroundEntityCrash` on the `getRenderer` call: Ok(None) is
        /// the suppressed null renderer, Err rethrows.
        pub fn rrls_workaround_entity_crash_renderer<T>(
            &self,
            lookup: impl FnOnce() -> Result<T, ()>,
            live_state: Option<State>,
        ) -> Result<Option<T>, ()> {
            match lookup() {
                Ok(t) => Ok(Some(t)),
                Err(()) => {
                    if overlay_helper::is_current_rendering_state(live_state) && self.level_is_none {
                        Ok(None)
                    } else {
                        Err(())
                    }
                }
            }
        }

        /// `rrls$workaroundEntityCrash` on the `CrashReport.forThrowable`
        /// call: true runs the original report, false cancels it.
        pub fn rrls_workaround_entity_crash_report(
            &self,
            reason: &str,
            live_state: Option<State>,
            log: &mut Vec<String>,
        ) -> bool {
            if overlay_helper::is_current_rendering_state(live_state) && self.level_is_none {
                let line = format!("Preventing: {}!", reason);
                texture_utils::log_warn(&line);
                log.push(line);
                false
            } else {
                true
            }
        }
    }

    /// Upstream: `workaround/GuiGraphicsExtractorMixin.java`.
    pub struct GuiGraphicsExtractorMixin;

    impl GuiGraphicsExtractorMixin {
        /// `rrls$fixSpriteCrash`: a sprite lookup failure in the two
        /// `blitSprite` overloads is cancelled outright (no state gate).
        pub fn rrls_fix_sprite_crash<T, E>(lookup: impl FnOnce() -> Result<T, E>) -> Result<Option<T>, ()> {
            match lookup() {
                Ok(t) => Ok(Some(t)),
                Err(_) => Ok(None),
            }
        }
    }

    /// Upstream: `workaround/textures/AbstractTextureMixin.java`.
    #[derive(Default)]
    pub struct AbstractTextureMixin {
        /// @Shadow `texture == null`.
        pub texture_is_none: bool,
        /// @Shadow `textureView == null`.
        pub texture_view_is_none: bool,
        /// `this instanceof ReloadableTexture`.
        pub is_reloadable: bool,
    }

    impl AbstractTextureMixin {
        /// `rrls$useMissingTexture` (HEAD, cancellable): a null texture on a
        /// reloadable texture forces a sync reload, and anything still null
        /// substitutes the missing texture (false cancels the original).
        pub fn rrls_use_missing_texture(
            &mut self,
            pending: &texture_utils::PendingTextures,
            path: &texture_utils::Identifier,
            loader: impl FnOnce() -> Result<texture_utils::TextureContents, ()>,
        ) -> bool {
            if self.texture_is_none && self.is_reloadable {
                let _ = pending.reload_texture_sync(path.clone(), loader);
                self.texture_is_none = false; // host seam: the reload fills the shadowed field
            }
            if self.texture_is_none {
                return false;
            }
            true
        }

        /// `rrls$useMissingTextureView`: same guard on the texture view.
        pub fn rrls_use_missing_texture_view(
            &mut self,
            pending: &texture_utils::PendingTextures,
            path: &texture_utils::Identifier,
            loader: impl FnOnce() -> Result<texture_utils::TextureContents, ()>,
        ) -> bool {
            if self.texture_view_is_none && self.is_reloadable {
                let _ = pending.reload_texture_sync(path.clone(), loader);
                self.texture_view_is_none = false;
            }
            if self.texture_view_is_none {
                return false;
            }
            true
        }
    }

    /// Upstream: `workaround/opengl/GlCommandEncoderMixin.java`. Upstream ships
    /// its body entirely commented out (`rrls$safeSetup`), so the 1:1 port
    /// keeps only the empty mixin.
    pub struct GlCommandEncoderMixin;

    /// Upstream: `workaround/vulkan/VulkanRenderPassMixin.java`. Upstream ships
    /// its body entirely commented out (`rrls$safeSetPipeline`, `rrls$safeDraw`,
    /// `rrls$handleVulkanThrow`), so the 1:1 port keeps only the empty mixin.
    pub struct VulkanRenderPassMixin;

    /// Upstream: `workaround/ReloadableResourceManagerMixin.java`, the last of
    /// the nine. Executors and listener kinds are the host seams; Horizon
    /// supplies the manager state and the `Minecraft` statics at the call site.
    pub enum Executor {
        /// `Util.backgroundExecutor()`
        Background,
        /// the game executor (`RRLS$MINECRAFT`)
        Game,
    }

    /// The three `BiConsumer` shapes the upstream call sites pass.
    pub enum ReloadCallback {
        /// `(_, throwable) -> future.completeExceptionally / complete(null)`
        CompleteFuture,
        /// `(_, _) -> rrls$refreshScreen()`
        RefreshScreen,
        /// `(_, _) -> {}`
        Noop,
    }

    /// The `instanceof` chain the init gate runs.
    #[derive(Clone, Debug, PartialEq)]
    pub enum ListenerKind {
        Font,
        Language,
        Splash,
        Atlas,
        Shader,
        Other,
    }

    /// One listener passing through the mixin, with the host's reload result.
    #[derive(Clone)]
    pub struct ReloadListenerCall {
        /// `listener.getName()`
        pub name: String,
        pub kind: ListenerKind,
        /// host seam: whether `listener.reload(...)` completes cleanly
        pub reload_succeeds: bool,
    }

    /// Host seam: the manager and listener state the mixin reads, plus the
    /// `Minecraft` statics and config flags it consults.
    #[derive(Default)]
    pub struct MinecraftSeam {
        pub live_state: Option<State>,
        /// `resources.getNamespaces().size()` (empty or `< 2` is the EBE
        /// workaround branch)
        pub namespace_count: usize,
        /// `!fontManager.fontSets.containsKey(Minecraft.DEFAULT_FONT)`
        pub default_font_missing: bool,
        /// `languageManager.getLanguages().size() <= 1`
        pub language_count: usize,
        /// `splashManager.splashes.isEmpty()`
        pub splashes_empty: bool,
        /// `atlasManager.spriteLookup.isEmpty()`
        pub sprite_lookup_empty: bool,
        /// `shaderManager.postChains.configs == ShaderManager.Configs.EMPTY`
        pub shader_configs_empty: bool,
        /// `RrlsConfig.INSTANCE.reInitScreen()`
        pub reinit_screen: bool,
        /// `RRLS$MINECRAFT.gui.screen() != null`, with the window's scaled size
        pub open_screen: Option<(u32, u32)>,
        /// screen re-inits performed by the refresh callbacks
        pub refreshed: usize,
    }

    /// Upstream: `workaround/ReloadableResourceManagerMixin.java`.
    pub struct ReloadableResourceManagerMixin {
        /// @Shadow `resources` (a live `CloseableResourceManager`)
        pub resources: crate::rrls::ResourceManager,
        /// `rrls$listeners`: quick-reload failures queued for `createReload`
        pub listeners: Vec<ReloadListenerCall>,
    }

    impl Default for ReloadableResourceManagerMixin {
        fn default() -> Self {
            Self {
                resources: crate::rrls::ResourceManager {},
                listeners: Vec::new(),
            }
        }
    }

    impl ReloadableResourceManagerMixin {
        /// `rrls$reloadListener`: Ok(count) after a clean quick reload,
        /// Err(()) queues the listener for the next `createReload`.
        pub fn rrls_reload_listener(
            &mut self,
            call: &ReloadListenerCall,
            executor: Executor,
            callback: ReloadCallback,
            seam: &mut MinecraftSeam,
            log: &mut Vec<String>,
        ) -> Result<usize, ()> {
            let _ = executor; // host seam: the future's completion executor
            log.push(format!("Quick reload listener '{}'", call.name));
            if call.reload_succeeds {
                if matches!(callback, ReloadCallback::RefreshScreen)
                    && seam.reinit_screen
                    && seam.open_screen.is_some()
                {
                    seam.refreshed += 1; // screen().init(guiScaledWidth, guiScaledHeight)
                }
                Ok(1)
            } else {
                self.listeners.push(call.clone());
                let line = format!("Failed to reload {}!", call.name);
                texture_utils::log_warn(&line);
                log.push(line);
                Err(())
            }
        }

        /// `rrls$initReloader` (TAIL of `registerReloadListener`): rebuilds a
        /// degenerate manager, quick-reloads the service's early mod listeners
        /// into the aggregate future, then runs the five listener-kind gates.
        pub fn rrls_init_reloader(
            &mut self,
            call: &ReloadListenerCall,
            seam: &mut MinecraftSeam,
            mod_reloaders: &mut ReloadersFuture,
            service: &dyn crate::services::RrlsReloaderService,
            mod_reload_succeeds: &dyn Fn() -> bool,
            atlas: Option<(&crate::utils::atlas_utils::PendingAtlases, &crate::utils::atlas_utils::AtlasEntry)>,
            log: &mut Vec<String>,
        ) -> usize {
            if !overlay_helper::is_current_rendering_state(seam.live_state) {
                return 0;
            }

            let mut reloaded = 0;

            // EBE workaround: an empty or single-namespace manager is rebuilt.
            if seam.namespace_count < 2 {
                log.push("Creating new resource manager!".to_string());
                seam.namespace_count = 2; // host seam: MultiPackResourceManager(openAllSelected())

                let mut early: Vec<crate::rrls::PreparableReloadListener> = Vec::new();
                service.collect_early_reloaders(&self.resources, &mut |l| early.push(l));
                let mut futures = 0usize;
                for (i, _) in early.into_iter().enumerate() {
                    let mod_call = ReloadListenerCall {
                        name: format!("mod#{}", i),
                        kind: ListenerKind::Other,
                        reload_succeeds: mod_reload_succeeds(),
                    };
                    if self
                        .rrls_reload_listener(&mod_call, Executor::Background, ReloadCallback::CompleteFuture, seam, log)
                        .is_ok()
                    {
                        futures += 1;
                    }
                }
                if futures > 0 {
                    mod_reloaders.set_new(None); // Rrls.MOD_RELOADERS_FUTURE.set(allOf(futures))
                }
                reloaded += futures;
            }

            match &call.kind {
                ListenerKind::Font if seam.default_font_missing => {
                    reloaded += self
                        .rrls_reload_listener(call, Executor::Game, ReloadCallback::RefreshScreen, seam, log)
                        .unwrap_or(0);
                }
                ListenerKind::Language if seam.language_count <= 1 => {
                    reloaded += self
                        .rrls_reload_listener(call, Executor::Background, ReloadCallback::Noop, seam, log)
                        .unwrap_or(0);
                }
                ListenerKind::Splash if seam.splashes_empty => {
                    reloaded += self
                        .rrls_reload_listener(call, Executor::Background, ReloadCallback::RefreshScreen, seam, log)
                        .unwrap_or(0);
                }
                ListenerKind::Atlas if seam.sprite_lookup_empty => {
                    if let Some((pending, entry)) = atlas {
                        let _ = pending.reload_atlas(entry);
                        reloaded += 1;
                    }
                }
                ListenerKind::Shader if seam.shader_configs_empty => {
                    reloaded += self
                        .rrls_reload_listener(call, Executor::Game, ReloadCallback::Noop, seam, log)
                        .unwrap_or(0);
                }
                _ => {}
            }
            reloaded
        }

        /// `rrls$initReloaders` (before `SimpleReloadInstance.create` in
        /// `createReload`): every queued listener gets a last quick reload,
        /// then the queue clears either way.
        pub fn rrls_init_reloaders(
            &mut self,
            seam: &mut MinecraftSeam,
            log: &mut Vec<String>,
        ) -> usize {
            let queued = std::mem::take(&mut self.listeners);
            let mut reloaded = 0;
            for call in &queued {
                let _ = self
                    .rrls_reload_listener(call, Executor::Game, ReloadCallback::Noop, seam, log);
                reloaded += 1; // the attempt runs either way
            }
            self.listeners.clear(); // upstream clears the whole set after the loop
            reloaded
        }
    }
}

#[cfg(test)]
mod workaround_tests {
    use super::workaround::*;
    use super::*;
    use crate::utils::texture_utils::{PendingTextures, TextureContents};

    #[test]
    fn exit_render_pass_follows_the_rendering_state() {
        // Hide is the rendering state; Default is not.
        assert!(!CommandEncoderMixin::rrls_exit_render_pass(Some(State::Hide), true));
        assert!(CommandEncoderMixin::rrls_exit_render_pass(Some(State::Default), true));
        assert!(!CommandEncoderMixin::rrls_exit_render_pass(Some(State::Default), false));
    }

    #[test]
    fn suppress_missing_cache_gates_on_rendering() {
        let mut log = Vec::new();
        let ok = CompilationCacheMixin::rrls_suppress_missing_cache(|| Ok::<u8, ()>(7), None, &mut log);
        assert!(matches!(ok, Ok(Some(7))) && log.is_empty());

        let mut log = Vec::new();
        let suppressed = CompilationCacheMixin::rrls_suppress_missing_cache(|| Err::<u8, ()>(()), Some(State::Hide), &mut log);
        assert!(matches!(suppressed, Ok(None)) && log.len() == 1);

        let mut log = Vec::new();
        let rethrown = CompilationCacheMixin::rrls_suppress_missing_cache(|| Err::<u8, ()>(()), Some(State::Default), &mut log);
        assert!(rethrown.is_err() && log.is_empty());
    }

    #[test]
    fn early_register_reloads_reloadable_textures_while_rendering() {
        let pending = PendingTextures::default();
        // Rendering + reloadable: the early reload runs.
        assert!(TextureManagerMixin::rrls_early_register(
            true, true, Some(State::Hide), &pending, &"textures/test.png".to_string(),
        ));
        // Outside rendering: no entry is created.
        assert!(TextureManagerMixin::rrls_early_register(
            true, true, Some(State::Default), &pending, &"textures/other.png".to_string(),
        ));
        assert!(!pending.is_pending("textures/other.png"));
    }

    #[test]
    fn entity_crash_only_suppressed_without_a_level() {
        let m = EntityRenderDispatcherMixin { level_is_none: true };
        let suppressed = m.rrls_workaround_entity_crash_renderer(|| Err::<u8, ()>(()), Some(State::Hide));
        assert!(matches!(suppressed, Ok(None)));

        let m = EntityRenderDispatcherMixin { level_is_none: false };
        let rethrown = m.rrls_workaround_entity_crash_renderer(|| Err::<u8, ()>(()), Some(State::Hide));
        assert!(rethrown.is_err());

        let m = EntityRenderDispatcherMixin { level_is_none: true };
        let mut log = Vec::new();
        assert!(!m.rrls_workaround_entity_crash_report("crashed", Some(State::Hide), &mut log));
        assert!(log[0].starts_with("Preventing:"));
        let mut log = Vec::new();
        assert!(m.rrls_workaround_entity_crash_report("crashed", Some(State::Default), &mut log));
        assert!(log.is_empty());
    }

    #[test]
    fn sprite_crash_is_cancelled_outright() {
        let ok = GuiGraphicsExtractorMixin::rrls_fix_sprite_crash(|| Ok::<u8, ()>(3));
        assert!(matches!(ok, Ok(Some(3))));
        let cancelled = GuiGraphicsExtractorMixin::rrls_fix_sprite_crash(|| Err::<u8, ()>(()));
        assert!(matches!(cancelled, Ok(None)));
    }

    #[test]
    fn missing_texture_reload_and_substitute() {
        let pending = PendingTextures::default();
        // Reloadable + null: sync reload runs and the texture fills in.
        let mut m = AbstractTextureMixin { texture_is_none: true, is_reloadable: true, ..Default::default() };
        assert!(m.rrls_use_missing_texture(&pending, &"textures/reloadable.png".to_string(), || Ok(TextureContents::Loaded)));
        // Non-reloadable + null: cancelled, missing texture substitutes.
        let mut m = AbstractTextureMixin { texture_is_none: true, is_reloadable: false, ..Default::default() };
        assert!(!m.rrls_use_missing_texture(&pending, &"textures/missing.png".to_string(), || Err(())));
        // Already present: proceeds untouched.
        let mut m = AbstractTextureMixin::default();
        assert!(m.rrls_use_missing_texture_view(&pending, &"textures/present.png".to_string(), || Err(())));
    }

    struct TwoListenerService;

    impl crate::services::AdvancedService for TwoListenerService {
        fn is_service_active(&self) -> bool {
            true
        }
    }

    impl crate::services::RrlsReloaderService for TwoListenerService {
        fn collect_early_reloaders(
            &self,
            _manager: &crate::rrls::ResourceManager,
            consumer: &mut dyn FnMut(crate::rrls::PreparableReloadListener),
        ) {
            consumer(crate::rrls::PreparableReloadListener);
            consumer(crate::rrls::PreparableReloadListener);
        }
    }

    fn mk_seam(live: Option<State>) -> MinecraftSeam {
        MinecraftSeam {
            live_state: live,
            ..Default::default()
        }
    }

    #[test]
    fn init_reloader_gates_on_rendering_state() {
        let mut m = ReloadableResourceManagerMixin::default();
        let mut seam = mk_seam(Some(State::Default));
        let mut mr = ReloadersFuture::new();
        let mut log = Vec::new();
        let svc = TwoListenerService;
        let call = ReloadListenerCall { name: "font".into(), kind: ListenerKind::Font, reload_succeeds: true };
        let n = m.rrls_init_reloader(&call, &mut seam, &mut mr, &svc, &|| true, None, &mut log);
        assert_eq!(n, 0);
        assert!(log.is_empty());
    }

    #[test]
    fn ebe_branch_rebuilds_manager_and_quick_reloads_mods() {
        let mut m = ReloadableResourceManagerMixin::default();
        let mut seam = mk_seam(Some(State::Hide));
        seam.namespace_count = 1;
        let mut mr = ReloadersFuture::new();
        let mut log = Vec::new();
        let svc = TwoListenerService;
        let call = ReloadListenerCall { name: "other".into(), kind: ListenerKind::Other, reload_succeeds: true };
        let _ = m.rrls_init_reloader(&call, &mut seam, &mut mr, &svc, &|| true, None, &mut log);
        assert!(log.iter().any(|l| l.contains("Creating new resource manager!")));
        assert_eq!(log.iter().filter(|l| l.contains("Quick reload listener")).count(), 2);
        assert!(mr.get_and_set_null(), "the aggregate mod-reloaders future was set");
        assert_eq!(seam.namespace_count, 2);
    }

    #[test]
    fn listener_kind_gates_fire_the_five_reloads() {
        let mut m = ReloadableResourceManagerMixin::default();
        let mut seam = mk_seam(Some(State::Hide));
        seam.namespace_count = 2; // populated manager: the EBE branch stays out
        seam.default_font_missing = true;
        seam.language_count = 1;
        seam.splashes_empty = true;
        seam.sprite_lookup_empty = true;
        seam.shader_configs_empty = true;
        seam.reinit_screen = true;
        seam.open_screen = Some((320, 240));
        let mut mr = ReloadersFuture::new();
        let mut log = Vec::new();
        let pending = crate::utils::atlas_utils::PendingAtlases::default();
        let entry = crate::utils::atlas_utils::AtlasEntry {
            definition_location: "atlas/gui".to_string(),
            upload: Box::new(|_| {}),
            max_mipmap_levels: 0,
            schedule_load: Box::new(|_| Ok(crate::utils::atlas_utils::Preparations::Ready)),
        };

        for (kind, name) in [
            (ListenerKind::Font, "font"),
            (ListenerKind::Language, "language"),
            (ListenerKind::Splash, "splash"),
            (ListenerKind::Atlas, "atlas"),
            (ListenerKind::Shader, "shader"),
        ] {
            let call = ReloadListenerCall { name: name.into(), kind, reload_succeeds: true };
            let n = m.rrls_init_reloader(&call, &mut seam, &mut mr, &TwoListenerService, &|| true, Some((&pending, &entry)), &mut log);
            assert_eq!(n, 1, "{} gate should reload", name);
        }
        // Font and Splash refresh the screen; the others do not.
        assert_eq!(seam.refreshed, 2);

        // A populated manager skips every gate.
        let mut seam2 = mk_seam(Some(State::Hide));
        seam2.namespace_count = 2; // the rebuilt manager keeps its namespaces
        seam2.default_font_missing = false;
        seam2.language_count = 2;
        seam2.splashes_empty = false;
        seam2.sprite_lookup_empty = false;
        seam2.shader_configs_empty = false;
        let mut log2 = Vec::new();
        let call = ReloadListenerCall { name: "font".into(), kind: ListenerKind::Font, reload_succeeds: true };
        assert_eq!(m.rrls_init_reloader(&call, &mut seam2, &mut ReloadersFuture::new(), &TwoListenerService, &|| true, None, &mut log2), 0);
        assert!(log2.is_empty());
    }

    #[test]
    fn failed_quick_reloads_queue_for_create_reload() {
        let mut m = ReloadableResourceManagerMixin::default();
        let mut seam = mk_seam(Some(State::Hide));
        let mut log = Vec::new();
        let call = ReloadListenerCall { name: "laggy".into(), kind: ListenerKind::Other, reload_succeeds: false };
        assert!(m.rrls_reload_listener(&call, Executor::Game, ReloadCallback::Noop, &mut seam, &mut log).is_err());
        assert!(log.iter().any(|l| l.contains("Failed to reload laggy!")));
        assert_eq!(m.listeners.len(), 1);

        // createReload: every queued listener reloads once, then clears.
        let mut log2 = Vec::new();
        let n = m.rrls_init_reloaders(&mut seam, &mut log2);
        assert_eq!(n, 1);
        assert!(m.listeners.is_empty());
    }
}
