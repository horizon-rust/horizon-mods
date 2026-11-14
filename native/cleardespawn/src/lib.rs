//! cleardespawn (Horizon Rust port), slice 1: the despawn filter model.
//!
//! 1:1 port of the upstream logic core (MIT): when an item entity despawns,
//! its poof particles are suppressed, except for entity ids on the ignore
//! list. Sound and render seams land in the next slice.
pub mod despawn;
