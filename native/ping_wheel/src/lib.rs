//! Exact 1:1 Rust translation of Ping Wheel by LukenSkyne (github.com/LukenSkyne/Minecraft-Ping-Wheel), MIT license.
//! Upstream Java package nx.pingwheel maps to the modules below; every Minecraft client/server type
//! sits behind the host seams in `seams`. Log lines, field names, branch order and quirks are kept as-is.
//!
//! - `global`      <- common/Global.java
//! - `compat`      <- common/compat/
//! - `config`      <- common/config/
//! - `network`     <- common/network/
//! - `math`        <- common/math/
//! - `util`        <- common/util/ (RateLimiter; InputUtils / DirectionalSoundInstance follow in a later phase)
//! - `core`        <- common/core/ (GameContext, PingData, PingView, PingController, PingManager, ServerCore)
//! - `render`      <- common/render/ (DrawContext, OverlayRenderer, DirectionIndicatorRenderer, PingLocationRenderer)
//! - `common_client` <- common/CommonClient.java (screen/ integration/ command/ mixins: later phases).

pub mod global;
pub mod common_client;
pub mod integration;
pub mod screen;
pub mod render;
pub mod compat;
pub mod config;
pub mod network;
pub mod math;
pub mod util;
pub mod resource;
pub mod core;
pub mod seams;
pub mod tests;
