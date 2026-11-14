//! Upstream mixin/client/Hotbar/HudMixin.java.
//!
//! Mixin priority 999 (upstream comment: <1000 so bedrockify applying first
//! doesn't crash modifyarg; >= -999999999 so the wrapoperation applies after
//! the raised mod fixing raised hotbar selectors).

use crate::duck::HudDuck;
use crate::sm_sc_cfg::vars;
use crate::seams::{argb_color, extractor};

/// Upstream mixin/client/Hotbar/HudMixin.java.
pub struct HudMixin {
    pub(crate) slot_width: i32,
    pub(crate) slot_count: i32,
    pub(crate) rollover_space: i32,
    pub(crate) masked: bool,
    /// pixels to the right of hotbar start (upstream comment)
    pub(crate) smooth_selector_pos: f32,
    pub(crate) rollover: i32,
}

impl Default for HudMixin {
    fn default() -> Self {
        Self {
            slot_width: 20,
            slot_count: 9,
            rollover_space: 4,
            masked: false,
            smooth_selector_pos: 0.0,
            rollover: 0,
        }
    }
}

impl HudMixin {
    /// Upstream extractHotbarWrap @WrapMethod(method = "extractItemHotbar").
    /// `operation` is the host's extractItemHotbar; the graphics extractor and
    /// delta tracker ride the seams.
    pub fn extract_hotbar_wrap(&mut self, operation: &mut dyn FnMut()) {
        let selected = crate::seams::player_inventory().selected_slot();

        self.rollover_space = if vars::static_selector() { 1 } else { 4 };
        let target = (selected - self.rollover * self.slot_count) * self.slot_width
            - self.rollover * self.rollover_space;
        self.smooth_selector_pos = (((self.smooth_selector_pos - target as f32) as f64
            * (vars::hotbar_smoothness() as f64).powf(crate::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + target as f64)) as f32;

        if self.smooth_selector_pos < 0.0 {
            self.smooth_selector_pos += (self.slot_count * self.slot_width + self.rollover_space) as f32;
            self.rollover -= 1;
        } else if self.smooth_selector_pos
            > (self.rollover_space * 2 + self.slot_width * self.slot_count) as f32
        {
            self.smooth_selector_pos -= (self.slot_count * self.slot_width + self.rollover_space) as f32;
            self.rollover += 1;
        }

        operation();
    }

    /// Upstream moveSelector @WrapOperation on the second blitSprite call in
    /// extractItemHotbar. `x` is the call's x argument; `operation` is the
    /// wrapped blitSprite with those arguments.
    pub fn move_selector(&mut self, x: i32, operation: &mut dyn FnMut()) {
        let selected = crate::seams::player_inventory().selected_slot();
        let g = extractor();
        let hotbar_start = x - selected * self.slot_width;

        if vars::static_selector() {
            g.pose_push();
            g.pose_translate(-((x - hotbar_start) as f32) + (self.slot_count / 2 * self.slot_width) as f32, 0.0);

            operation();

            g.pose_pop();
            return;
        }

        if self.smooth_selector_pos.round() > (self.slot_width * 8) as f32 {
            self.enable_mask();
            g.pose_push();
            g.pose_translate(
                self.smooth_selector_pos - (x - hotbar_start) as f32
                    - (self.slot_count * self.slot_width + self.rollover_space) as f32,
                0.0,
            );

            operation();

            g.pose_pop();
        }

        g.pose_push();
        g.pose_translate(self.smooth_selector_pos - (x - hotbar_start) as f32, 0.0);
        operation();
        g.pose_pop();

        if self.masked {
            if vars::enable_mask_debug() {
                g.fill(-100, -100, g.gui_width(), g.gui_height(), argb_color(50, 0, 255, 255));
            }
            g.disable_scissor();
            self.masked = false;
        }
    }

    /// Upstream enableMask @Unique.
    fn enable_mask(&mut self) {
        let g = extractor();
        let x2 = g.gui_width() / 2 - 91;
        let y2 = g.gui_height() - 22;
        if !vars::static_selector() {
            g.enable_scissor(x2 - 1, y2 - 1, x2 + 182 + 1, y2 + 22 + 1);
        } else {
            g.enable_scissor(x2, y2 - 1, x2 + 182, y2 + 22 + 1);
        }
        self.masked = true;
    }

    /// Upstream moveHotbar @WrapOperation on the first blitSprite call in
    /// extractItemHotbar.
    pub fn move_hotbar(&mut self, operation: &mut dyn FnMut()) {
        self.move_whole_hotbar(operation);
    }

    /// Upstream moveItems @WrapOperation on the first extractSlot call in
    /// extractItemHotbar. `x`/`y`/`seed` are the call's arguments.
    pub fn move_items(&mut self, x: i32, y: i32, seed: i32, operation: &mut dyn FnMut()) {
        self.move_whole_hotbar(&mut || {
            extractor().extract_slot(x, y, seed);
            operation();
        });
    }

    /// Upstream moveWholeHotbar @Unique.
    fn move_whole_hotbar(&mut self, draw: &mut dyn FnMut()) {
        let g = extractor();
        if !vars::static_selector() {
            draw();
            return;
        }
        self.enable_mask();
        g.pose_push();
        g.pose_translate(
            -self.smooth_selector_pos + (self.slot_count / 2 * self.slot_width) as f32,
            0.0,
        );

        draw();

        if self.smooth_selector_pos > (self.slot_count / 2 * self.slot_width) as f32 {
            g.pose_translate(((self.slot_count * self.slot_width + self.rollover_space)) as f32, 0.0);
        } else {
            g.pose_translate(-((self.slot_count * self.slot_width + self.rollover_space)) as f32, 0.0);
        }

        draw();

        g.pose_pop();

        if self.masked {
            if vars::enable_mask_debug() {
                g.fill(-100, -100, g.gui_width(), g.gui_height(), argb_color(50, 0, 255, 255));
            }
            g.disable_scissor();
            self.masked = false;
        }
    }

    /// Test visibility for the wrap checks.
    #[cfg(test)]
    pub(crate) fn state(&self) -> (f32, i32, bool) {
        (self.smooth_selector_pos, self.rollover, self.masked)
    }
}

impl HudDuck for HudMixin {
    fn increase_rollover(&mut self) {
        self.rollover += 1;
    }

    fn decrease_rollover(&mut self) {
        self.rollover -= 1;
    }
}
