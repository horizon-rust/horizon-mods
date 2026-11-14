//! Exact 1:1 Rust translation of Smooth Scrolling
//! (SmajloSlovakian/Minecraft-Smooth-Scrolling, GPL-3.0-only).
//!
//! Upstream package io.github.smajloslovakian.smoothscroll, @639d45b.
//! Translation by sparticle. Carries the upstream GPL-3.0 license and credit.
//!
//! Module map (upstream -> here):
//!   SmoothScrolling.java, SmoothSc.java, Globals.java -> smooth_sc
//!   cfg/CfgValue.java          -> cfg_value
//!   cfg/CfgValueBuilder.java   -> cfg_value_builder
//!   cfg/NewConfig.java         -> new_config
//!   cfg/SmScCfg.java           -> sm_sc_cfg
//!   menu/*                     -> menu
//!   mixin/client/*             -> mixins
//!   duck/*                     -> duck
//!   TransformationAccess.java  -> transformation_access
//! Host seams for Minecraft / platform types live in seams.

pub mod seams;
pub mod smooth_sc;
pub mod cfg_value;
pub mod cfg_value_builder;
pub mod new_config;
pub mod sm_sc_cfg;
pub mod menu;
pub mod duck;
pub mod transformation_access;
pub mod mixins;
#[cfg(test)]
mod tests;

/// Upstream SmoothScrolling.MOD_ID.
pub const MOD_ID: &str = "smoothscroll";
