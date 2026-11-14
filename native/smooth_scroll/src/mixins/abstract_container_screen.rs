//! Upstream mixin/client/CreativeScreen/AbstractContainerScreenMixin.java.

use crate::duck::CreativeModeInventoryScreenDuck;
use crate::seams::ExtractorSeam;

/// Upstream extractSlotHighlightBackWrap @WrapMethod. `creative` is the
/// screen viewed as CreativeModeInventoryScreenDuck (None when it is not the
/// creative screen).
pub fn extract_slot_highlight_back_wrap(
    graphics: &dyn ExtractorSeam,
    creative: Option<&dyn CreativeModeInventoryScreenDuck>,
    operation: &mut dyn FnMut(&dyn ExtractorSeam),
) {
    if let Some(a) = creative {
        if a.is_mouse_inbounds() && a.is_selected_tab_scrollable() {
            a.en_mask(graphics);
            operation(graphics);
            a.de_mask(graphics);
            return;
        }
    }
    operation(graphics);
}

/// Upstream extractSlotHighlightFrontWrap @WrapMethod.
pub fn extract_slot_highlight_front_wrap(
    graphics: &dyn ExtractorSeam,
    creative: Option<&dyn CreativeModeInventoryScreenDuck>,
    operation: &mut dyn FnMut(&dyn ExtractorSeam),
) {
    if let Some(a) = creative {
        if a.is_mouse_inbounds() && a.is_selected_tab_scrollable() {
            a.en_mask(graphics);
            operation(graphics);
            a.de_mask(graphics);
            return;
        }
    }
    operation(graphics);
}
