//! crosshair-indicator (Horizon Rust port), slice 1: the progress model.
//!
//! 1:1 port of the upstream logic core (CC0-1.0): the crosshair swaps between
//! variants (vanilla cross, attack-indicator arc, break-progress arc) based on
//! what the player is doing, and colors follow the vanilla attack strength.
//! Render hooks land in the next slice behind the same seam.
pub mod indicator;
