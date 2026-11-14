//! Upstream mixin/client/CreativeScreen/CreativeModeInventoryScreenMixin.java.
//!
//! TODO upstream: check compatibility with item borders, flow. The unused
//! upstream constructor, the commented test-item-count block, and the unused
//! ItemPickerMenu scroll wrap are carried as comments only.

use std::rc::Rc;

use crate::duck::CreativeModeInventoryScreenDuck;
use crate::seams::ExtractorSeam;
use crate::seams::java_round_f32;
use crate::sm_sc_cfg::vars;
use crate::smooth_sc::SmoothSc;
use crate::smooth_sc::touchpad_scrolled_y;

/// Upstream `@Unique private int slotSize = 18` (never mutated).
const SLOT_SIZE: i32 = 18;

/// Upstream `@Unique private int scrollItemCount = 45`.
const SCROLL_ITEM_COUNT: i32 = 45;

/// Upstream MouseButtonEvent as the wraps rebuild it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseButtonEvent {
    pub x: f64,
    pub y: f64,
    /// The event's buttonInfo.
    pub button: i32,
}

/// The blit arguments the extractBackground wrap intercepts and replays.
#[derive(Clone, Debug)]
pub struct BlitArgs {
    pub pipeline: String,
    pub texture: String,
    pub x: i32,
    pub y: i32,
    pub u: f32,
    pub v: f32,
    pub width: i32,
    pub height: i32,
    pub texture_width: i32,
    pub texture_height: i32,
}

/// The CreativeModeInventoryScreen surface the mixin touches.
pub trait CreativeScreenHost {
    /// The scrollOffs shadow field (the scroll target).
    fn scroll_offs(&self) -> f32;
    fn set_scroll_offs(&self, v: f32);
    /// The static selectedTab's canScroll().
    fn selected_tab_can_scroll(&self) -> bool;
    /// menu.items: its size and each entry.
    fn menu_items_len(&self) -> usize;
    fn menu_item(&self, index: usize) -> String;
    /// menu.scrollTo(f)
    fn menu_scroll_to(&self, offs: f32);
    /// ((ItemPickerMenuAccessor) menu).getCalculatedRowCount()
    fn calculated_row_count(&self) -> i32;
    /// FabricLoader object share "flow:is_caching_screen" instanceof Boolean b && b.
    fn flow_is_caching_screen(&self) -> bool;
}

/// Upstream mixin/client/CreativeScreen/CreativeModeInventoryScreenMixin.java.
pub struct CreativeModeInventoryScreenMixin {
    host: Rc<dyn CreativeScreenHost>,
    pub(crate) mouse_in_bounds: std::cell::Cell<bool>,
    pub(crate) row_offset: std::cell::Cell<i32>,
    pub(crate) smooth_scroll_offs: std::cell::Cell<f32>,
    item_counter: std::cell::Cell<i32>,
}

impl CreativeModeInventoryScreenMixin {
    pub fn new(host: Rc<dyn CreativeScreenHost>) -> Self {
        Self {
            host,
            mouse_in_bounds: std::cell::Cell::new(true),
            row_offset: std::cell::Cell::new(0),
            smooth_scroll_offs: std::cell::Cell::new(0.0),
            item_counter: std::cell::Cell::new(SCROLL_ITEM_COUNT),
        }
    }

    fn slot_size(&self) -> i32 {
        SLOT_SIZE
    }

    /// Upstream drawBackgroundWrap @WrapOperation on the extractBackground blit.
    /// `operation` re-invokes the blit with the given args on the graphics.
    pub fn draw_background_wrap(
        &self,
        graphics: &dyn ExtractorSeam,
        a: &BlitArgs,
        operation: &mut dyn FnMut(&BlitArgs),
        mouse_x: i32,
        mouse_y: i32,
    ) {
        let host = self.host.as_ref();
        let x = a.x;
        let y = a.y;
        let ix = x + 8;
        let iy = y + 17;
        let iu = a.u + 8.0;
        let iv = a.v + 17.0;
        let iwidth = 162;
        let iheight = 90;

        // transparent background compatibility: up, down, left, right
        graphics.enable_scissor(-10000, -10000, 10000, iy + 1);
        operation(a);
        graphics.disable_scissor();
        graphics.enable_scissor(-10000, iy + iheight - 1, 10000, 10000);
        operation(a);
        graphics.disable_scissor();
        graphics.enable_scissor(-10000, iy + 1, ix, iy + iheight - 1);
        operation(a);
        graphics.disable_scissor();
        graphics.enable_scissor(ix + iwidth, iy + 1, 10000, iy + iheight - 1);
        operation(a);
        graphics.disable_scissor();

        if host.flow_is_caching_screen() {
            return;
        }

        self.smooth_scroll_offs.set(
            (self.smooth_scroll_offs.get() - host.scroll_offs())
                * vars::creative_screen_smoothness().powf(SmoothSc::get_last_frame_duration())
                + host.scroll_offs(),
        );

        let mut row_count = self.calculate_row_count();
        if row_count != 0 {
            if mouse_y - y > 60 && self.smooth_scroll_offs.get() != 0.0 {
                self.row_offset.set(1);
            } else {
                self.row_offset.set(0);
            }
            row_count = self.row_offset.get() / row_count;
        }
        host.menu_scroll_to(self.smooth_scroll_offs.get() + row_count as f32);

        self.mouse_in_bounds.set(
            mouse_x >= ix && mouse_x < ix + iwidth && mouse_y >= iy && mouse_y < iy + iheight,
        );

        let y_offset = -(self.scroll_offs_to_pixels(self.smooth_scroll_offs.get())
            % (self.slot_size() * 5) as f32);

        graphics.enable_scissor(ix, iy + 1, ix + iwidth, iy + iheight - 1);

        graphics.pose_push();
        graphics.pose_translate(0.0, y_offset);

        if vars::creative_use_scissor_texture() {
            graphics.enable_scissor(ix, iy, ix + iwidth, iy + iheight + 1);
            operation(a);
            graphics.disable_scissor();

            graphics.pose_translate(0.0, (self.slot_size() * 5) as f32);

            graphics.enable_scissor(ix, iy + 1, ix + iwidth, iy + iheight);
            operation(a);
            graphics.disable_scissor();
        } else {
            let inner = BlitArgs {
                x: ix,
                y: iy,
                u: iu,
                v: iv,
                width: iwidth,
                height: iheight,
                ..a.clone()
            };
            operation(&inner);

            graphics.pose_translate(0.0, (self.slot_size() * 5) as f32);

            operation(&inner);
        }
        graphics.pose_pop();

        graphics.pose_push();
        graphics.pose_translate(0.0, self.get_draw_offset());
        let curr_row = (self.smooth_scroll_offs.get() * self.calculate_row_count() as f32) as f64 as i32;
        let from_index = (curr_row + 5 - self.row_offset.get() * 5) * 9;
        let mut i = from_index;
        while i >= 0 && (i as usize) < host.menu_items_len() && i < from_index + 9 {
            let item = host.menu_item(i as usize);
            graphics.item(
                &item,
                x + 9 + i % 9 * self.slot_size(),
                y + 6 * self.slot_size() - self.row_offset.get() * 6 * self.slot_size(),
            );
            i += 1;
        }
        graphics.pose_pop();

        graphics.disable_scissor();
    }

    /// Upstream extractSlots override. `run_super` is the host's own
    /// AbstractContainerScreen.extractSlots.
    pub fn extract_slots(&self, graphics: &dyn ExtractorSeam, run_super: &mut dyn FnMut()) {
        if self.host.selected_tab_can_scroll() {
            self.item_counter.set(0);
            self.en_mask(graphics);
        }
        run_super();
    }

    /// Upstream extractSlot override. `run_super` is the host's own
    /// AbstractContainerScreen.extractSlot.
    pub fn extract_slot(&self, graphics: &dyn ExtractorSeam, run_super: &mut dyn FnMut()) {
        run_super();

        self.item_counter.set(self.item_counter.get() + 1);
        if self.item_counter.get() == SCROLL_ITEM_COUNT {
            self.de_mask(graphics);
        }
    }

    /// Upstream mouseScrolledWrap @WrapMethod.
    pub fn mouse_scrolled_wrap(
        &self,
        x: f64,
        y: f64,
        scroll_x: f64,
        scroll_y: f64,
        operation: &mut dyn FnMut(f64, f64, f64, f64) -> bool,
    ) -> bool {
        let host = self.host.as_ref();
        let prev_scroll = host.scroll_offs();
        let ret = operation(x, y, scroll_x, scroll_y);

        // if unscrollable
        if !ret {
            return ret;
        }

        let mut pixelscroll = self.scroll_offs_to_pixels(prev_scroll);
        pixelscroll -= vars::creative_screen_amount() * scroll_y as f32;
        host.set_scroll_offs(self.pixels_to_scroll_offs(pixelscroll).clamp(0.0, 1.0));
        if touchpad_scrolled_y() {
            self.smooth_scroll_offs.set(host.scroll_offs());
        }

        ret
    }

    /// Upstream extractContents override.
    pub fn extract_contents(
        &self,
        graphics: &dyn ExtractorSeam,
        mouse_x: i32,
        mouse_y: i32,
        a: f32,
        run_super: &mut dyn FnMut(i32, i32),
    ) {
        let offset = if self.mouse_in_bounds.get() {
            -java_round_f32(self.get_draw_offset())
        } else {
            0
        };
        let _ = (graphics, a);
        run_super(mouse_x, mouse_y + offset);
    }

    /// Upstream mouseClickedWrap @WrapMethod.
    pub fn mouse_clicked_wrap(
        &self,
        event: &MouseButtonEvent,
        double_click: bool,
        operation: &mut dyn FnMut(&MouseButtonEvent, bool) -> bool,
    ) -> bool {
        let offset = if self.mouse_in_bounds.get() {
            -java_round_f32(self.get_draw_offset())
        } else {
            0
        };
        operation(
            &MouseButtonEvent { x: event.x, y: event.y + offset as f64, button: event.button },
            double_click,
        )
    }

    /// Upstream mouseReleasedWrap @WrapMethod.
    pub fn mouse_released_wrap(
        &self,
        event: &MouseButtonEvent,
        operation: &mut dyn FnMut(&MouseButtonEvent) -> bool,
    ) -> bool {
        let offset = if self.mouse_in_bounds.get() {
            -java_round_f32(self.get_draw_offset())
        } else {
            0
        };
        operation(&MouseButtonEvent { x: event.x, y: event.y + offset as f64, button: event.button })
    }

    /// Upstream selectTabWrap @WrapMethod (the tab argument only passes
    /// through to the operation).
    pub fn select_tab_wrap(&self, operation: &mut dyn FnMut()) {
        self.smooth_scroll_offs.set(0.0);
        operation();
    }

    /// Upstream getDrawOffset @Unique.
    pub fn get_draw_offset(&self) -> f32 {
        if !self.host.selected_tab_can_scroll() {
            return 0.0;
        }
        -self.scroll_offs_to_pixels(self.smooth_scroll_offs.get()) % self.slot_size() as f32
            + self.slot_size() as f32 * self.row_offset.get() as f32
    }

    /// Upstream scrollOffsToPixels @Unique.
    fn scroll_offs_to_pixels(&self, scroll_offs: f32) -> f32 {
        scroll_offs * self.calculate_row_count() as f32 * self.slot_size() as f32
    }

    /// Upstream pixelsToScrollOffs @Unique.
    fn pixels_to_scroll_offs(&self, pixels: f32) -> f32 {
        pixels / self.slot_size() as f32 / self.calculate_row_count() as f32
    }

    /// Upstream calculateRowCount @Unique (through the accessor).
    fn calculate_row_count(&self) -> i32 {
        self.host.calculated_row_count()
    }
}

impl CreativeModeInventoryScreenDuck for CreativeModeInventoryScreenMixin {
    /// Upstream enMask @Override (duck).
    fn en_mask(&self, graphics: &dyn ExtractorSeam) {
        graphics.enable_scissor(8, 18, 170, 106);
        graphics.pose_push();
        graphics.pose_translate(0.0, self.get_draw_offset());
    }

    /// Upstream deMask @Override (duck).
    fn de_mask(&self, graphics: &dyn ExtractorSeam) {
        graphics.pose_pop();

        graphics.disable_scissor();
    }

    fn is_mouse_inbounds(&self) -> bool {
        self.mouse_in_bounds.get()
    }

    fn is_selected_tab_scrollable(&self) -> bool {
        self.host.selected_tab_can_scroll()
    }
}
