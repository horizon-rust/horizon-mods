//! 3dskinlayers runtime (behavioral port of tr7zw's 3D Skin Layers, 1.11.4).
//!
//! Upstream renders the skin's second layer as an extruded 3D shell with
//! animation and per-part toggles. This module is the client-side wiring for
//! Horizon's native-mod runtime: it owns the per-player layer state, rebuilds
//! the shell from the skin overlay, and applies the render policy.
//!
//! Ported clean-room under CC0-1.0: no upstream code or assets are included.
//! The behavior described here is the public, observable behavior of the mod.

/// Per-player layer state as the renderer consumes it.
#[derive(Clone, Debug, Default)]
pub struct SkinLayerState {
    /// Rebuilt cuboid sets per part, from the skin overlay.
    pub parts: Vec<(u8, Vec<crate::layers::Cuboid>)>,
    pub anim_ticks: u32,
}

impl SkinLayerState {
    /// Rebuild after a skin change: parse the overlay, gate by config, merge
    /// opaque runs into quads per face.
    pub fn rebuild(&mut self, cfg: &crate::Config, skin: &crate::layers::Skin) {
        let out = crate::PlayerLayers::rebuild(cfg, skin);
        self.parts = out.parts;
        self.anim_ticks = out.anim_ticks;
    }

    /// Advance the shell animation when enabled.
    pub fn tick(&mut self, cfg: &crate::Config, ticks: u32) {
        if cfg.animations {
            self.anim_ticks = self.anim_ticks.wrapping_add(ticks);
        }
    }

    /// Render policy: the shell on the player's own head is hidden in first
    /// person unless explicitly enabled; third-person renders all built parts.
    pub fn should_render_first_person(&self, cfg: &crate::Config) -> bool {
        cfg.first_person_visible
    }
}
