//! Upstream mixin/client/Chat/SuggestionsListMixin.java.

use std::cell::Cell;

use crate::seams::extractor;
use crate::seams::ExtractorSeam;
use crate::sm_sc_cfg::vars;
use crate::smooth_sc::touchpad_scrolled_y;

/// The SuggestionsList surface the mixin touches (@Shadow fields).
pub trait SuggestionsListHost {
    /// The offset shadow field: scroll position, index of the uppermost line
    /// (up < down).
    fn offset(&self) -> i32;
    fn set_offset(&self, v: i32);
    fn rect_y(&self) -> i32;
    fn rect_height(&self) -> i32;
    fn rect_contains(&self, x: i32, y: i32) -> bool;
}

/// The inject points the host's extractRenderState body drives: the
/// translate inject fires at the first List.get and the unTranslate
/// ModifyVariable fires at each Message STORE.
pub struct TranslateHooks<'a> {
    mixin: &'a SuggestionsListMixin,
    g: &'a dyn ExtractorSeam,
}

impl TranslateHooks<'_> {
    /// Upstream translate @Inject(method = "extractRenderState", at = @At(value
    /// = "INVOKE", target = "Ljava/util/List;get(I)Ljava/lang/Object;", ordinal = 0)).
    pub fn translate(&self, host: &dyn SuggestionsListHost) {
        if self.mixin.translated.get() {
            return;
        }
        self.g.enable_scissor(0, host.rect_y(), self.g.gui_width(), host.rect_y() + host.rect_height());
        self.g.pose_push();
        self.g.pose_translate(0.0, self.mixin.get_draw_offset(host));
        self.mixin.translated.set(true);
    }

    /// Upstream unTranslate @ModifyVariable(method = "extractRenderState",
    /// at = @At(value = "STORE"), ordinal = 0).
    pub fn try_untranslate(&self) {
        self.mixin.try_untranslate(self.g);
    }
}

/// Upstream mixin/client/Chat/SuggestionsListMixin.java.
pub struct SuggestionsListMixin {
    pub(crate) line_height: Cell<i32>,
    pub(crate) smooth_scroll_pos: Cell<f32>,
    pub(crate) target_scroll_pos: Cell<f32>,
    pub(crate) translated: Cell<bool>,
    /// Package-private upstream: `boolean align_pixels = true`.
    pub align_pixels: Cell<bool>,
}

impl Default for SuggestionsListMixin {
    fn default() -> Self {
        Self {
            line_height: Cell::new(12),
            smooth_scroll_pos: Cell::new(0.0),
            target_scroll_pos: Cell::new(0.0),
            translated: Cell::new(false),
            align_pixels: Cell::new(true),
        }
    }
}

impl SuggestionsListMixin {
    /// The wrap head of extractRenderStateWrap: smoothing, half-pixel snap,
    /// offset = floor(smooth).
    pub fn render_head(&self, host: &dyn SuggestionsListHost) {
        self.smooth_scroll_pos.set((self.smooth_scroll_pos.get() - self.target_scroll_pos.get())
            * (vars::suggestion_window_smoothness() as f64)
                .powf(crate::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
                as f32
            + self.target_scroll_pos.get());

        // snap on less than half a pixel difference
        if (self.smooth_scroll_pos.get() - self.target_scroll_pos.get()).abs() < 1.0 / self.line_height.get() as f32 / 2.0 {
            self.smooth_scroll_pos.set(self.target_scroll_pos.get());
        }

        host.set_offset(self.smooth_scroll_pos.get().floor() as i32);
    }

    /// Upstream extractRenderStateWrap @WrapMethod(method =
    /// "extractRenderState"). The body receives the adjusted mouse coords and
    /// the inject-point hooks.
    pub fn extract_render_state_wrap(
        &self,
        g: &dyn ExtractorSeam,
        host: &dyn SuggestionsListHost,
        mouse_x: i32,
        mouse_y: i32,
        body: &mut dyn FnMut(i32, i32, &TranslateHooks),
    ) {
        self.render_head(host);

        let mx = if host.rect_contains(mouse_x, mouse_y) { mouse_x } else { -1 };
        let my = (mouse_y as f32 - self.get_draw_offset(host)).floor() as i32;

        body(mx, my, &TranslateHooks { mixin: self, g });

        self.try_untranslate(g);
    }

    /// Upstream tryUntranslate @Unique.
    pub fn try_untranslate(&self, g: &dyn ExtractorSeam) {
        if self.translated.get() {
            g.pose_pop();
            if vars::enable_mask_debug() {
                g.fill(-100, -100, g.gui_width(), g.gui_height(), crate::seams::argb_color(50, 255, 255, 0));
            }
            g.disable_scissor();
        }
        self.translated.set(false);
    }

    /// Upstream mouseScrolledWrap @WrapMethod(method = "mouseScrolled").
    pub fn mouse_scrolled_wrap(
        &self,
        host: &dyn SuggestionsListHost,
        scroll: f64,
        operation: &mut dyn FnMut(f64) -> bool,
    ) -> bool {
        if vars::suggestion_window_amount() == 0.0 {
            return operation(scroll);
        }

        let index_before = host.offset();

        let new_target = self.target_scroll_pos.get() as f64
            - scroll
                * (if vars::suggestion_window_amount() != 0.0 {
                    vars::suggestion_window_amount() / self.line_height.get() as f32
                } else {
                    1.0
                }) as f64;
        host.set_offset(new_target.ceil() as i32);

        // we'll only use the clamp from the call and also the return value
        let ret = operation(0.0);

        if !ret {
            host.set_offset(index_before);
            return ret;
        }

        let mut new_target = new_target;
        if new_target > host.offset() as f64 {
            new_target = host.offset() as f64;
        }
        if new_target < 0.0 {
            new_target = 0.0;
        }

        self.target_scroll_pos.set(new_target as f32);

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos.set(self.target_scroll_pos.get());
        }

        ret
    }

    /// Upstream cycleWrap @WrapMethod(method = "cycle").
    /// this shouldn't touch the scroll amount, it is called when scrolling by
    /// keyboard-selecting
    pub fn cycle_wrap(&self, host: &dyn SuggestionsListHost, off: i32, operation: &mut dyn FnMut(i32)) {
        let index_before = host.offset();
        host.set_offset(self.target_scroll_pos.get().floor() as i32);

        operation(off);

        self.target_scroll_pos.set(host.offset() as f32);
        host.set_offset(index_before);
    }

    /// Upstream mouseClickedWrap @WrapMethod(method = "mouseClicked"). The
    /// mouseClickedRedirectCondition redirect makes the host's own
    /// Rect2i.contains always true, so only this outer check gates the call.
    pub fn mouse_clicked_wrap(
        &self,
        host: &dyn SuggestionsListHost,
        x: i32,
        y: i32,
        operation: &mut dyn FnMut(i32, i32) -> bool,
    ) -> bool {
        if !host.rect_contains(x, y) {
            return false;
        }
        operation(x, y - crate::seams::java_round_f32(self.get_draw_offset(host)))
    }

    /// Upstream addLineUnder @ModifyVariable(method = "extractRenderState",
    /// at = @At("STORE"), ordinal = 2).
    pub fn add_line_under(&self, host: &dyn SuggestionsListHost, limit: i32) -> i32 {
        if self.get_draw_offset(host) == 0.0 {
            limit
        } else {
            limit + 1
        }
    }

    /// Upstream requestCursorRectCondition @WrapOperation on the
    /// Rect2i.contains call in extractRenderState: the y argument gets the
    /// draw offset folded in.
    pub fn request_cursor_rect_y(&self, host: &dyn SuggestionsListHost, y: i32) -> i32 {
        (y as f32 + self.get_draw_offset(host)).floor() as i32
    }

    /// Upstream getDrawOffset @Unique.
    pub fn get_draw_offset(&self, host: &dyn SuggestionsListHost) -> f32 {
        let ret = (host.offset() as f32 - self.smooth_scroll_pos.get()) * self.line_height.get() as f32;
        if self.align_pixels.get() {
            crate::seams::java_round_f32(ret) as f32
        } else {
            ret
        }
    }
}
