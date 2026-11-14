//! smooth-gui (Horizon Rust port), slice 1: the animation model.
//!
//! 1:1 port of the upstream logic core (MIT): screens ease in over a
//! configurable duration with configurable slide/fade amounts; the animation
//! state machine drives open, close and cancel (instantly closing a screen
//! cancels without playing the close animation). Screen callbacks land in the
//! next slice.
pub mod animation;
