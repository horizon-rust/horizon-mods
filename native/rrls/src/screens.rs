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
            crate::rrls::LOGGER;
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
