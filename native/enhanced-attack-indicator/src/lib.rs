//! enhanced-attack-indicator (Horizon Rust port), slice 1: the indicator model.
//!
//! 1:1 port of the upstream logic core (MIT): the vanilla attack indicator is
//! replaced by selectable variants (the horizontal bar under the crosshair, or
//! the circumferential arc around it), driven by the vanilla attack cooldown
//! with the mod's charge/fill colors. Render hooks land in the next slice.
pub mod indicator;
