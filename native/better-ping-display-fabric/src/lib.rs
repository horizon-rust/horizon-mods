//! better-ping-display (Horizon Rust port), slice 1: the ping model.
//!
//! 1:1 port of the upstream logic core (MIT): a smoothed ping value per player,
//! colored by thresholds, rendered as "+<ms> ms" text next to the vanilla bar
//! in the player list. List-attach seams land in the next slice.
pub mod ping;
