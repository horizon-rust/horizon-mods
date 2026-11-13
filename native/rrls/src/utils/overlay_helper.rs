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

use crate::config::{HideType, RrlsConfig};

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
