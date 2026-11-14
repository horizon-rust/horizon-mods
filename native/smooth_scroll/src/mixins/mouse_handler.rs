//! Upstream mixin/client/Hotbar/MouseHandlerMixin.java.
//!
//! The mouse handler only drives the hotbar rollover counter; the Hud cast to
//! HudDuck rides the seams.

use crate::seams::player_inventory;
use crate::sm_sc_cfg::vars;

/// Upstream onScrollWrap @WrapMethod(method = "onScroll"). `yoffset` is the
/// wrapped call's yoffset parameter; `operation` is the host's onScroll.
pub fn on_scroll_wrap(yoffset: f64, operation: &mut dyn FnMut()) {
    if !player_inventory().player_present() {
        operation();
        return;
    }

    let old_slot = player_inventory().selected_slot();
    let hud_duck = crate::seams::hud_duck();

    operation();

    if !vars::hotbar_rollover() || vars::hotbar_smoothness() == 0.0 || !player_inventory().player_present() {
        return;
    }

    let new_slot = player_inventory().selected_slot();
    if new_slot == old_slot {
        return;
    }

    // vertical > 0 -> Slot Decreases (Left)
    // vertical < 0 -> Slot Increases (Right)
    if yoffset > 0.0 {
        // Scrolling Left
        if new_slot > old_slot {
            // Wrapped (e.g. 0 -> 8)
            hud_duck.borrow_mut().increase_rollover();
        }
    } else if yoffset < 0.0 {
        // Scrolling Right
        if new_slot < old_slot {
            // Wrapped (e.g. 8 -> 0)
            hud_duck.borrow_mut().decrease_rollover();
        }
    }
}
