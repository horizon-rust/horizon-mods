//! Upstream duck/ interfaces: capabilities mixins attach to host classes.

/// Upstream duck/ChatComponentDuck.java.
pub trait ChatComponentDuck {
    fn floaty_scroll(&mut self, scroll_y: f64);
}

/// Upstream duck/CreativeModeInventoryScreenDuck.java.
pub trait CreativeModeInventoryScreenDuck {
    fn en_mask(&self, graphics: &dyn crate::seams::ExtractorSeam);
    fn de_mask(&self, graphics: &dyn crate::seams::ExtractorSeam);
    fn is_selected_tab_scrollable(&self) -> bool;
    fn is_mouse_inbounds(&self) -> bool;
}

/// Upstream duck/HudDuck.java.
pub trait HudDuck {
    fn increase_rollover(&mut self);
    fn decrease_rollover(&mut self);
}
