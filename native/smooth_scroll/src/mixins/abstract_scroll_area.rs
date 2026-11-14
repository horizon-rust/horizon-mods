//! Upstream mixin/client/Widgets/AbstractScrollAreaMixin.java.
//!
//! The host's setScrollAmount dispatches the mixin's `set_scroll_amount_tail`
//! after storing the amount (Java virtual dispatch into the mixin instance),
//! so the mixin state lives in Cells: the dispatch cycle reenters it from the
//! host's shared reference.

use std::cell::Cell;

use crate::sm_sc_cfg::vars;
use crate::smooth_sc::touchpad_scrolled_y;

/// The AbstractScrollArea surface the mixin touches.
pub trait ScrollAreaHost {
    /// The scrollAmount shadow field: pixels scrolled down (up < down).
    fn scroll_amount(&self) -> f64;
    /// The host's setScrollAmount; after storing, it dispatches the mixin's
    /// `set_scroll_amount_tail` (the TAIL inject), which the mixin's own
    /// noSetScrollT guard silences around the updateScroll workaround.
    fn set_scroll_amount(&self, v: f64);
}

/// Upstream mixin/client/Widgets/AbstractScrollAreaMixin.java.
pub struct AbstractScrollAreaMixin {
    pub(crate) smooth_scroll_pos: Cell<f64>,
    pub(crate) target_scroll_pos: Cell<f64>,
    pub(crate) no_set_scroll_t: Cell<bool>,
}

impl Default for AbstractScrollAreaMixin {
    fn default() -> Self {
        Self {
            smooth_scroll_pos: Cell::new(0.0),
            target_scroll_pos: Cell::new(0.0),
            no_set_scroll_t: Cell::new(false),
        }
    }
}

impl AbstractScrollAreaMixin {
    /// Upstream setScrollT @Inject(method = "setScrollAmount", at = @At("TAIL")).
    pub fn set_scroll_amount_tail(&self, host: &dyn ScrollAreaHost) {
        if self.no_set_scroll_t.get() {
            return;
        }
        self.target_scroll_pos.set(host.scroll_amount());
        self.smooth_scroll_pos.set(host.scroll_amount());
    }

    /// Upstream updateScroll @Inject(method = "extractScrollbar", at = @At("HEAD")).
    pub fn update_scroll(&self, host: &dyn ScrollAreaHost) {
        if vars::entry_list_smoothness() == 0.0 {
            return;
        }

        self.smooth_scroll_pos
            .set((self.smooth_scroll_pos.get() - self.target_scroll_pos.get())
                * (vars::entry_list_smoothness() as f64)
                    .powf(crate::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
                + self.target_scroll_pos.get());
        let rounded = self.smooth_scroll_pos.get().round();

        // TODO not so pretty workaround, might fix later
        // basically setscroll also makes the screen redraw
        self.no_set_scroll_t.set(true);
        host.set_scroll_amount(rounded);
        self.no_set_scroll_t.set(false);
    }

    /// Upstream mouseScrolledWrap @WrapMethod(method = "mouseScrolled").
    /// `operation` is the host's mouseScrolled.
    pub fn mouse_scrolled_wrap(
        &self,
        host: &dyn ScrollAreaHost,
        _mouse_x: f64,
        _mouse_y: f64,
        h_a: f64,
        v_a: f64,
        operation: &mut dyn FnMut() -> bool,
    ) -> bool {
        self.no_set_scroll_t.set(true);
        let prev_scroll_pos = host.scroll_amount();
        if vars::entry_list_smoothness() != 0.0 {
            host.set_scroll_amount(self.target_scroll_pos.get());
        }
        let ret = operation();

        if vars::entry_list_amount() != 0.0 && ret {
            host.set_scroll_amount(self.target_scroll_pos.get() - vars::entry_list_amount() * v_a);
        }
        self.target_scroll_pos.set(host.scroll_amount());

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos.set(self.target_scroll_pos.get());
        }

        if vars::entry_list_smoothness() != 0.0 {
            host.set_scroll_amount(prev_scroll_pos);
        }
        self.no_set_scroll_t.set(false);
        ret
    }
}
