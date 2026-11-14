//! 1:1 Rust port of Show Me Your Skin by enjarai (MIT (c) 2022 enjarai,
//! https://codeberg.org/enjarai/show-me-your-skin @ dd4f63e).
//!
//! Swap in progress: the 1:1 source tree replaces the earlier behavioral
//! clean-room port module by module; upstream Java is translated
//! code-for-code with engine surfaces behind `seams`, exactly where the
//! Java mod calls into Minecraft. The crate keeps compiling between stages.

pub mod client;
pub mod compat;
pub mod config;
pub mod gui;
pub mod mixin;
pub mod net;
pub mod pond;
pub mod seams;
pub mod show_me_your_skin;
pub mod show_me_your_skin_client;
pub mod util;

/// nl.enjarai.showmeyourskin.ShowMeYourSkin.MODID
pub const MODID: &str = "showmeyourskin";

pub mod runtime_splice_check;
