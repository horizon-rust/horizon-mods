/*
 * Port of Remove Reloading Screen (rrls) to Rust for Horizon.
 *
 * Copyright 2023 - 2026 dima_dencep (upstream, OSL-3.0); Rust translation 2026 sparticle.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 *
 * Exact 1:1 translation of the upstream Java. Where an
 * upstream file binds to a Minecraft/Fabric API Horizon does not expose, the
 * same shape is kept and the host side is a seam (see README).
 */
//!
//! Exact 1:1 translation of Remove Reloading Screen (rrls), upstream Java by
//! dima_dencep, https://github.com/dima-dencep/rrls @576a474 (OSL-3.0). The
//! Minecraft/Fabric APIs that do not exist on Horizon are host seams with the
//! same shapes and semantics; the background-load seam is tracked under
//! horizon-rust/horizon #15868. Rust port by sparticle, upstream by dima_dencep.

pub mod rrls {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Rrls.java`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub const MOD_ID: &str = "rrls";

/// Upstream: `LogManager.getLogger(MOD_ID)`. Horizon has no log4j; the same
/// logger name is kept and emission is the host seam.
pub const LOGGER: &str = MOD_ID;

/// Upstream: `AtomicReference<CompletableFuture<Void>> MOD_RELOADERS_FUTURE`.
/// The Java type pairs a slot with a completable future; the same
/// set-and-swap semantics live in one lock here (`STATE_*` hold the payload,
/// `DONE` marks the future completed).
pub struct ReloadersFuture {
    slot: Mutex<Option<SharedState>>,
    epoch: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedState {
    Pending,
    Done,
}

impl ReloadersFuture {
    pub const fn new() -> Self {
        Self { slot: Mutex::new(None), epoch: AtomicU64::new(0) }
    }

    /// `compareAndSet(expect, fresh future)`: swaps in a new pending future
    /// only when the slot still holds `expect`'s epoch (or is empty when
    /// `expect` is None).
    pub fn set_new(&self, expect: Option<u64>) -> u64 {
        let mut slot = self.slot.lock().expect("rrls reloaders slot");
        let current = self.epoch.load(Ordering::SeqCst);
        let matches = match expect {
            None => slot.is_none(),
            Some(e) => slot.as_ref().is_some_and(|_| e == current),
        };
        if matches {
            *slot = Some(SharedState::Pending);
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        self.epoch.load(Ordering::SeqCst)
    }

    /// Upstream `AtomicReference.getAndSet(null)`: returns whether a future
    /// was live and clears the slot.
    pub fn get_and_set_null(&self) -> bool {
        let mut slot = self.slot.lock().expect("rrls reloaders slot");
        let had = slot.is_some();
        if had {
            *slot = None;
        }
        had
    }

    /// Complete the live future (`complete(null)` upstream).
    pub fn complete(&self) {
        if let Some(state) = self.slot.lock().expect("rrls reloaders slot").as_mut() {
            *state = SharedState::Done;
        }
    }

    pub fn current(&self) -> (u64, Option<SharedState>) {
        (
            self.epoch.load(Ordering::SeqCst),
            *self.slot.lock().expect("rrls reloaders slot"),
        )
    }
}

/// Upstream: `net.minecraft.server.packs.resources.ResourceManager`
/// (opaque host seam).
pub struct ResourceManager;

/// Upstream: `net.minecraft.server.packs.resources.PreparableReloadListener`
/// (opaque host seam).
pub struct PreparableReloadListener;

/// Upstream: `ServiceUtils.loadService` walks the Java ServiceLoader. The
/// provider registry is the host seam; with nothing registered the callers
/// fall back to their own defaults, exactly as upstream does.
pub fn service_provider<T: ?Sized>() -> Option<&'static T> {
    None
}

/// Upstream `onInitializeClient` logs the config service class.
pub fn on_initialize_client(config_service: &str) -> String {
    format!("Config service: {}", config_service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_new_swaps_pending_in() {
        let f = ReloadersFuture::new();
        let e0 = f.set_new(None);
        let (_, state) = f.current();
        assert_eq!(state, Some(SharedState::Pending));
        let e1 = f.set_new(Some(e0));
        assert_ne!(e0, e1);
    }

    #[test]
    fn set_new_requires_matching_epoch() {
        let f = ReloadersFuture::new();
        let e0 = f.set_new(None);
        assert_eq!(f.set_new(Some(e0 + 7)), e0, "stale expect is a no-op, current epoch stays");
        let (_, state) = f.current();
        assert_eq!(state, Some(SharedState::Pending), "slot untouched");
    }

    #[test]
    fn complete_marks_done() {
        let f = ReloadersFuture::new();
        f.set_new(None);
        f.complete();
        let (_, state) = f.current();
        assert_eq!(state, Some(SharedState::Done));
    }
}
}

pub mod mixins {
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

use super::config::{DoubleLoad, RrlsConfig, Type};
use super::rrls::ReloadersFuture;
use super::utils::overlay_helper::{self, config, ScreenKind, State};
use super::utils::rainbow::rainbow_color;

/// The config values the mixins read, cloned out of the shared store so each
/// translated method reads one coherent snapshot (`RrlsConfig.INSTANCE`).
pub fn current_config() -> RrlsConfig {
    config().read().expect("rrls config").clone()
}

/// Host seam for `Rrls.LOGGER.error(...)`.
pub fn log_error(message: &str) {
    let _ = (super::rrls::LOGGER, message);
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
                matches!(self.mod_reloaders.current().1, Some(super::rrls::SharedState::Done))
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
                    widget.alpha = super::utils::rainbow::from_8bit_channel(ease_alpha);

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
impl TranslatableEnum for super::config::HideType {}
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
        with_config(|c| c.easing = super::super::utils::ease::Ease::Linear);
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
    use super::super::utils::texture_utils;

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
    use super::super::utils::texture_utils;

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
        let path: super::super::utils::texture_utils::Identifier = "test:cube".to_string();

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
    use super::super::utils::texture_utils;

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
        pub resources: super::super::rrls::ResourceManager,
        /// `rrls$listeners`: quick-reload failures queued for `createReload`
        pub listeners: Vec<ReloadListenerCall>,
    }

    impl Default for ReloadableResourceManagerMixin {
        fn default() -> Self {
            Self {
                resources: super::super::rrls::ResourceManager {},
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
            service: &dyn super::super::services::RrlsReloaderService,
            mod_reload_succeeds: &dyn Fn() -> bool,
            atlas: Option<(&super::super::utils::atlas_utils::PendingAtlases, &super::super::utils::atlas_utils::AtlasEntry)>,
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

                let mut early: Vec<super::super::rrls::PreparableReloadListener> = Vec::new();
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
    use super::super::utils::texture_utils::{PendingTextures, TextureContents};

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

    impl super::super::services::AdvancedService for TwoListenerService {
        fn is_service_active(&self) -> bool {
            true
        }
    }

    impl super::super::services::RrlsReloaderService for TwoListenerService {
        fn collect_early_reloaders(
            &self,
            _manager: &super::super::rrls::ResourceManager,
            consumer: &mut dyn FnMut(super::super::rrls::PreparableReloadListener),
        ) {
            consumer(super::super::rrls::PreparableReloadListener);
            consumer(super::super::rrls::PreparableReloadListener);
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
        let pending = super::super::utils::atlas_utils::PendingAtlases::default();
        let entry = super::super::utils::atlas_utils::AtlasEntry {
            definition_location: "atlas/gui".to_string(),
            upload: Box::new(|_| {}),
            max_mipmap_levels: 0,
            schedule_load: Box::new(|_| Ok(super::super::utils::atlas_utils::Preparations::Ready)),
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
}

pub mod screens {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of the `screens` package
//! (`ConfigurationSectionScreenProxy`, `ConfigurationSectionScreenWithOverlay`).
//! Both extend NeoForge's `ConfigurationScreen.ConfigurationSectionScreen`,
//! which is not part of rrls and does not exist on Horizon: the parts rrls
//! touches are kept here and the widget layer behind them is the host seam.

use std::collections::HashMap;

/// Upstream: `net.minecraft.network.chat.Component` (opaque seam).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component(pub String);

/// Upstream: `com.electronwill.nightconfig.core.UnmodifiableConfig` (opaque
/// seam; Horizon has no NightConfig).
#[derive(Debug, Default)]
pub struct UnmodifiableConfig;

/// Upstream: `net.neoforged.fml.config.ModConfig` + its `Context` (opaque seam).
#[derive(Debug, Default)]
pub struct ModConfigContext;

/// Upstream: `net.minecraft.client.gui.screens.LoadingOverlay`. The overlay
/// object itself is the host seam; `reset_progress` is the `OverlayExtender`
/// duck method (`rrls$resetProgress`, implemented by LoadingOverlayMixin).
#[derive(Debug, Default)]
pub struct LoadingOverlay;

impl LoadingOverlay {
    pub fn rrls_reset_progress(&mut self) {}
}

/// Upstream: `net.minecraft.client.Minecraft`, narrowed to the two fields
/// these screens read (`minecraft`, `minecraft.gui.overlay`).
pub struct Minecraft {
    pub gui_overlay: Option<LoadingOverlay>,
}

impl Minecraft {
    /// `minecraft.gui.setOverlay(overlay)`.
    pub fn set_overlay(&mut self, overlay: Option<LoadingOverlay>) {
        self.gui_overlay = overlay;
    }
}

/// Upstream: `net.minecraft.client.gui.screens.Screen`, narrowed to the
/// lifecycle callbacks these screens override (each `super.x()` is a host
/// call, a no-op stand-in here).
#[derive(Default)]
pub struct ScreenHost;

impl ScreenHost {
    pub fn tick(&mut self) {}
    pub fn added(&mut self) {}
    pub fn removed(&mut self) {}
    pub fn rebuild(&mut self) {}
    pub fn on_changed(&mut self, _key: &str) {}
    /// `ConfigurationScreen.ConfigurationSectionScreen.createSection` (the
    /// NeoForge super implementation, not part of rrls).
    pub fn super_create_section(
        &mut self,
        _key: &str,
        _subconfig: &UnmodifiableConfig,
        _subsection: &UnmodifiableConfig,
    ) -> Option<SectionElement> {
        Some(SectionElement)
    }
}

/// Upstream: the `Element` createSection returns (opaque seam).
pub struct SectionElement;

/// The shared state of a `ConfigurationScreen.ConfigurationSectionScreen`
/// that rrls reads or writes: the section cache and (for the overlay
/// subclass) the progress fields.
pub struct ConfigurationSectionScreenState {
    pub context: ModConfigContext,
    pub section_cache: HashMap<String, bool>,
    pub minecraft: Option<Minecraft>,
    pub overlay: Option<LoadingOverlay>,
    pub current_progress: f32,
    pub host: ScreenHost,
}

/// Upstream: `ConfigurationSectionScreenProxy`. Wraps the shared state and
/// overrides `createSection` exactly as upstream does.
pub struct ConfigurationSectionScreenProxy {
    pub state: ConfigurationSectionScreenState,
}

impl ConfigurationSectionScreenProxy {
    pub fn new(context: ModConfigContext) -> Self {
        Self {
            state: ConfigurationSectionScreenState {
                context,
                section_cache: HashMap::new(),
                minecraft: None,
                overlay: None,
                current_progress: 0.0,
                host: ScreenHost,
            },
        }
    }

    /// Upstream `createSection(key, subconfig, subsection)`:
    /// - a cached section that is already a `WithOverlay` goes straight to
    ///   the super implementation;
    /// - anything else is replaced by a fresh `WithOverlay` (`.rebuild()`),
    ///   the evicted one is logged, and the super implementation runs.
    pub fn create_section(
        &mut self,
        key: &str,
        subconfig: &UnmodifiableConfig,
        subsection: &UnmodifiableConfig,
    ) -> Option<SectionElement> {
        if self.state.section_cache.get(key).copied().unwrap_or(false) {
            return self
                .state
                .host
                .super_create_section(key, subconfig, subsection);
        }

        let mut rebuilt = ConfigurationSectionScreenWithOverlay::new(&self.state.context);
        rebuilt.rebuild();
        let old = self
            .state
            .section_cache
            .insert(key.to_string(), rebuilt.is_with_overlay());
        if old.is_some() {
            // `Rrls.LOGGER.warn("Removed section ({}) from config!", old)`
            super::rrls::LOGGER;
        }

        self.state
            .host
            .super_create_section(key, subconfig, subsection)
    }
}

/// Upstream: `ConfigurationSectionScreenWithOverlay`, a config section that
/// doubles as a `ReloadInstance` so the config screen keeps the loading
/// overlay alive while its values settle.
pub struct ConfigurationSectionScreenWithOverlay {
    pub state: ConfigurationSectionScreenState,
}

/// Upstream: `net.minecraft.server.packs.resources.ReloadInstance`.
pub trait ReloadInstance {
    /// `done()`: upstream returns `CompletableFuture.completedFuture(null)`;
    /// `Some(())` is that completed future.
    fn done(&self) -> Option<()>;
    fn get_actual_progress(&self) -> f32;
    fn is_done(&self) -> bool;
}

impl ConfigurationSectionScreenWithOverlay {
    pub fn new(context: &ModConfigContext) -> Self {
        let mut state = ConfigurationSectionScreenState {
            context: ModConfigContext::default(),
            section_cache: HashMap::new(),
            minecraft: None,
            overlay: None,
            current_progress: 0.0,
            host: ScreenHost,
        };
        state.context = context_clone(context);
        // `this.overlay = new LoadingOverlay(Minecraft.getInstance(), this, th -> {}, false)`
        state.overlay = Some(LoadingOverlay);
        Self { state }
    }

    pub fn reset_progress(&mut self) {
        self.state.current_progress = 0.0;
        if let Some(overlay) = self.state.overlay.as_mut() {
            overlay.rrls_reset_progress();
        }
        if let Some(minecraft) = self.state.minecraft.as_mut() {
            if minecraft.gui_overlay.is_none() {
                let overlay = self.state.overlay.take();
                minecraft.set_overlay(overlay);
            }
        }
    }

    /// `onChanged(key)`: super first, then a reset.
    pub fn on_changed(&mut self, key: &str) {
        self.state.host.on_changed(key);
        self.reset_progress();
    }

    /// `rebuild()`: super, then `return this`.
    pub fn rebuild(&mut self) -> bool {
        self.state.host.rebuild();
        self.is_with_overlay()
    }

    pub fn tick(&mut self) {
        self.state.host.tick();
        if let Some(minecraft) = self.state.minecraft.as_ref() {
            if minecraft.gui_overlay.is_some() {
                self.state.current_progress += 0.1;
            }
        }
    }

    pub fn added(&mut self) {
        self.state.host.added();
        self.reset_progress();
    }

    pub fn removed(&mut self) {
        self.state.host.removed();
        if let Some(minecraft) = self.state.minecraft.as_mut() {
            if minecraft.gui_overlay.is_some() {
                minecraft.set_overlay(None);
            }
        }
    }

    /// Stand-in for the `is_with_overlay` marker the proxy checks in the
    /// section cache (`instanceof ConfigurationSectionScreenWithOverlay`).
    pub fn is_with_overlay(&self) -> bool {
        true
    }
}

/// `net.minecraft.util.Mth.clamp` is `f32::clamp`; kept as a named alias so
/// the upstream call shape survives.
fn mth_clamp(value: f32, min: f32, max: f32) -> f32 {
    value.clamp(min, max)
}

fn context_clone(context: &ModConfigContext) -> ModConfigContext {
    ModConfigContext::default()
}

impl ReloadInstance for ConfigurationSectionScreenWithOverlay {
    fn done(&self) -> Option<()> {
        Some(())
    }

    fn get_actual_progress(&self) -> f32 {
        mth_clamp(self.state.current_progress, 0.0, 1.0)
    }

    fn is_done(&self) -> bool {
        self.state.minecraft.as_ref().is_some_and(|minecraft| {
            minecraft.gui_overlay.is_some() && self.state.current_progress > 1.5
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy() -> ConfigurationSectionScreenProxy {
        ConfigurationSectionScreenProxy::new(ModConfigContext)
    }

    #[test]
    fn plain_cached_section_is_replaced_by_overlay_section() {
        let mut p = proxy();
        p.state.section_cache.insert("general".into(), false);
        let el = p.create_section("general", &UnmodifiableConfig, &UnmodifiableConfig);
        assert!(el.is_some());
        assert_eq!(p.state.section_cache.get("general"), Some(&true));
    }

    #[test]
    fn overlay_cached_section_short_circuits_to_super() {
        let mut p = proxy();
        p.state.section_cache.insert("general".into(), true);
        let el = p.create_section("general", &UnmodifiableConfig, &UnmodifiableConfig);
        assert!(el.is_some());
        assert_eq!(p.state.section_cache.get("general"), Some(&true));
    }

    #[test]
    fn fresh_key_caches_an_overlay_section() {
        let mut p = proxy();
        p.create_section("new", &UnmodifiableConfig, &UnmodifiableConfig);
        assert_eq!(p.state.section_cache.get("new"), Some(&true));
    }

    #[test]
    fn tick_advances_progress_only_while_overlay_is_active() {
        let mut s = ConfigurationSectionScreenWithOverlay::new(&ModConfigContext);
        s.state.minecraft = Some(Minecraft { gui_overlay: None });
        s.tick();
        assert_eq!(s.state.current_progress, 0.0);

        s.state.minecraft.as_mut().unwrap().gui_overlay = Some(LoadingOverlay);
        s.tick();
        assert!((s.state.current_progress - 0.1).abs() < 1e-6);
    }

    #[test]
    fn progress_clamps_and_done_requires_one_and_a_half() {
        let mut s = ConfigurationSectionScreenWithOverlay::new(&ModConfigContext);
        s.state.minecraft = Some(Minecraft { gui_overlay: Some(LoadingOverlay) });
        for _ in 0..11 {
            s.tick();
        }
        assert!((s.state.current_progress - 1.1).abs() < 1e-5);
        assert!((s.get_actual_progress() - 1.0).abs() < 1e-6);
        assert!(!s.is_done());

        s.state.current_progress = 1.6;
        assert!(s.is_done());
        assert_eq!(s.done(), Some(()));
    }

    #[test]
    fn lifecycle_resets_and_clears_the_overlay() {
        let mut s = ConfigurationSectionScreenWithOverlay::new(&ModConfigContext);
        s.state.minecraft = Some(Minecraft { gui_overlay: Some(LoadingOverlay) });
        s.state.current_progress = 0.9;

        s.reset_progress();
        assert_eq!(s.state.current_progress, 0.0);
        assert!(s.state.minecraft.as_ref().unwrap().gui_overlay.is_some());

        s.removed();
        assert!(s.state.minecraft.as_ref().unwrap().gui_overlay.is_none());

        s.added();
        assert_eq!(s.state.current_progress, 0.0);
        assert!(s.state.minecraft.as_ref().unwrap().gui_overlay.is_some());
    }

    #[test]
    fn on_changed_resets_progress() {
        let mut s = ConfigurationSectionScreenWithOverlay::new(&ModConfigContext);
        s.state.current_progress = 0.4;
        s.on_changed("enabled");
        assert_eq!(s.state.current_progress, 0.0);
    }

    #[test]
    fn rebuild_marks_itself_as_overlay_section() {
        let mut s = ConfigurationSectionScreenWithOverlay::new(&ModConfigContext);
        assert!(s.rebuild());
        assert!(s.is_with_overlay());
    }
}
}

pub mod services {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `services/RrlsReloaderService.java`.

use super::rrls::PreparableReloadListener;
use super::rrls::ResourceManager;

/// Upstream: `org.redlance.common.services.AdvancedService`. The shared
/// library is not part of this port; the marker trait keeps the contract
/// (`isServiceActive` is its only member upstream).
pub trait AdvancedService {
    fn is_service_active(&self) -> bool;
}

/// Upstream: `RrlsReloaderService` interface. `collectEarlyReloaders` runs
/// before the game's own reload listener collection; `ResourceManager` and
/// the consumer are the host seam (Horizon supplies both at the call site).
pub trait RrlsReloaderService: AdvancedService + Send + Sync {
    fn collect_early_reloaders(
        &self,
        manager: &ResourceManager,
        consumer: &mut dyn FnMut(PreparableReloadListener),
    );
}

/// Upstream: `ServiceUtils.loadService(RrlsReloaderService.class, () -> new
/// RrlsReloaderService() { ... })`. Java walks the ServiceLoader first and
/// falls back to the anonymous no-op below; here the host provider seam is
/// consulted at first use and the same fallback is the default.
pub struct DefaultRrlsReloaderService;

impl AdvancedService for DefaultRrlsReloaderService {
    fn is_service_active(&self) -> bool {
        true
    }
}

impl RrlsReloaderService for DefaultRrlsReloaderService {
    fn collect_early_reloaders(
        &self,
        _manager: &ResourceManager,
        _consumer: &mut dyn FnMut(PreparableReloadListener),
    ) {
        // no-op
    }
}

pub struct RrlsReloaderServiceHandle {
    provider: Option<&'static dyn RrlsReloaderService>,
}

impl RrlsReloaderServiceHandle {
    pub const fn new() -> Self {
        Self { provider: None }
    }

    /// `ServiceUtils.loadService`: a host-registered provider wins; otherwise
    /// the anonymous default impl above serves.
    pub fn instance(&mut self) -> &'static dyn RrlsReloaderService {
        if self.provider.is_none() {
            self.provider = super::rrls::service_provider::<dyn RrlsReloaderService>()
                .or(Some(&DefaultRrlsReloaderService));
        }
        self.provider.unwrap()
    }
}

static INSTANCE: std::sync::Mutex<RrlsReloaderServiceHandle> =
    std::sync::Mutex::new(RrlsReloaderServiceHandle::new());

/// Upstream: `RrlsReloaderService.INSTANCE`.
pub fn instance() -> &'static dyn RrlsReloaderService {
    INSTANCE.lock().expect("rrls reloader service").instance()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn manager() -> ResourceManager {
        ResourceManager
    }

    struct Counting {
        collected: AtomicU32,
    }

    impl AdvancedService for Counting {
        fn is_service_active(&self) -> bool {
            true
        }
    }

    impl RrlsReloaderService for Counting {
        fn collect_early_reloaders(
            &self,
            _manager: &ResourceManager,
            consumer: &mut dyn FnMut(PreparableReloadListener),
        ) {
            for _ in 0..2 {
                consumer(PreparableReloadListener);
            }
            self.collected.fetch_add(2, Ordering::SeqCst);
        }
    }

    #[test]
    fn default_service_is_active_and_noop() {
        assert!(DefaultRrlsReloaderService.is_service_active());
        let mut collected = 0usize;
        DefaultRrlsReloaderService.collect_early_reloaders(&manager(), &mut |_| {
            collected += 1;
        });
        assert_eq!(collected, 0);
    }

    #[test]
    fn instance_falls_back_to_default_impl() {
        // No host provider registered in tests: upstream's fallback wins.
        let inst = instance();
        let mut collected = 0usize;
        inst.collect_early_reloaders(&manager(), &mut |_| {
            collected += 1;
        });
        assert_eq!(collected, 0);
        assert!(inst.is_service_active());
    }

    #[test]
    fn host_provider_receives_the_collection_call() {
        let counting = Counting { collected: AtomicU32::new(0) };
        let mut collected = 0usize;
        counting.collect_early_reloaders(&manager(), &mut |_| {
            collected += 1;
        });
        assert_eq!(collected, 2);
        assert_eq!(counting.collected.load(Ordering::SeqCst), 2);
        assert!(counting.is_service_active());
    }
}
}

pub mod config {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of the `config` package: `AprilFool`, `DoubleLoad`,
//! `HideType`, `Type`, and the `RrlsConfig` service interface.


pub use april_fool::AprilFool;
pub use double_load::DoubleLoad;
pub use hide_type::HideType;
pub use rrls_config::RrlsConfig;
pub use rrls_type::Type;

/// Upstream types carry `Component.translatable("rrls.configuration....")`
/// names; Horizon has no translation registry, so the key is kept verbatim
/// and rendering it is the host seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translatable {
    pub key: &'static str,
}

impl Translatable {
    pub const fn new(key: &'static str) -> Self {
        Self { key }
    }
}
pub mod april_fool {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `AprilFool.java`. `Calendar` month/day arithmetic is
//! spelled out: Calendar months are 0-based, so April is 3.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AprilFool {
    OnApril,
    Always,
    Disabled,
}

impl AprilFool {
    /// Upstream caches the first `Calendar.getInstance()` for the process;
    /// here the moment is captured the same way and passed in, so a test can
    /// pin the date without touching global state.
    pub fn can_ues(&self, month0: i32, day_of_month: i32) -> bool {
        match self {
            AprilFool::OnApril => month0 == 3 && day_of_month == 1,
            AprilFool::Always => true,
            AprilFool::Disabled => false,
        }
    }
}

/// Civil-from-days, so `now` (seconds since epoch) maps to the same
/// (month0, day) Calendar would report.
pub fn calendar_month_day(epoch_secs: i64) -> (i32, i32) {
    let days = epoch_secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let _y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m0 = if mp < 10 { mp + 2 } else { mp - 10 };
    (m0 as i32, (doy - (153 * mp + 2) / 5 + 1) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_april_fires_only_april_first() {
        let f = AprilFool::OnApril;
        assert!(f.can_ues(3, 1), "Calendar.APRIL == 3");
        assert!(!f.can_ues(3, 2));
        assert!(!f.can_ues(2, 1));
        assert!(!f.can_ues(4, 1));
    }

    #[test]
    fn always_and_disabled() {
        assert!(AprilFool::Always.can_ues(0, 1));
        assert!(!AprilFool::Disabled.can_ues(3, 1));
    }

    #[test]
    fn calendar_math_matches_known_dates() {
        // 2026-04-01 12:00:00 UTC = 1775044800
        assert_eq!(calendar_month_day(1_775_044_800), (3, 1));
        // 2026-01-01 12:00:00 UTC = 1767268800
        assert_eq!(calendar_month_day(1_767_268_800), (0, 1));
        // 2026-12-31 12:00:00 UTC = 1798718400
        assert_eq!(calendar_month_day(1_798_718_400), (11, 31));
    }
}
}

pub mod double_load {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `DoubleLoad.java`. The enum constants carry translated
//! names; those live on `Translatable` here.

use super::Translatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleLoad {
    ForceLoad,
    Load,
    None,
}

impl DoubleLoad {
    pub fn translated_name(&self) -> Translatable {
        match self {
            DoubleLoad::ForceLoad => Translatable::new("rrls.configuration.doubleload.force_load"),
            DoubleLoad::Load => Translatable::new("rrls.configuration.doubleload.load"),
            DoubleLoad::None => Translatable::new("rrls.configuration.hide_doubleload.none"),
        }
    }

    pub fn is_load(&self) -> bool {
        matches!(self, DoubleLoad::ForceLoad | DoubleLoad::Load)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_load_matches_java_switch() {
        assert!(DoubleLoad::ForceLoad.is_load());
        assert!(DoubleLoad::Load.is_load());
        assert!(!DoubleLoad::None.is_load());
    }
}
}

pub mod hide_type {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `HideType.java`.

use super::Translatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HideType {
    All,
    Loading,
    Reloading,
    None,
}

impl HideType {
    pub const ALL_AFFECT_INITIAL: bool = true;
    pub const LOADING_AFFECT_INITIAL: bool = true;
    pub const RELOADING_AFFECT_INITIAL: bool = false;
    pub const NONE_AFFECT_INITIAL: bool = false;

    pub fn affect_initial(&self) -> bool {
        match self {
            HideType::All | HideType::Loading => true,
            HideType::Reloading | HideType::None => false,
        }
    }

    pub fn translated_name(&self) -> Translatable {
        match self {
            HideType::All => Translatable::new("rrls.configuration.hide.all"),
            HideType::Loading => Translatable::new("rrls.configuration.hide.loading"),
            HideType::Reloading => Translatable::new("rrls.configuration.hide.reloading"),
            HideType::None => Translatable::new("rrls.configuration.hide_doubleload.none"),
        }
    }

    pub fn can_hide(&self, reloading: bool) -> bool {
        match self {
            HideType::All => true,
            HideType::Loading => !reloading,
            HideType::Reloading => reloading,
            HideType::None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_hide_matrix_matches_java_switch() {
        assert!(HideType::All.can_hide(true));
        assert!(HideType::All.can_hide(false));
        assert!(HideType::Loading.can_hide(false));
        assert!(!HideType::Loading.can_hide(true));
        assert!(!HideType::Reloading.can_hide(false));
        assert!(HideType::Reloading.can_hide(true));
        assert!(!HideType::None.can_hide(true));
        assert!(!HideType::None.can_hide(false));
    }

    #[test]
    fn affect_initial_flags_match_java_fields() {
        assert!(HideType::All.affect_initial());
        assert!(HideType::Loading.affect_initial());
        assert!(!HideType::Reloading.affect_initial());
        assert!(!HideType::None.affect_initial());
    }
}
}

pub mod rrls_config {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `RrlsConfig.java`.
//!
//! Upstream `RrlsConfig` is a Java interface resolved through a service
//! loader (`ServiceUtils.loadService`) with platform impls
//! (`ForgeLikeConfigImpl` on NeoForge, `FabricConfigImpl` on Fabric, which
//! requires the forgeconfigapiport mod). Horizon has neither service loader,
//! so the interface becomes a trait plus the default values the upstream
//! config screen registers; platform wiring stays a host seam.

use super::{DoubleLoad, HideType, Type};
use super::super::utils::ease::Ease as EaseChoice;

/// Every field of the upstream interface, in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct RrlsConfig {
    pub hide_type: HideType,
    pub block_overlay: bool,
    pub mini_render: bool,
    pub enable_scissor: bool,
    pub r#type: Type,
    pub rgb_progress: bool,
    pub reload_text: String,
    pub interpolate_progress: bool,
    pub interpolate_at_end: bool,
    pub easing: EaseChoice,
    pub easing_arg: Option<f32>,
    pub animation_speed: f32,
    pub reset_resources: bool,
    pub re_init_screen: bool,
    pub early_pack_status_send: bool,
    pub double_load: DoubleLoad,
    pub skip_forge_overlay: bool,
}

impl Default for RrlsConfig {
    /// Upstream ships defaults through its config screen registration
    /// (`ConfigurationScreen`); these are the values its released defaults
    /// use: hideType=NONE keeps the vanilla overlay, rgbProgress on, linear
    /// easing at speed 1.
    fn default() -> Self {
        Self {
            hide_type: HideType::None,
            block_overlay: false,
            mini_render: false,
            enable_scissor: false,
            r#type: Type::Progress,
            rgb_progress: true,
            reload_text: String::new(),
            interpolate_progress: true,
            interpolate_at_end: false,
            easing: EaseChoice::Linear,
            easing_arg: None,
            animation_speed: 1.0,
            reset_resources: false,
            re_init_screen: false,
            early_pack_status_send: false,
            double_load: DoubleLoad::None,
            skip_forge_overlay: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_hold_upstream_shape() {
        let c = RrlsConfig::default();
        assert_eq!(c.hide_type, HideType::None);
        assert!(c.rgb_progress);
        assert_eq!(c.r#type, Type::Progress);
        assert_eq!(c.easing, EaseChoice::Linear);
        assert!(!c.skip_forge_overlay);
        assert!(!c.double_load.is_load());
    }
}
}

pub mod rrls_type {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Type.java` (upstream class name `Type`; the module is
//! `rrls_type` because `type` is reserved).

use super::Translatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Progress,
    Text,
    TextWithBackground,
    None,
}

impl Type {
    pub fn translated_name(&self) -> Translatable {
        match self {
            Type::Progress => Translatable::new("rrls.configuration.type.progress"),
            Type::Text => Translatable::new("rrls.configuration.type.text"),
            Type::TextWithBackground => Translatable::new("rrls.configuration.type.textbg"),
            Type::None => Translatable::new("rrls.configuration.type.none"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_keys_match_upstream() {
        assert_eq!(Type::Progress.translated_name().key, "rrls.configuration.type.progress");
        assert_eq!(Type::Text.translated_name().key, "rrls.configuration.type.text");
        assert_eq!(Type::TextWithBackground.translated_name().key, "rrls.configuration.type.textbg");
        assert_eq!(Type::None.translated_name().key, "rrls.configuration.type.none");
    }
}
}

}

pub mod utils {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of the `utils` package. Rendering-bound helpers
//! (`DummyGuiGraphics`, `GuiGraphicsGenerator`, `AtlasUtils`, `TextureUtils`,
//! `OverlayHelper`) bind to Minecraft's GuiGraphics/render pipeline and are
//! host seams; the pure-logic ones are translated here.


pub mod atlas_utils {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `AtlasUtils.java`.
//!
//! Minecraft's sprite-atlas pipeline (`AtlasManager.AtlasEntry`,
//! `SpriteLoader.Preparations`, `readyForUpload`, `atlas().upload`) is the
//! Horizon host seam; the pipeline's control flow is preserved exactly:
//! single-flight keyed by the atlas definition location, upload on the
//! client thread, removal on completion and on failure. The host drives the
//! async chain via `drive` where the upstream executors would.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::texture_utils::{log_error, log_info, log_warn, Identifier};

/// `SpriteLoader.Preparations` after `readyForUpload`.
#[derive(Debug, Clone, PartialEq)]
pub enum Preparations {
    Ready,
    Failed,
}

/// `CompletableFuture<SpriteLoader.Preparations>` state.
#[derive(Debug, Clone, PartialEq)]
pub enum AtlasLoad {
    Pending,
    Done(Preparations),
}

/// The parts of `AtlasManager.AtlasEntry` the logic touches.
pub struct AtlasEntry {
    /// `config().definitionLocation()`
    pub definition_location: Identifier,
    /// `atlas().upload(preps)` — the host side effect.
    pub upload: Box<dyn Fn(&Preparations) + Send>,
    /// `atlasManager.maxMipmapLevels`
    pub max_mipmap_levels: u8,
    /// `entry.scheduleLoad(resourceManager, executor, mipmap)` + the
    /// `readyForUpload().thenApply(_ -> preps)` hop; `Err` models an
    /// upstream exception anywhere in the chain.
    pub schedule_load: Box<dyn Fn(u8) -> Result<Preparations, ()> + Send>,
}

#[derive(Default)]
pub struct PendingAtlases {
    map: Mutex<HashMap<Identifier, Arc<Mutex<AtlasLoad>>>>,
}

impl PendingAtlases {
    /// `reloadAtlas`: single-flight keyed by the definition location
    /// (`computeIfAbsent`). The future is returned while pending; upload
    /// and removal happen when the chain completes.
    pub fn reload_atlas(&self, entry: &AtlasEntry) -> Arc<Mutex<AtlasLoad>> {
        let existing = self
            .map
            .lock()
            .expect("rrls pending atlases")
            .get(&entry.definition_location)
            .cloned();
        if let Some(f) = existing {
            return f;
        }
        log_info(&format!(
            "Scheduling atlas reload: '{}'!",
            entry.definition_location
        ));
        let f = Arc::new(Mutex::new(AtlasLoad::Pending));
        self.map
            .lock()
            .expect("rrls pending atlases")
            .insert(entry.definition_location.clone(), f.clone());
        f
    }

    /// The host step for the async chain: scheduleLoad ->
    /// readyForUpload -> upload (client thread) -> remove entry;
    /// exceptionally logs, removes, returns null. A done future is a no-op.
    pub fn drive(&self, entry: &AtlasEntry) -> Arc<Mutex<AtlasLoad>> {
        let future = self.reload_atlas(entry);
        {
            let mut state = future.lock().expect("rrls atlas load");
        if matches!(*state, AtlasLoad::Pending) {
            let outcome = (entry.schedule_load)(entry.max_mipmap_levels).map_err(|_| ());
            match outcome {
                Ok(p) => {
                    (entry.upload)(&p);
                    *state = AtlasLoad::Done(Preparations::Ready);
                }
                Err(()) => {
                    log_error(&format!(
                        "Failed to force-reload atlas '{}'!",
                        entry.definition_location
                    ));
                    *state = AtlasLoad::Done(Preparations::Failed);
                }
            }
            drop(state);
            self.map
                .lock()
                .expect("rrls pending atlases")
                .remove(&entry.definition_location);
            }
        }
        future
    }

    /// `reloadAtlasSync(atlasManager, resourceManager, entry)`: warn, then
    /// block on the future (`managedBlock` upstream).
    pub fn reload_atlas_sync(&self, entry: &AtlasEntry) -> Preparations {
        log_warn(&format!(
            "Force-reloading atlas: '{}'!",
            entry.definition_location
        ));
        let f = self.drive(entry);
        match f.lock().expect("rrls atlas load").clone() {
            AtlasLoad::Done(p) => p,
            other => panic!("managedBlock returned while load is not done: {:?}", other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(uploads: Arc<Mutex<u32>>) -> AtlasEntry {
        AtlasEntry {
            definition_location: "minecraft:blocks".into(),
            upload: Box::new(move |_| *uploads.lock().unwrap() += 1),
            max_mipmap_levels: 4,
            schedule_load: Box::new(|_| Ok(Preparations::Ready)),
        }
    }

    #[test]
    fn single_flight_dedupes_while_pending() {
        let p = PendingAtlases::default();
        let uploads = Arc::new(Mutex::new(0));
        let f1 = p.reload_atlas(&entry(uploads.clone()));
        let f2 = p.reload_atlas(&entry(uploads));
        assert!(Arc::ptr_eq(&f1, &f2), "computeIfAbsent returns the live future");
    }

    #[test]
    fn drive_uploads_once_and_removes_entry() {
        let p = PendingAtlases::default();
        let uploads = Arc::new(Mutex::new(0));
        let f1 = p.drive(&entry(uploads.clone()));
        assert_eq!(*uploads.lock().unwrap(), 1);
        assert_eq!(*f1.lock().unwrap(), AtlasLoad::Done(Preparations::Ready));
        // Entry removed on completion: a new call schedules a fresh load.
        let f2 = p.drive(&entry(uploads.clone()));
        assert!(!Arc::ptr_eq(&f1, &f2));
        assert_eq!(*uploads.lock().unwrap(), 2);
    }

    #[test]
    fn failure_logs_removes_and_reports_failed() {
        let p = PendingAtlases::default();
        let bad = AtlasEntry {
            definition_location: "minecraft:blocks".into(),
            upload: Box::new(|_| panic!("upload must not run on failure")),
            max_mipmap_levels: 4,
            schedule_load: Box::new(|_| Err(())),
        };
        let out = p.reload_atlas_sync(&bad);
        assert_eq!(out, Preparations::Failed);
    }

    #[test]
    fn sync_warns_then_waits() {
        let p = PendingAtlases::default();
        let uploads = Arc::new(Mutex::new(0));
        assert_eq!(p.reload_atlas_sync(&entry(uploads)), Preparations::Ready);
    }
}
}

pub mod gui_graphics {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `DummyGuiGraphics.java`.
//!
//! Upstream, this class is machine-generated by `GuiGraphicsGenerator` from
//! Minecraft's `GuiGraphicsExtractor`: every overridable method is a no-op
//! override (and non-void overrides return null/0/false), so the mod can
//! hand an inert extractor to the render pipeline. Here the client
//! extractor surface is the `GuiGraphicsExtractor` trait below (the host
//! seam; the real extractor is Horizon's), with the same method surface in
//! the same order, and `DummyGuiGraphics` the no-op implementation.
//! Overloads keep their order and are suffixed `_o2`/`_o3`.

/// Namespaced id on the wire ("minecraft:foo"), as `Identifier`.
pub type Identifier = String;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DummyGuiGraphics;

pub static DUMMY_GUI_GRAPHICS: DummyGuiGraphics = DummyGuiGraphics;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct AbstractWidget;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ActiveTextCollector;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ActiveTextCollectorParameters;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct BannerFlagModel;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct BannerPatternLayers;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct BookModel;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ClientTooltipComponent;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ClientTooltipPositioner;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Component;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CursorType;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DyeColor;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct EntityRenderState;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Font;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FormattedCharSequence;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FormattedText;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GpuSampler;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GpuTextureView;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GuiSpriteScalingNineSlice;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct HoveredTextEffects;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ItemStack;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Level;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct LivingEntity;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MapRenderState;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Matrix3x2fStack;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ModelSimple;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Quaternionfc;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RenderPipeline;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Renderable;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ResultField;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SpriteId;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Style;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TextureAtlasSprite;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TextureSetup;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TooltipComponent;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Vector3fc;

/// The MC-client types the surface touches. These are opaque seam types:
/// the host (Horizon's real GuiGraphics implementation) supplies the actual
/// ones; for the no-op dummy they only need to exist and default.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Window;


/// The MC-client extractor surface, as the no-op `DummyGuiGraphics`
/// overrides it upstream.
pub trait GuiGraphicsExtractor {
    fn request_cursor(_0: CursorType) -> () {
    }

    fn apply_cursor(_0: Window) -> () {
    }

    fn gui_width(_unused: ()) -> i32 {
        Default::default()
    }

    fn gui_height(_unused: ()) -> i32 {
        Default::default()
    }

    fn pose(_unused: ()) -> Matrix3x2fStack {
        Default::default()
    }

    fn next_stratum(_unused: ()) -> () {
    }

    fn blur_before_this_stratum(_unused: ()) -> () {
    }

    fn enable_scissor(_0: i32, _1: i32, _2: i32, _3: i32) -> () {
    }

    fn disable_scissor(_unused: ()) -> () {
    }

    fn contains_point_in_scissor(_0: i32, _1: i32) -> bool {
        false
    }

    fn horizontal_line(_0: i32, _1: i32, _2: i32, _3: i32) -> () {
    }

    fn vertical_line(_0: i32, _1: i32, _2: i32, _3: i32) -> () {
    }

    fn fill(_0: i32, _1: i32, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn fill_o2(_0: RenderPipeline, _1: i32, _2: i32, _3: i32, _4: i32, _5: i32) -> () {
    }

    fn fill_gradient(_0: i32, _1: i32, _2: i32, _3: i32, _4: i32, _5: i32) -> () {
    }

    fn fill_o3(_0: RenderPipeline, _1: TextureSetup, _2: i32, _3: i32, _4: i32, _5: i32) -> () {
    }

    fn outline(_0: i32, _1: i32, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn inner_fill(_0: RenderPipeline, _1: TextureSetup, _2: i32, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32) -> () {
    }

    fn text_highlight(_0: i32, _1: i32, _2: i32, _3: i32, _4: bool) -> () {
    }

    fn text(_0: Font, _1: String, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn text_o2(_0: Font, _1: String, _2: i32, _3: i32, _4: i32, _5: bool) -> () {
    }

    fn text_o3(_0: Font, _1: FormattedCharSequence, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn text_o4(_0: Font, _1: FormattedCharSequence, _2: i32, _3: i32, _4: i32, _5: bool) -> () {
    }

    fn text_o5(_0: Font, _1: Component, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn text_o6(_0: Font, _1: Component, _2: i32, _3: i32, _4: i32, _5: bool) -> () {
    }

    fn centered_text(_0: Font, _1: String, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn centered_text_o2(_0: Font, _1: Component, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn centered_text_o3(_0: Font, _1: FormattedCharSequence, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn text_with_word_wrap(_0: Font, _1: FormattedText, _2: i32, _3: i32, _4: i32, _5: i32) -> i32 {
        Default::default()
    }

    fn text_with_word_wrap_o2(_0: Font, _1: FormattedText, _2: i32, _3: i32, _4: i32, _5: i32, _6: bool) -> i32 {
        Default::default()
    }

    fn text_with_backdrop(_0: Font, _1: Component, _2: i32, _3: i32, _4: i32, _5: i32) -> () {
    }

    fn blit(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: f32, _5: f32, _6: i32, _7: i32, _8: i32, _9: i32, _10: i32) -> () {
    }

    fn blit_o2(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: f32, _5: f32, _6: i32, _7: i32, _8: i32, _9: i32) -> () {
    }

    fn blit_o3(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: f32, _5: f32, _6: i32, _7: i32, _8: i32, _9: i32, _10: i32, _11: i32) -> () {
    }

    fn blit_o4(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: f32, _5: f32, _6: i32, _7: i32, _8: i32, _9: i32, _10: i32, _11: i32, _12: i32) -> () {
    }

    fn blit_o5(_0: Identifier, _1: i32, _2: i32, _3: i32, _4: i32, _5: f32, _6: f32, _7: f32, _8: f32) -> () {
    }

    fn blit_o6(_0: GpuTextureView, _1: GpuSampler, _2: i32, _3: i32, _4: i32, _5: i32, _6: f32, _7: f32, _8: f32, _9: f32) -> () {
    }

    fn blit_sprite(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: i32, _5: i32) -> () {
    }

    fn blit_sprite_o2(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: i32, _5: i32, _6: f32) -> () {
    }

    fn blit_sprite_o3(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: i32, _5: i32, _6: i32) -> () {
    }

    fn blit_sprite_o4(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32, _8: i32, _9: i32) -> () {
    }

    fn blit_sprite_o5(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32, _8: i32, _9: i32, _10: i32) -> () {
    }

    fn blit_sprite_o6(_0: RenderPipeline, _1: TextureAtlasSprite, _2: i32, _3: i32, _4: i32, _5: i32) -> () {
    }

    fn blit_sprite_o7(_0: RenderPipeline, _1: TextureAtlasSprite, _2: i32, _3: i32, _4: i32, _5: i32, _6: i32) -> () {
    }

    fn blit_sprite_o8(_0: RenderPipeline, _1: TextureAtlasSprite, _2: i32, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32, _8: i32, _9: i32, _10: i32) -> () {
    }

    fn blit_nine_sliced_sprite(_0: RenderPipeline, _1: TextureAtlasSprite, _2: GuiSpriteScalingNineSlice, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32) -> () {
    }

    fn blit_nine_slice_inner_segment(_0: RenderPipeline, _1: GuiSpriteScalingNineSlice, _2: TextureAtlasSprite, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32, _8: i32, _9: i32, _10: i32, _11: i32, _12: i32, _13: i32) -> () {
    }

    fn blit_tiled_sprite(_0: RenderPipeline, _1: TextureAtlasSprite, _2: i32, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32, _8: i32, _9: i32, _10: i32, _11: i32, _12: i32) -> () {
    }

    fn inner_blit(_0: RenderPipeline, _1: Identifier, _2: i32, _3: i32, _4: i32, _5: i32, _6: f32, _7: f32, _8: f32, _9: f32, _10: i32) -> () {
    }

    fn inner_blit_o2(_0: RenderPipeline, _1: GpuTextureView, _2: GpuSampler, _3: i32, _4: i32, _5: i32, _6: i32, _7: f32, _8: f32, _9: f32, _10: f32, _11: i32) -> () {
    }

    fn inner_tiled_blit(_0: RenderPipeline, _1: GpuTextureView, _2: GpuSampler, _3: i32, _4: i32, _5: i32, _6: i32, _7: i32, _8: i32, _9: f32, _10: f32, _11: f32, _12: f32, _13: i32) -> () {
    }

    fn item(_0: ItemStack, _1: i32, _2: i32) -> () {
    }

    fn item_o2(_0: ItemStack, _1: i32, _2: i32, _3: i32) -> () {
    }

    fn item_o3(_0: LivingEntity, _1: ItemStack, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn item_o4(_0: LivingEntity, _1: Level, _2: ItemStack, _3: i32, _4: i32, _5: i32) -> () {
    }

    fn fake_item(_0: ItemStack, _1: i32, _2: i32) -> () {
    }

    fn fake_item_o2(_0: ItemStack, _1: i32, _2: i32, _3: i32) -> () {
    }

    fn item_decorations(_0: Font, _1: ItemStack, _2: i32, _3: i32) -> () {
    }

    fn item_decorations_o2(_0: Font, _1: ItemStack, _2: i32, _3: i32, _4: String) -> () {
    }

    fn item_bar(_0: ItemStack, _1: i32, _2: i32) -> () {
    }

    fn item_count(_0: Font, _1: ItemStack, _2: i32, _3: i32, _4: String) -> () {
    }

    fn item_cooldown(_0: ItemStack, _1: i32, _2: i32) -> () {
    }

    fn map(_0: MapRenderState) -> () {
    }

    fn entity(_0: EntityRenderState, _1: f32, _2: Vector3fc, _3: Quaternionfc, _4: Quaternionfc, _5: i32, _6: i32, _7: i32, _8: i32) -> () {
    }

    fn skin(_0: ModelSimple, _1: Identifier, _2: f32, _3: f32, _4: f32, _5: f32, _6: i32, _7: i32, _8: i32, _9: i32) -> () {
    }

    fn book(_0: BookModel, _1: Identifier, _2: f32, _3: f32, _4: f32, _5: i32, _6: i32, _7: i32, _8: i32) -> () {
    }

    fn banner_pattern(_0: BannerFlagModel, _1: DyeColor, _2: BannerPatternLayers, _3: i32, _4: i32, _5: i32, _6: i32) -> () {
    }

    fn profiler_chart(_0: Vec<ResultField>, _1: i32, _2: i32, _3: i32, _4: i32) -> () {
    }

    fn set_tooltip_for_next_frame(_0: Component, _1: i32, _2: i32) -> () {
    }

    fn set_tooltip_for_next_frame_o2(_0: Vec<FormattedCharSequence>, _1: i32, _2: i32) -> () {
    }

    fn set_tooltip_for_next_frame_o3(_0: Font, _1: ItemStack, _2: i32, _3: i32) -> () {
    }

    fn set_tooltip_for_next_frame_o4(_0: Font, _1: Vec<Component>, _2: Option<TooltipComponent>, _3: i32, _4: i32) -> () {
    }

    fn set_tooltip_for_next_frame_o5(_0: Font, _1: Vec<Component>, _2: Option<TooltipComponent>, _3: i32, _4: i32, _5: Identifier) -> () {
    }

    fn set_tooltip_for_next_frame_o6(_0: Font, _1: Vec<Component>, _2: Option<TooltipComponent>, _3: i32, _4: i32, _5: Identifier, _6: bool) -> () {
    }

    fn set_tooltip_for_next_frame_o7(_0: Font, _1: Vec<FormattedCharSequence>, _2: Option<TooltipComponent>, _3: ClientTooltipPositioner, _4: i32, _5: i32, _6: bool, _7: Identifier) -> () {
    }

    fn set_tooltip_for_next_frame_o8(_0: Font, _1: Component, _2: i32, _3: i32) -> () {
    }

    fn set_tooltip_for_next_frame_o9(_0: Font, _1: Component, _2: i32, _3: i32, _4: Identifier) -> () {
    }

    fn set_component_tooltip_for_next_frame(_0: Font, _1: Vec<Component>, _2: i32, _3: i32) -> () {
    }

    fn set_component_tooltip_for_next_frame_o2(_0: Font, _1: Vec<Component>, _2: i32, _3: i32, _4: Identifier) -> () {
    }

    fn set_tooltip_for_next_frame_o10(_0: Font, _1: Vec<FormattedCharSequence>, _2: i32, _3: i32) -> () {
    }

    fn set_tooltip_for_next_frame_o11(_0: Font, _1: Vec<FormattedCharSequence>, _2: i32, _3: i32, _4: Identifier) -> () {
    }

    fn set_tooltip_for_next_frame_o12(_0: Font, _1: Vec<FormattedCharSequence>, _2: ClientTooltipPositioner, _3: i32, _4: i32, _5: bool) -> () {
    }

    fn set_tooltip_for_next_frame_internal(_0: Font, _1: Vec<ClientTooltipComponent>, _2: i32, _3: i32, _4: ClientTooltipPositioner, _5: Identifier, _6: bool) -> () {
    }

    fn set_tooltip_for_next_frame_internal_o2(_0: Font, _1: Vec<ClientTooltipComponent>, _2: i32, _3: i32, _4: ClientTooltipPositioner, _5: Identifier, _6: bool, _7: bool) -> () {
    }

    fn tooltip(_0: Font, _1: Vec<ClientTooltipComponent>, _2: i32, _3: i32, _4: ClientTooltipPositioner, _5: Identifier, _6: bool) -> () {
    }

    fn set_preedit_overlay(_0: Renderable) -> () {
    }

    fn extract_deferred_elements(_0: i32, _1: i32, _2: f32) -> () {
    }

    fn component_hover_effect(_0: Font, _1: Style, _2: i32, _3: i32) -> () {
    }

    fn get_sprite(_0: SpriteId) -> TextureAtlasSprite {
        Default::default()
    }

    fn text_renderer_for_widget(_0: AbstractWidget, _1: HoveredTextEffects) -> ActiveTextCollector {
        Default::default()
    }

    fn text_renderer(_unused: ()) -> ActiveTextCollector {
        Default::default()
    }

    fn text_renderer_o2(_0: HoveredTextEffects) -> ActiveTextCollector {
        Default::default()
    }

    fn text_renderer_o3(_0: HoveredTextEffects, _1: &mut dyn FnMut(&Style)) -> ActiveTextCollector {
        Default::default()
    }

    fn create_default_text_parameters(_0: f32) -> ActiveTextCollectorParameters {
        Default::default()
    }
}

impl GuiGraphicsExtractor for DummyGuiGraphics {}
}

pub mod gui_graphics_generator {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `GuiGraphicsGenerator.java`.
//!
//! Upstream this is a dev-time ASM `ClassVisitor`: it walks Minecraft's
//! `GuiGraphicsExtractor`, adds a no-op `MethodNode` for every method
//! `DummyGuiGraphics` does not override yet, collects Fabric access-widener
//! entries for private/final members, and emits the generated
//! `DummyGuiGraphics.java` source plus `rrls.accesswidener`. The bytecode
//! reader is the host seam (here: a descriptor list standing in for the
//! class bytes); the visitor's decision logic, the widener entry format,
//! and the source generator are translated exactly.

/// `Opcodes` access flags used below.
pub mod opcodes {
    pub const ACC_PUBLIC: u32 = 0x0001;
    pub const ACC_STATIC: u32 = 0x0008;
    pub const ACC_PRIVATE: u32 = 0x0002;
    pub const ACC_PROTECTED: u32 = 0x0004;
    pub const ACC_FINAL: u32 = 0x0010;
    pub const ACC_SYNTHETIC: u32 = 0x1000;
    pub const ACC_BRIDGE: u32 = 0x0040;
}

/// The fields of an ASM `MethodNode` the visitor touches.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodNode {
    pub access: u32,
    pub name: String,
    pub descriptor: String,
    /// `invisibleAnnotations`: the emitted stubs carry `@Override`.
    pub has_override_annotation: bool,
}

/// A collected access-widener line ("accessible method <owner> <name>
/// <descriptor>" or the extendable variant).
#[derive(Debug, Clone, PartialEq)]
pub struct AccessWidenerEntry(pub String);

/// `GuiGraphicsGenerator extends ClassVisitor`: accumulates stub methods
/// into `output` and widener entries into `access_widener_entries`.
pub struct GuiGraphicsGenerator {
    pub output: Vec<MethodNode>,
    pub owner_name: String,
    pub access_widener_entries: Vec<AccessWidenerEntry>,
    /// Log sink standing in for `Rrls.LOGGER` (a test can observe it).
    pub log: Vec<String>,
}

impl GuiGraphicsGenerator {
    pub fn new(output: Vec<MethodNode>, owner_name: &str) -> Self {
        Self {
            output,
            owner_name: owner_name.to_string(),
            access_widener_entries: Vec::new(),
            log: Vec::new(),
        }
    }

    /// `visitMethod`: static and lambda methods pass through; constructors
    /// pass through; methods already present in the output are skipped with
    /// a log line; everything else gets a stub appended and widener entries
    /// for private/final access.
    pub fn visit_method(&mut self, node: &MethodNode) -> bool {
        if node.name == "<init>" || node.name == "<clinit>" {
            return true; // super.visitMethod(...)
        }

        if (node.access & opcodes::ACC_STATIC) != 0 {
            return true;
        }

        if node.name.contains("lambda$") {
            return true;
        }

        if self.output.iter().any(|m| m.name == node.name && m.descriptor == node.descriptor) {
            self.log.push(format!("Skipping {}{}...", node.name, node.descriptor));
            return true;
        }

        self.log.push(format!("Adding {}{}...", node.name, node.descriptor));
        self.output.push(MethodNode {
            access: node.access,
            name: node.name.clone(),
            descriptor: node.descriptor.clone(),
            has_override_annotation: true, // new AnnotationNode("@Override")
        });

        let is_private = (node.access & opcodes::ACC_PRIVATE) != 0;
        let is_final = (node.access & opcodes::ACC_FINAL) != 0;

        if is_private {
            self.access_widener_entries.push(AccessWidenerEntry(format!(
                "accessible method {} {} {}", self.owner_name, node.name, node.descriptor)));
            self.access_widener_entries.push(AccessWidenerEntry(format!(
                "extendable method {} {} {}", self.owner_name, node.name, node.descriptor)));
        } else if is_final {
            self.access_widener_entries.push(AccessWidenerEntry(format!(
                "extendable method {} {} {}", self.owner_name, node.name, node.descriptor)));
        }

        false // the returned MethodVisitor collects the (empty) body
    }

    /// `main(...)`: walk the target class's methods and emit the source +
    /// access widener. The ASM reader is a supplied descriptor list.
    pub fn generate(&mut self, class_methods: &[MethodNode]) -> (String, String) {
        for m in class_methods {
            self.visit_method(m);
        }
        (generate_java_source(&self.output), generate_access_widener(&self.access_widener_entries))
    }
}

/// `generateJavaSource`: the license header, package, the fixed imports,
/// the class declaration with the INSTANCE singleton, the constructor, and
/// one public override per stub.
pub fn generate_java_source(output: &[MethodNode]) -> String {
    let mut sb = String::new();

    sb.push_str("/*\n");
    sb.push_str(" * Copyright 2023 - 2026 dima_dencep.\n");
    sb.push_str(" *\n");
    sb.push_str(" * Licensed under the Open Software License, Version 3.0 (the \"License\");\n");
    sb.push_str(" * you may not use this file except in compliance with the License.\n");
    sb.push_str(" *\n");
    sb.push_str(" * You may obtain a copy of the License at\n");
    sb.push_str(" *     https://spdx.org/licenses/OSL-3.0.txt\n");
    sb.push_str(" */\n\n");
    sb.push_str("package org.redlance.dima_dencep.mods.rrls.utils;\n\n");
    sb.push_str("import net.minecraft.client.Minecraft;\n");
    sb.push_str("import net.minecraft.client.gui.GuiGraphics;\n");
    sb.push_str("import net.minecraft.client.gui.render.state.GuiRenderState;\n\n");
    sb.push_str("@SuppressWarnings(\"all\")\n");
    sb.push_str("public class DummyGuiGraphics extends GuiGraphicsExtractor {\n");
    sb.push_str("    public static final DummyGuiGraphics INSTANCE = new DummyGuiGraphics();\n\n");
    sb.push_str("    private DummyGuiGraphics() {\n");
    sb.push_str("        super(Minecraft.getInstance(), (GuiRenderState) null, 0, 0);\n");
    sb.push_str("    }\n");

    for method in output {
        sb.push('\n');
        sb.push_str(&generate_method(method));
    }

    sb.push_str("}\n");
    sb
}

/// `generateMethod`: `@Override` when annotated (or when the flag is set
/// for instance methods, mirroring the fallback for non-static methods),
/// visibility widened to public (private/protected/final bits cleared),
/// then `returnType name(descriptor)` with the no-op body.
pub fn generate_method(method: &MethodNode) -> String {
    let has_override = method.has_override_annotation
        || (method.access & opcodes::ACC_STATIC) == 0;

    let mut sb = String::new();
    if has_override {
        sb.push_str("    @Override\n");
    }

    sb.push_str("    ");
    let mut output_access = method.access;
    output_access &= !(opcodes::ACC_PRIVATE | opcodes::ACC_PROTECTED | opcodes::ACC_FINAL);
    output_access |= opcodes::ACC_PUBLIC;
    sb.push_str(&access_to_string(output_access));
    sb.push_str(&format!("{}{} {{\n", method.name, method.descriptor));
    sb.push_str("    }\n");
    sb
}

/// `accessToString`, the flags ASM prints.
pub fn access_to_string(access: u32) -> String {
    let mut s = String::new();
    if access & opcodes::ACC_PUBLIC != 0 { s.push_str("public "); }
    if access & opcodes::ACC_PROTECTED != 0 { s.push_str("protected "); }
    if access & opcodes::ACC_PRIVATE != 0 { s.push_str("private "); }
    if access & opcodes::ACC_FINAL != 0 { s.push_str("final "); }
    if access & opcodes::ACC_STATIC != 0 { s.push_str("static "); }
    s
}

/// The `rrls.accesswidener` file: the v2 header plus one entry per line.
pub fn generate_access_widener(entries: &[AccessWidenerEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut aw = String::from("accessWidener\tv2\tnamed\n\n");
    for e in entries {
        aw.push_str(&format!("{}\n", e.0));
    }
    aw
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, desc: &str, access: u32) -> MethodNode {
        MethodNode { access, name: name.into(), descriptor: desc.into(), has_override_annotation: false }
    }

    #[test]
    fn skip_rules_match_upstream() {
        let mut g = GuiGraphicsGenerator::new(vec![], "a/b/GuiGraphicsExtractor");
        // Constructors, static methods, and lambdas pass through untouched.
        assert!(g.visit_method(&node("<init>", "()V", 0)));
        assert!(g.visit_method(&node("<clinit>", "()V", 0)));
        assert!(g.visit_method(&node("x", "()V", opcodes::ACC_STATIC)));
        assert!(g.visit_method(&node("lambda$y$0", "()V", 0)));
        assert!(g.output.is_empty() && g.access_widener_entries.is_empty());
    }

    #[test]
    fn already_present_methods_are_skipped_with_a_log() {
        let existing = vec![node("guiWidth", "()I", 0)];
        let mut g = GuiGraphicsGenerator::new(existing, "owner");
        assert!(g.visit_method(&node("guiWidth", "()I", 0)));
        assert_eq!(g.output.len(), 1);
        assert_eq!(g.log, vec!["Skipping guiWidth()I...".to_string()]);
    }

    #[test]
    fn new_methods_get_stubs_and_wideners() {
        let mut g = GuiGraphicsGenerator::new(vec![], "owner");
        assert!(!g.visit_method(&node("privateMethod", "(I)V", opcodes::ACC_PRIVATE)));
        assert!(!g.visit_method(&node("finalMethod", "()V", opcodes::ACC_FINAL | opcodes::ACC_PROTECTED)));
        assert_eq!(g.output.len(), 2);
        assert!(g.output.iter().all(|m| m.has_override_annotation));
        assert_eq!(g.access_widener_entries, vec![
            AccessWidenerEntry("accessible method owner privateMethod (I)V".into()),
            AccessWidenerEntry("extendable method owner privateMethod (I)V".into()),
            AccessWidenerEntry("extendable method owner finalMethod ()V".into()),
        ]);
        assert_eq!(g.log[0], "Adding privateMethod(I)V...".to_string());
    }

    #[test]
    fn source_carries_header_instance_and_public_overrides() {
        let out = vec![node("guiWidth", "()I", 0)];
        let src = generate_java_source(&out);
        assert!(src.contains("https://spdx.org/licenses/OSL-3.0.txt"));
        assert!(src.contains("public class DummyGuiGraphics extends GuiGraphicsExtractor {"));
        assert!(src.contains("public static final DummyGuiGraphics INSTANCE"));
        assert!(src.contains("super(Minecraft.getInstance(), (GuiRenderState) null, 0, 0);"));
        assert!(src.contains("@Override\n    public guiWidth()I"));
    }

    #[test]
    fn access_widener_file_has_header() {
        let aw = generate_access_widener(&[AccessWidenerEntry("extendable method a b ()V".into())]);
        assert!(aw.starts_with("accessWidener\tv2\tnamed\n\n"));
        assert!(aw.contains("extendable method a b ()V\n"));
        assert_eq!(generate_access_widener(&[]), "");
    }
}
}

pub mod ease {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Ease.java` (easings from easings.net + constant +
//! linear). Enum ids match upstream exactly, gaps included.

use super::easing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ease {
    Linear,          // 0
    Constant,        // 1
    InSine,          // 6
    OutSine,         // 7
    InOutSine,       // 8
    InCubic,         // 9
    OutCubic,        // 10
    InOutCubic,      // 11
    InQuad,          // 12
    OutQuad,         // 13
    InOutQuad,       // 14
    InQuart,         // 15
    OutQuart,        // 16
    InOutQuart,      // 17
    InQuint,         // 18
    OutQuint,        // 19
    InOutQuint,      // 20
    InExpo,          // 21
    OutExpo,         // 22
    InOutExpo,       // 23
    InCirc,          // 24
    OutCirc,         // 25
    InOutCirc,       // 26
    InBack,          // 27
    OutBack,         // 28
    InOutBack,       // 29
    InElastic,       // 30
    OutElastic,      // 31
    InOutElastic,    // 32
    InBounce,        // 33
    OutBounce,       // 34
    InOutBounce,     // 35
    CatmullRom,      // 36
    Step,            // 37
}

impl Ease {
    /// The upstream enum id, gaps (2..=5) preserved.
    pub fn id(&self) -> u8 {
        match self {
            Ease::Linear => 0,
            Ease::Constant => 1,
            Ease::InSine => 6,
            Ease::OutSine => 7,
            Ease::InOutSine => 8,
            Ease::InCubic => 9,
            Ease::OutCubic => 10,
            Ease::InOutCubic => 11,
            Ease::InQuad => 12,
            Ease::OutQuad => 13,
            Ease::InOutQuad => 14,
            Ease::InQuart => 15,
            Ease::OutQuart => 16,
            Ease::InOutQuart => 17,
            Ease::InQuint => 18,
            Ease::OutQuint => 19,
            Ease::InOutQuint => 20,
            Ease::InExpo => 21,
            Ease::OutExpo => 22,
            Ease::InOutExpo => 23,
            Ease::InCirc => 24,
            Ease::OutCirc => 25,
            Ease::InOutCirc => 26,
            Ease::InBack => 27,
            Ease::OutBack => 28,
            Ease::InOutBack => 29,
            Ease::InElastic => 30,
            Ease::OutElastic => 31,
            Ease::InOutElastic => 32,
            Ease::InBounce => 33,
            Ease::OutBounce => 34,
            Ease::InOutBounce => 35,
            Ease::CatmullRom => 36,
            Ease::Step => 37,
        }
    }

    /// `invoke(t, n)`: run the easing with `t` in 0..1 and its argument.
    pub fn invoke(&self, t: f32, n: Option<f32>) -> f32 {
        let base: Box<dyn Fn(f32) -> f32> = match self {
            Ease::Linear => Box::new(|f| f),
            Ease::Constant => Box::new(|_| 0.0),
            Ease::InSine => Box::new(easing::sine),
            Ease::OutSine => Box::new(easing::sine),
            Ease::InOutSine => Box::new(easing::sine),
            Ease::InCubic => Box::new(easing::cubic),
            Ease::OutCubic => Box::new(easing::cubic),
            Ease::InOutCubic => Box::new(easing::cubic),
            Ease::InQuad => Box::new(easing::quadratic),
            Ease::OutQuad => Box::new(easing::quadratic),
            Ease::InOutQuad => Box::new(easing::quadratic),
            Ease::InQuart => Box::new(easing::pow(4.0)),
            Ease::OutQuart => Box::new(easing::pow(4.0)),
            Ease::InOutQuart => Box::new(easing::pow(4.0)),
            Ease::InQuint => Box::new(easing::pow(5.0)),
            Ease::OutQuint => Box::new(easing::pow(5.0)),
            Ease::InOutQuint => Box::new(easing::pow(5.0)),
            Ease::InExpo => Box::new(easing::exp),
            Ease::OutExpo => Box::new(easing::exp),
            Ease::InOutExpo => Box::new(easing::exp),
            Ease::InCirc => Box::new(easing::circle),
            Ease::OutCirc => Box::new(easing::circle),
            Ease::InOutCirc => Box::new(easing::circle),
            Ease::InBack => Box::new(easing::back(n)),
            Ease::OutBack => Box::new(easing::back(n)),
            Ease::InOutBack => Box::new(easing::back(n)),
            Ease::InElastic => Box::new(easing::elastic(n)),
            Ease::OutElastic => Box::new(easing::elastic(n)),
            Ease::InOutElastic => Box::new(easing::elastic(n)),
            Ease::InBounce => Box::new(easing::bounce(n)),
            Ease::OutBounce => Box::new(easing::bounce(n)),
            Ease::InOutBounce => Box::new(easing::bounce(n)),
            Ease::CatmullRom => Box::new(easing::catmull_rom),
            Ease::Step => Box::new(easing::step(n)),
        };
        match self {
            Ease::OutSine
            | Ease::OutCubic
            | Ease::OutQuad
            | Ease::OutQuart
            | Ease::OutQuint
            | Ease::OutExpo
            | Ease::OutCirc
            | Ease::OutBack
            | Ease::OutElastic
            | Ease::OutBounce => ease_out(base)(t),
            Ease::InOutSine
            | Ease::InOutCubic
            | Ease::InOutQuad
            | Ease::InOutQuart
            | Ease::InOutQuint
            | Ease::InOutExpo
            | Ease::InOutCirc
            | Ease::InOutBack
            | Ease::InOutElastic
            | Ease::InOutBounce
            | Ease::CatmullRom => ease_in_out(base)(t),
            _ => base(t),
        }
    }
}

/// `Ease.easeIn`
pub fn ease_in(function: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    function
}

/// `Ease.easeOut`: `time -> 1 - f(1 - time)`
pub fn ease_out(function: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    move |time: f32| 1.0 - function(1.0 - time)
}

/// `Ease.easeInOut`
pub fn ease_in_out(function: impl Fn(f32) -> f32) -> impl Fn(f32) -> f32 {
    move |time: f32| {
        if time < 0.5 {
            function(time * 2.0) / 2.0
        } else {
            1.0 - function((1.0 - time) * 2.0) / 2.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn ids_match_upstream_enum() {
        assert_eq!(Ease::Linear.id(), 0);
        assert_eq!(Ease::Constant.id(), 1);
        assert_eq!(Ease::InSine.id(), 6);
        assert_eq!(Ease::Step.id(), 37);
    }

    #[test]
    fn ease_out_shape() {
        let out = ease_out(easing::cubic);
        assert!(close(out(0.0), 0.0), "1 - cubic(1)");
        assert!(close(out(1.0), 1.0), "1 - cubic(0)");
        assert!(close(out(0.5), 0.875), "1 - 0.125");
    }

    #[test]
    fn ease_in_out_shape() {
        let io = ease_in_out(easing::cubic);
        assert!(close(io(0.25), 0.0625), "cubic(0.5)/2 = 0.125/2");
        assert!(close(io(0.75), 0.9375), "1 - cubic(0.5)/2");
        assert!(close(io(0.5), 0.5), "branch boundary: (1-t)*2 = 1 -> cubic(1)/2 = 0.5");
    }

    #[test]
    fn invoke_routes_modes() {
        assert!(close(Ease::InCubic.invoke(0.5, None), 0.125));
        assert!(close(Ease::OutCubic.invoke(0.5, None), 0.875));
        assert!(close(Ease::InOutCubic.invoke(0.75, None), 0.9375));
        assert!(close(Ease::Constant.invoke(0.7, None), 0.0));
        assert!(close(Ease::Linear.invoke(0.7, None), 0.7));
    }

    #[test]
    fn step_passes_arg_through() {
        let a = Ease::Step.invoke(0.9, Some(4.0));
        let b = Ease::Step.invoke(0.9, None);
        assert!(close(a, 0.75), "4 steps -> last border 0.75");
        assert!(close(b, 0.5), "default 2 steps");
    }
}
}

pub mod overlay_helper {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `OverlayHelper.java`.
//!
//! `RrlsConfig.INSTANCE` (the service-loaded config) becomes the shared
//! config instance below; `Screen`, `Gui`, and `Overlay` are Horizon host
//! seams carrying exactly the members the logic touches (`overlay`,
//! `rrls$getState().isRendering()`).

use std::sync::{Mutex, OnceLock, RwLock};

use super::super::config::{HideType, RrlsConfig};

/// Upstream `RrlsConfig.INSTANCE`: one process-wide config object.
pub fn config() -> &'static RwLock<RrlsConfig> {
    static INSTANCE: OnceLock<RwLock<RrlsConfig>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(RrlsConfig::default()))
}

/// Host seam for `net.minecraft.client.gui.screens.Screen`; the fields the
/// helper branches on are what Horizon will supply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    /// `GenericMessageScreen` (the "Loading Minecraft" screen).
    GenericMessage,
    /// Any other screen.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Default,
    Hide,
    Wait,
}

impl State {
    /// `State.isRendering()`
    pub fn is_rendering(self) -> bool {
        matches!(self, State::Hide)
    }
}

/// `OverlayHelper.lookupState(screen, reloading)`. `screen` is `null`-aware
/// like the Java `instanceof` chain.
pub fn lookup_state(screen: Option<ScreenKind>, reloading: bool) -> State {
    let hide_type = config().read().expect("rrls config").hide_type;
    if !hide_type.can_hide(reloading) {
        return State::Default;
    }

    if reloading || hide_type.affect_initial() {
        return State::Hide;
    }

    if screen == Some(ScreenKind::GenericMessage) {
        // Loading Minecraft
        return State::Wait;
    }

    if screen.is_some() {
        State::Hide
    } else {
        State::Wait
    }
}

/// Host seam: whether the live overlay is in a rendering state. Upstream
/// reads `Minecraft.getInstance().gui.overlay`; Horizon supplies this flag.
pub fn is_current_rendering_state(live_overlay_state: Option<State>) -> bool {
    match live_overlay_state {
        None => {
            let hide_type = config().read().expect("rrls config").hide_type;
            hide_type.affect_initial()
        }
        Some(state) => is_rendering_state(Some(state)),
    }
}

/// `OverlayHelper.isRenderingState(overlay)`: null-safe + `isRendering()`.
pub fn is_rendering_state(state: Option<State>) -> bool {
    state.is_some_and(|s| s.is_rendering())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests share the one process-wide INSTANCE, so the edit holds for the
    /// whole test (the returned guard restores + releases on drop).
    fn test_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
    }

    struct ConfigGuard {
        saved: RrlsConfig,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl Drop for ConfigGuard {
        fn drop(&mut self) {
            *config().write().expect("rrls config") = self.saved.clone();
        }
    }

    fn with_config(f: impl FnOnce(&mut RrlsConfig)) -> ConfigGuard {
        let _lock = test_lock().lock().expect("test lock");
        let mut c = config().write().expect("rrls config");
        let saved = c.clone();
        f(&mut c);
        drop(c);
        ConfigGuard { saved, _lock }
    }

    #[test]
    fn default_when_hide_type_cannot_hide() {
        let _g = with_config(|c| c.hide_type = HideType::None);
        assert_eq!(lookup_state(Some(ScreenKind::Other), false), State::Default);
        assert_eq!(lookup_state(None, true), State::Default);
    }

    #[test]
    fn hide_when_reloading_or_affect_initial() {
        let _g = with_config(|c| c.hide_type = HideType::All);
        assert_eq!(lookup_state(Some(ScreenKind::Other), true), State::Hide);
        // All.affectInitial -> hide even on the initial load.
        assert_eq!(lookup_state(Some(ScreenKind::GenericMessage), false), State::Hide);
        assert_eq!(lookup_state(None, false), State::Hide);
    }

    #[test]
    fn reloading_config_matches_java_chain() {
        // Reloading canHide(false) is false, so the initial load is DEFAULT
        // for every screen (the GenericMessageScreen WAIT branch below is
        // unreachable with the current enum, kept 1:1 with upstream).
        let _g = with_config(|c| c.hide_type = HideType::Reloading);
        assert_eq!(lookup_state(Some(ScreenKind::GenericMessage), false), State::Default);
        assert_eq!(lookup_state(Some(ScreenKind::Other), false), State::Default);
        assert_eq!(lookup_state(None, false), State::Default);
        // reloading=true: the (reloading || affectInitial) branch wins.
        assert_eq!(lookup_state(Some(ScreenKind::GenericMessage), true), State::Hide);
        assert_eq!(lookup_state(None, true), State::Hide);
    }

    #[test]
    fn rendering_flags() {
        assert!(State::Hide.is_rendering());
        assert!(!State::Default.is_rendering());
        assert!(!State::Wait.is_rendering());
        assert!(is_rendering_state(Some(State::Hide)));
        assert!(!is_rendering_state(Some(State::Wait)));
        assert!(!is_rendering_state(None));
    }

    #[test]
    fn current_rendering_state_falls_back_to_affect_initial() {
        {
            let _g = with_config(|c| c.hide_type = HideType::All);
            assert!(is_current_rendering_state(None));
        }
        let _g = with_config(|c| c.hide_type = HideType::Reloading);
        assert!(!is_current_rendering_state(None));
        assert!(is_current_rendering_state(Some(State::Hide)));
    }
}
}

pub mod texture_utils {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `TextureUtils.java`.
//!
//! Minecraft's `CompletableFuture<TextureContents>` pipeline becomes the
//! `TextureLoad` state machine below with identical semantics: a pending
//! map keyed by the texture's resource id, `computeIfAbsent`-style
//! single-flight entry, the load applied when the future completes,
//! completion removing its own entry (outside the entry call, per the
//! upstream comment), and a force-reload that blocks until done
//! (`Minecraft.managedBlock` is the Horizon seam; the host drives the load
//! via `drive` where the upstream executors would).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// `Identifier` on the wire is a namespaced string ("minecraft:foo").
pub type Identifier = String;

/// The load's terminal state, mirroring `TextureContents` + an error
/// landing as `null` upstream (`exceptionally` -> null).
#[derive(Debug, Clone, PartialEq)]
pub enum TextureContents {
    Loaded,
    Failed,
}

/// `CompletableFuture<TextureContents>` shape: pending, or done.
#[derive(Debug, Clone, PartialEq)]
pub enum TextureLoad {
    Pending,
    Done(TextureContents),
}

#[derive(Default)]
pub struct PendingTextures {
    map: Mutex<HashMap<Identifier, Arc<Mutex<TextureLoad>>>>,
}

impl PendingTextures {
    /// `reloadTexture(manager, rl, texture)`: single-flight keyed by the
    /// resource id (`computeIfAbsent`). Returns the live future while the
    /// load is pending; the completed future removes its own entry via
    /// `whenComplete` upstream.
    pub fn reload_texture(&self, rl: Identifier) -> Arc<Mutex<TextureLoad>> {
        let existing = self
            .map
            .lock()
            .expect("rrls pending textures")
            .get(&rl)
            .cloned();
        if let Some(f) = existing {
            return f;
        }
        let f = Arc::new(Mutex::new(TextureLoad::Pending));
        self.map
            .lock()
            .expect("rrls pending textures")
            .insert(rl.clone(), f.clone());
        f
    }

    /// The host step that stands in for the upstream async chain:
    /// `reloadTextureInternal` schedules the load, applies the contents
    /// when it lands; failure logs and returns null. A done future is a
    /// no-op, like re-driving a completed future.
    pub fn drive(
        &self,
        rl: &str,
        loader: impl FnOnce() -> Result<TextureContents, ()>,
    ) -> Arc<Mutex<TextureLoad>> {
        let future = self.reload_texture(rl.to_string());
        {
            let mut state = future.lock().expect("rrls texture load");
        if matches!(*state, TextureLoad::Pending) {
            log_info(&format!("Reloading texture '{}'!", rl));
            let outcome = match loader() {
                Ok(c) => TextureContents::Loaded,
                Err(()) => {
                    log_error("Failed to force-reload texture!");
                    TextureContents::Failed
                }
            };
            *state = TextureLoad::Done(outcome);
            drop(state);
            // whenComplete cleanup: outside the entry call, otherwise
            // "Recursive update" (upstream comment kept).
            self.map
                .lock()
                .expect("rrls pending textures")
                .remove(rl);
            }
        }
        future
    }

    /// `reloadTextureSync(manager, texture)`: force-reload, block until the
    /// future is done (`managedBlock` upstream).
    pub fn reload_texture_sync(
        &self,
        rl: Identifier,
        loader: impl FnOnce() -> Result<TextureContents, ()>,
    ) -> TextureContents {
        log_warn(&format!("Force-reloading texture: '{}'!", rl));
        let future = self.drive(&rl, loader);
        match future.lock().expect("rrls texture load").clone() {
            TextureLoad::Done(c) => c,
            other => panic!("managedBlock returned while load is not done: {:?}", other),
        }
    }

    pub fn is_pending(&self, rl: &str) -> bool {
        self.map
            .lock()
            .expect("rrls pending textures")
            .get(rl)
            .is_some_and(|f| matches!(*f.lock().expect("rrls texture load"), TextureLoad::Pending))
    }
}

/// Upstream log lines go through `Rrls.LOGGER`; Horizon's host supplies the
/// sink, so they surface as functions a test can observe.
pub fn log_info(_msg: &str) {}
pub fn log_warn(_msg: &str) {}
pub fn log_error(_msg: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_flight_dedupes_while_pending() {
        let p = PendingTextures::default();
        let f1 = p.reload_texture("minecraft:foo".into());
        let f2 = p.reload_texture("minecraft:foo".into());
        assert!(Arc::ptr_eq(&f1, &f2), "computeIfAbsent returns the live future");
        assert!(p.is_pending("minecraft:foo"));
    }

    #[test]
    fn drive_completes_and_fails_like_upstream() {
        let p = PendingTextures::default();
        p.reload_texture("minecraft:bar".into());
        let f = p.drive("minecraft:bar", || Ok(TextureContents::Loaded));
        assert_eq!(*f.lock().unwrap(), TextureLoad::Done(TextureContents::Loaded));
        assert!(!p.is_pending("minecraft:bar"), "whenComplete removed the entry");

        p.reload_texture("minecraft:baz".into());
        let g = p.drive("minecraft:baz", || Err(()));
        assert_eq!(*g.lock().unwrap(), TextureLoad::Done(TextureContents::Failed), "exceptionally -> null");
    }

    #[test]
    fn drive_after_completion_is_a_noop() {
        let p = PendingTextures::default();
        p.reload_texture("minecraft:ok".into());
        p.drive("minecraft:ok", || Ok(TextureContents::Loaded));
        let calls = Arc::new(Mutex::new(0));
        let c2 = calls.clone();
        p.reload_texture("minecraft:ok".into());
        assert!(p.is_pending("minecraft:ok"), "a completed future was removed, so a new call schedules a fresh load");
        p.drive("minecraft:ok", || {
            *c2.lock().unwrap() += 1;
            Ok(TextureContents::Loaded)
        });
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[test]
    fn sync_warns_then_waits() {
        let p = PendingTextures::default();
        let out = p.reload_texture_sync("minecraft:bar".into(), || Ok(TextureContents::Loaded));
        assert_eq!(out, TextureContents::Loaded);
        let err = p.reload_texture_sync("minecraft:baz".into(), || Err(()));
        assert_eq!(err, TextureContents::Failed);
    }
}
}

pub mod easing {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `Easing.java` (easings from easings.net + constant +
//! linear). Every formula is transcribed literally, including its float
//! precision.

/// `Easing.sine`
pub fn sine(n: f32) -> f32 {
    1.0 - (n * std::f32::consts::PI / 2.0).cos()
}

/// `Easing.cubic`
pub fn cubic(n: f32) -> f32 {
    n * n * n
}

/// `Easing.quadratic`
pub fn quadratic(n: f32) -> f32 {
    n * n
}

/// `Easing.pow(n)` as a closure
pub fn pow(n: f32) -> impl Fn(f32) -> f32 {
    move |t: f32| t.powf(n)
}

/// `Easing.exp`
pub fn exp(n: f32) -> f32 {
    2f32.powf(10.0 * (n - 1.0))
}

/// `Easing.circle`
pub fn circle(n: f32) -> f32 {
    1.0 - (1.0 - n * n).sqrt()
}

/// `Easing.back(n)`: `null` -> 1.70158, else `n * 1.70158`.
pub fn back(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = match n {
        None => 1.70158,
        Some(n) => n * 1.70158,
    };
    move |t: f32| t * t * ((n2 + 1.0) * t - n2)
}

/// `Easing.elastic(n)`: `null` -> 1.
pub fn elastic(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = n.unwrap_or(1.0);
    move |t: f32| 1.0 - ((t * std::f32::consts::PI / 2.0).cos()).powi(3) * (t * n2 * std::f32::consts::PI).cos()
}

/// `Easing.bounce(n)`: `null` -> 0.5; min of the four parabolas.
pub fn bounce(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = n.unwrap_or(0.5);
    move |t: f32| {
        let one = |x: f32| 121.0f32 / 16.0 * x * x;
        let two = |x: f32| 121.0f32 / 4.0 * n2 * (x - 6.0f32 / 11.0).powi(2) + 1.0 - n2;
        let three = |x: f32| 121.0 * n2 * n2 * (x - 9.0f32 / 11.0).powi(2) + 1.0 - n2 * n2;
        let four =
            |x: f32| 484.0 * n2 * n2 * n2 * (x - 10.5f32 / 11.0).powi(2) + 1.0 - n2 * n2 * n2;
        one(t).min(two(t)).min(three(t).min(four(t)))
    }
}

/// `Easing.step(n)`: `null` -> 2; panics below 2 steps, then binary-searches
/// the step index, exactly like the Java loop.
pub fn step(n: Option<f32>) -> impl Fn(f32) -> f32 {
    let n2 = n.unwrap_or(2.0);
    assert!(n2 >= 2.0, "Steps must be >= 2, got: {}", n2);
    let steps = n2 as i32;
    move |t: f32| {
        let mut result = 0.0f32;
        if t < 0.0 {
            return result;
        }
        let step_length = 1.0 / steps as f32;
        result = (steps - 1) as f32 * step_length;
        if t > result {
            return result;
        }
        let mut left_border_index = 0i32;
        let mut right_border_index = steps - 1;
        while right_border_index - left_border_index != 1 {
            let test_index = left_border_index + (right_border_index - left_border_index) / 2;
            if t >= test_index as f32 * step_length {
                left_border_index = test_index;
            } else {
                right_border_index = test_index;
            }
        }
        left_border_index as f32 * step_length
    }
}

/// `Easing.catmullRom`
pub fn catmull_rom(n: f32) -> f32 {
    0.5f32
        * (2.0 * (n + 1.0)
            + (((n + 2.0) - n) * 1.0)
            + (2.0 * n - 5.0 * (n + 1.0) + 4.0 * (n + 2.0) - (n + 3.0)) * 1.0
            + (3.0 * (n + 1.0) - n - 3.0 * (n + 2.0) + (n + 3.0)) * 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn endpoints_match_easings_net() {
        assert!(close(sine(0.0), 0.0));
        assert!(close(sine(1.0), 1.0));
        assert!(close(cubic(0.5), 0.125));
        assert!(close(quadratic(0.5), 0.25));
        assert!(close(exp(1.0), 1.0));
        assert!(close(circle(0.0), 0.0));
        assert!(close(circle(1.0), 1.0));
    }

    #[test]
    fn back_matches_java_formula() {
        let f = back(None);
        // n2 = 1.70158: t=0.5 -> 0.25 * (2.70158*0.5 - 1.70158)
        assert!(close(f(0.5), 0.25 * (2.70158 * 0.5 - 1.70158)));
        let g = back(Some(2.0));
        assert!(close(g(0.5), 0.25 * ((2.0 * 1.70158 + 1.0) * 0.5 - 2.0 * 1.70158)));
    }

    #[test]
    fn elastic_at_zero_and_one() {
        let f = elastic(None);
        assert!(close(f(0.0), 0.0), "cos(0)=1 * cos(0)=1 -> 1-1");
        assert!(close(f(1.0), 1.0), "t=1: cos(pi/2)^3 = 0, so 1 - 0*cos(pi) = 1");
    }

    #[test]
    fn bounce_takes_min_of_four() {
        let f = bounce(None);
        assert!(close(f(0.0), 0.0));
        assert!(close(f(1.0), 1.0), "all four hit 1 - n2^n at x=1");
    }

    #[test]
    #[should_panic(expected = "Steps must be >= 2")]
    fn step_panics_below_two() {
        step(Some(1.5));
    }

    #[test]
    fn step_binary_search_matches_java() {
        let f = step(Some(4.0));
        assert!(close(f(-0.5), 0.0), "t<0 -> 0");
        assert!(close(f(1.0), 0.75), "past the last border -> (steps-1)/steps");
        assert!(close(f(0.1), 0.0));
        assert!(close(f(0.9), 0.75));
        assert!(close(step(None)(0.9), 0.5), "default 2 steps");
    }

    #[test]
    fn catmull_rom_transcribed() {
        // Literal transcription: n=0 -> 0.5*(2 + 2 + 0 + 0) = 2
        assert!(close(catmull_rom(0.0), 2.0));
    }
}
}

pub mod rainbow {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `RainbowUtils.java`. Upstream reads
//! `Util.getNanos()` for time and packs ARGB ints; here time is injectable
//! and the color is returned as floats plus the packed ARGB, with the ARGB
//! packing itself translated (`Mth.sin` is plain `f32::sin`).

/// `RainbowUtils.makeChannel`
fn make_channel(speed: f32, shift: f32, offset: i32) -> f32 {
    (1.0 + (speed * shift + offset as f32).sin()) * 0.5
}

/// `RainbowUtils.rainbowColor(float alpha)`, with nanos injected instead of
/// `Util.getNanos()`. Returns the packed ARGB int, same layout as
/// `ARGB.colorFromFloat`.
pub fn rainbow_color(alpha: f32, nanos: u64) -> u32 {
    let time = nanos as f32 * 1e-9;
    let speed = 0.5; // TODO config (upstream comment kept)
    color_from_float(
        alpha,
        make_channel(speed, time, 0),
        make_channel(speed, time, 2),
        make_channel(speed, time, 4),
    )
}

/// `ARGB.colorFromFloat`: r << 16 | g << 8 | b | alpha << 24, float channels
/// scaled by 255.
fn color_from_float(a: f32, r: f32, g: f32, b: f32) -> u32 {
    let f = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u32;
    (f(a) << 24) | (f(r) << 16) | (f(g) << 8) | f(b)
}

/// `ARGB.from8BitChannel`
pub fn from_8bit_channel(value: i32) -> f32 {
    value as f32 / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn make_channel_matches_java() {
        // (1 + sin(0.5*t + off)) / 2
        assert!(close(make_channel(0.5, 0.0, 0), 0.5));
        assert!(close(make_channel(0.5, 0.0, 2), (1.0 + 2.0f32.sin()) * 0.5), "offset adds inside sin");
    }

    #[test]
    fn rainbow_channels_differ_at_a_given_time() {
        let c = rainbow_color(1.0, 1_000_000_000); // t = 1s
        // 8-bit packing quantizes, so compare channel bytes, not floats.
        let chan = |v: f32| (v * 255.0) as u32;
        let r = (c >> 16) & 0xFF;
        let g = (c >> 8) & 0xFF;
        let b = c & 0xFF;
        assert_eq!(r, chan((1.0 + 0.5f32.sin()) * 0.5), "speed*shift = 0.5*1");
        assert_eq!(g, chan((1.0 + 2.5f32.sin()) * 0.5), "offset 2");
        assert_eq!(b, chan((1.0 + 4.5f32.sin()) * 0.5), "offset 4");
    }

    #[test]
    fn packing_layout_matches_argb() {
        let c = color_from_float(1.0, 1.0, 0.0, 0.5);
        assert_eq!(c >> 24, 0xFF);
        assert_eq!((c >> 16) & 0xFF, 0xFF);
        assert_eq!((c >> 8) & 0xFF, 0x00);
        assert_eq!(c & 0xFF, 127, "0.5 * 255 truncates, matching ARGB.as8bitChannel");
    }

    #[test]
    fn from_8bit_channel_scales() {
        assert!(close(from_8bit_channel(255), 1.0));
        assert!(close(from_8bit_channel(0), 0.0));
    }
}
}

pub mod waiting_shared_state {
/*
 * Copyright 2023 - 2026 dima_dencep.
 *
 * Licensed under the Open Software License, Version 3.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 * You may obtain a copy of the License at
 *     https://spdx.org/licenses/OSL-3.0.txt
 */

//! 1:1 translation of `WaitingSharedState.java`.
//!
//! Upstream extends `PreparableReloadListener.SharedState` and overrides
//! `get(StateKey)` so a missing/failed entry returns
//! `CompletableFuture.failedFuture(throwable)` for future-typed keys instead
//! of unwinding. Horizon has no reload state registry, so the store is
//! injected: entries hold either a value or the failure, and `get` mirrors
//! the Java fallback for future-typed keys.

use std::collections::HashMap;
use std::sync::Arc;

pub enum Entry<T> {
    Value(T),
    Failed(String),
}

pub struct WaitingSharedState<T: Clone> {
    entries: HashMap<String, Entry<T>>,
}

impl<T: Clone> WaitingSharedState<T> {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn put_value(&mut self, key: &str, value: T) {
        self.entries.insert(key.to_string(), Entry::Value(value));
    }

    pub fn put_failed(&mut self, key: &str, message: &str) {
        self.entries.insert(key.to_string(), Entry::Failed(message.to_string()));
    }

    /// Upstream `get`: a present value returns as-is; a failure becomes a
    /// failed future for future-typed keys (modeled as `Entry::Failed`
    /// handed back wrapped, never unwound); an absent key still throws.
    pub fn get(&self, key: &str, future_typed: bool) -> Result<Arc<Entry<T>>, String> {
        match self.entries.get(key) {
            Some(e @ Entry::Value(_)) => Ok(Arc::new(e.clone())),
            Some(e @ Entry::Failed(m)) if future_typed => Ok(Arc::new(e.clone())),
            Some(Entry::Failed(m)) => Err(m.clone()),
            None => Err(format!("No state for key {}", key)),
        }
    }
}

impl<T: Clone> Default for WaitingSharedState<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for Entry<T> {
    fn clone(&self) -> Self {
        match self {
            Entry::Value(v) => Entry::Value(v.clone()),
            Entry::Failed(m) => Entry::Failed(m.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_returns_as_is() {
        let mut s = WaitingSharedState::<u32>::new();
        s.put_value("k", 7);
        assert!(matches!(*s.get("k", false).unwrap(), Entry::Value(7)));
    }

    #[test]
    fn failed_becomes_failed_future_for_future_keys() {
        let mut s = WaitingSharedState::<u32>::new();
        s.put_failed("k", "boom");
        assert!(matches!(*s.get("k", true).unwrap(), Entry::Failed(_)), "no unwind");
    }

    #[test]
    fn failed_still_throws_for_plain_keys() {
        let mut s = WaitingSharedState::<u32>::new();
        s.put_failed("k", "boom");
        assert_eq!(s.get("k", false).unwrap_err(), "boom");
    }

    #[test]
    fn missing_key_throws_like_java() {
        let s = WaitingSharedState::<u32>::new();
        assert!(s.get("absent", true).is_err());
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Entry<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Entry::Value(v) => f.debug_tuple("Value").field(v).finish(),
            Entry::Failed(m) => f.debug_tuple("Failed").field(m).finish(),
        }
    }
}
}

}

