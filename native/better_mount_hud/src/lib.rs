//! better-mount-hud (Horizon Rust port), 1:1.
//!
//! Upstream: https://github.com/Lortseam/better-mount-hud
//! Copyright (C) Lortseam and contributors. Licensed under GPL-3.0; this port
//! carries the upstream license and credit per the port policy.
//!
//! Upstream logic: `mixin/HudMixin.java` (62 lines). The mixin's injections
//! into `Hud` (extractVehicleHealth / extractPlayerHealth / getAirBubbleYLine
//! / nextContextualInfoState / extractHotbarAndDecorations) are represented by
//! their handler bodies running against a [`HudMixinEnv`] seam that supplies
//! the shadowed `Hud` state (`minecraft`, `getPlayerVehicleWithHealth`,
//! `getVehicleMaxHearts`) and the player/game-mode surface the handlers read.
pub mod hud_mixin;
