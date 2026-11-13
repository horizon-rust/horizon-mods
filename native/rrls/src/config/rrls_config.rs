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
use crate::utils::ease::Ease as EaseChoice;

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
