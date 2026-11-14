//! Exact 1:1 translation of Auth Me (upstream Java, MIT),
//! main @78db0f5 (2026-09-19, v9.3.0+26.3). The Minecraft/Fabric APIs and
//! Mojang authlib types that do not exist on Horizon are host seams with the
//! same shapes and semantics. The fabric entrypoint `AuthMeFabric` maps to
//! the carrier's client-init hook; the ModMenu entrypoint (resourcefulconfig
//! screen) joins the config-screen batch later.

pub mod auth_me;
pub mod config;
pub mod md5;
pub mod microsoft_utils;
pub mod session_utils;
#[cfg(test)]
pub mod tests;

pub mod gui;
pub mod screens;
pub mod widget;
pub mod mixins;
pub mod fabric_entry;
