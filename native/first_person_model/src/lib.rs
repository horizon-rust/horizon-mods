//! Exact 1:1 Rust translation of First-person Model
//! (tr7zw/FirstPersonModel, MIT).
//!
//! Upstream package dev.tr7zw.firstperson, @6b5859f (Merge pull request #667,
//! fix/fishing-redirect-mixinextras). Translation by sparticle. Carries the
//! upstream MIT license and credit.
//!
//! Module map (upstream -> here):
//!   versionless/FirstPersonBase.java      -> first_person_base
//!   versionless/Constants.java            -> constants
//!   versionless/config/*                  -> config
//!   versionless/mixinbase/ModelPartBase   -> mixinbase
//!   InventoryUtil.java                    -> inventory_util
//!   KeybindHolder.java                    -> keybind_holder
//!   LogicHandler.java                     -> logic_handler
//!   FirstPersonModelCore.java             -> first_person_model_core
//!   FirstPersonBootstrap.java, FirstPersonModelMod.java -> bootstrap
//!   access/*                              -> access
//!   api/*                                 -> api
//!   modsupport/*                          -> modsupport
//!   mixins/*                              -> mixins (in progress)
//! Host seams for Minecraft / platform types live in seams. Upstream is a
//! multiloader preprocessor source (//? if fabric / forge / >= 1.21.x); this
//! port carries the fabric, >= 1.21.5 variant paths, which is the branch
//! Horizon's 26.3 tree corresponds to.
//!
//! Note: the mod's own README credits sparticle only; upstream's MIT license
//! text ships in LICENSE and the upstream credit is carried here.

pub mod seams;
pub use seams::RUNTIME;
pub mod constants;
pub mod config;
pub mod mixinbase;
pub mod inventory_util;
pub mod keybind_holder;
pub mod access;
pub mod api;
pub mod logic_handler;
pub mod mixins;
pub mod first_person_base;
pub mod first_person_model_core;
pub mod bootstrap;
pub mod modsupport;

#[cfg(test)]
mod tests;

/// Upstream mod id.
pub const MOD_ID: &str = "firstperson";
