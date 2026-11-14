//! world-play-time (Horizon Rust port), slice 1: the playtime model.
//!
//! 1:1 port of the upstream logic core (MIT): playtime accumulates per world
//! (per save and per server) in seconds, is persisted through a JSON store the
//! upstream keeps per world, and is rendered by the same human formatting the
//! mod uses. HUD/overlay seams land in the next slice.
pub mod playtime;
