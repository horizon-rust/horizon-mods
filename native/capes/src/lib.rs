//! capes (Horizon Rust port), 1:1.
//!
//! Upstream: https://github.com/CaelTheColher/Capes
//! Copyright (C) CaelTheColher and contributors. Licensed under LGPL-2.1; this
//! port carries the upstream license and credit per the port policy.
//!
//! Upstream logic: 14 Kotlin sources (`Capes.kt`, `CapeConfig.kt`,
//! `CapeType.kt`, `util/Utils.kt`, `handler/PlayerHandler.kt` + data,
//! `menu/*.kt`, `render/PlaceholderEntity*.kt`) and 5 client mixins
//! (`MixinPlayerInfo`, `MixinCapeFeatureRenderer`,
//! `MixinEntityRenderDispatcher`, `AccessorEntityRenderDispatcher`,
//! `MixinSkinCustomizationScreen`). Engine-facing surfaces (screens, rendering,
//! textures, HTTP, config file) run behind seams mirroring the upstream calls.
pub mod capes_mod;
pub mod config;
pub mod handler;
pub mod menus;
pub mod mixins;
pub mod render;
pub mod util;
pub mod tests;
