//! 1:1 Horizon Rust port of Continuity by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity).
//!
//! Swap in progress: the 1:1 source tree replaces the earlier behavioral
//! clean-room port module by module; upstream Java is translated
//! code-for-code with engine surfaces behind `seams`, exactly where the
//! Java mod calls into Minecraft. The crate keeps compiling between stages.

pub mod connecting_properties;
pub mod quad_util;
pub mod simple_sprite_provider;
pub mod processor_compact;
pub mod math_util;
pub mod parsing;
pub mod random_index_provider;
pub mod properties;
pub mod properties_loader;
pub mod resource_redirect;
pub mod processor_base;
pub mod processor_enums;
pub mod seams;
pub mod config;
pub mod render_api;
pub mod processing_context_impl;
pub mod quad_processors;
pub mod overlay_properties;
pub mod overlay_processors;
pub mod model;
pub mod emissive_baked_model;
pub mod config_screen;
pub mod model_wrapping;
pub mod custom_block_layers;
pub mod emissive_suffix_loader;
pub mod continuity_client;
pub mod baked_model_manager_reload;
