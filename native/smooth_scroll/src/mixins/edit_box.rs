//! Upstream mixin/client/Widgets/EditBoxMixin.java.
//!
//! Java String.length() counts UTF-16 units; the port works in chars, which
//! is the same for the ASCII text these paths were written against.

use crate::seams::extractor;
use crate::seams::java_round_f32;
use crate::sm_sc_cfg::vars;
use crate::smooth_sc::touchpad_scrolled_y;

/// The EditBox surface the mixin touches (shadow fields + Font).
pub trait EditBoxHost {
    fn is_hovered(&self) -> bool;
    /// The value shadow field.
    fn value(&self) -> String;
    /// Font.width(s)
    fn font_width(&self, s: &str) -> i32;
    /// Font.plainSubstrByWidth(s, width): s trimmed to fit width.
    fn plain_substr(&self, s: &str, width: i32) -> String;
    /// The displayPos shadow field.
    fn display_pos(&self) -> i32;
    fn set_display_pos(&self, v: i32);
    /// The textX / textY shadow fields and the shadowed getInnerWidth().
    fn text_x(&self) -> i32;
    fn text_y(&self) -> i32;
    fn inner_width(&self) -> i32;
}

/// Upstream mixin/client/Widgets/EditBoxMixin.java.
pub struct EditBoxMixin {
    /// in pixels (left < right)
    pub(crate) smooth_scroll_pos: f32,
    pub(crate) target_scroll_pos: f32,
    pub(crate) prev_cursor_pixel: f32,
}

impl Default for EditBoxMixin {
    fn default() -> Self {
        Self { smooth_scroll_pos: 0.0, target_scroll_pos: 0.0, prev_cursor_pixel: 0.0 }
    }
}

impl EditBoxMixin {
    /// Upstream mouseScrolled override.
    /// safety net - vanilla checks for hovering before calling this function,
    /// mods occasionally do not
    pub fn mouse_scrolled(&mut self, host: &dyn EditBoxHost, h_a: f64, v_a: f64) -> bool {
        if !host.is_hovered() {
            return false;
        }
        self.target_scroll_pos = ((self.target_scroll_pos as f64
            - (v_a - h_a) * vars::text_amount() as f64)
            .clamp(0.0, host.font_width(&host.value()) as f64)) as f32;

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos = self.target_scroll_pos;
        }
        true
    }

    /// Upstream renderWidgetWrap @WrapMethod(method = "extractWidgetRenderState").
    pub fn render_widget_wrap(&mut self, host: &dyn EditBoxHost, operation: &mut dyn FnMut()) {
        self.smooth_scroll_pos = (self.smooth_scroll_pos - self.target_scroll_pos)
            * (vars::text_smoothness() as f64)
                .powf(crate::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
                as f32
            + self.target_scroll_pos;

        let shown = host.plain_substr(&host.value(), java_round_f32(self.smooth_scroll_pos));
        host.set_display_pos(shown.chars().count() as i32);

        let g = extractor();
        g.enable_scissor(host.text_x(), host.text_y() - 10, host.text_x() + host.inner_width(), host.text_y() + 10);
        g.pose_push();
        g.pose_translate(self.get_draw_offset(host), 0.0);
        operation();
        g.pose_pop();
        if vars::enable_mask_debug() {
            g.fill(-100, -100, g.gui_width(), g.gui_height(), crate::seams::argb_color(50, 0, 255, 255));
        }
        g.disable_scissor();
    }

    /// Upstream drawBackgroundWrap @WrapOperation on drawBackground's
    /// blitSprite. Runs after the host popped its pose and disabled the
    /// scissor; the wrap re-arms both around the wrapped call.
    pub fn draw_background_wrap(&mut self, host: &dyn EditBoxHost, operation: &mut dyn FnMut()) {
        let g = extractor();
        g.pose_pop(); // TODO somehow remove this code duplication
        g.disable_scissor();
        operation();
        g.enable_scissor(host.text_x(), host.text_y() - 10, host.text_x() + host.inner_width(), host.text_y() + 10);
        g.pose_push();
        g.pose_translate(self.get_draw_offset(host), 0.0);
    }

    /// Upstream addCharacters @ModifyVariable(method = "extractWidgetRenderState",
    /// at = @At(value = "STORE"), name = "displayed").
    pub fn add_characters(&self, host: &dyn EditBoxHost, displayed: &str) -> String {
        let value = host.value();
        let dp = host.display_pos() as usize;
        let dlen = displayed.chars().count();
        if dp + dlen >= value.chars().count() || dlen == 0 {
            return displayed.to_string();
        }

        let first_char_width = host.font_width(&displayed.chars().next().unwrap().to_string());
        let tail: String = value.chars().skip(dp + dlen).collect();
        let mut additional_chars = host.plain_substr(&tail, first_char_width);
        let total_len = dp + dlen + additional_chars.chars().count();
        if value.chars().count() > total_len {
            additional_chars.push(value.chars().nth(total_len).unwrap());
        }

        format!("{}{}", displayed, additional_chars)
    }

    /// Upstream findClickedPositionInTextWrap @WrapMethod(method =
    /// "findClickedPositionInText"). `button_info` is event.buttonInfo().
    pub fn find_clicked_position_in_text_wrap(
        &self,
        host: &dyn EditBoxHost,
        event_x: f32,
        event_y: f32,
        button_info: &str,
        operation: &mut dyn FnMut(f32, f32, &str) -> i32,
    ) -> i32 {
        operation(event_x - self.get_draw_offset(host), event_y, button_info)
    }

    /// Upstream scrollToWrap @WrapMethod(method = "scrollTo").
    pub fn scroll_to_wrap(&mut self, host: &dyn EditBoxHost, pos: i32, operation: &mut dyn FnMut(i32)) {
        if !vars::text_custom_update() {
            operation(pos);
            let prefix: String = host.value().chars().take(host.display_pos() as usize).collect();
            self.target_scroll_pos = host.font_width(&prefix) as f32;
            return;
        }

        let value = host.value();
        let prefix: String = value.chars().take(pos as usize).collect();
        let cursor_pixel = host.font_width(&prefix) as f32;
        let margin = vars::text_margin() / 100.0 * host.inner_width() as f32;
        let inner = host.inner_width() as f32;

        if vars::text_margin() > 50.0 {
            if cursor_pixel < self.prev_cursor_pixel && cursor_pixel < self.target_scroll_pos + margin {
                self.target_scroll_pos = cursor_pixel - margin;
            } else if cursor_pixel > self.prev_cursor_pixel
                && cursor_pixel > self.target_scroll_pos + inner - margin
            {
                self.target_scroll_pos = cursor_pixel - inner + margin;
            }
            self.target_scroll_pos = self.target_scroll_pos.clamp(cursor_pixel - inner, cursor_pixel);
        } else if cursor_pixel < self.target_scroll_pos + margin {
            self.target_scroll_pos = cursor_pixel - margin;
        } else if cursor_pixel > self.target_scroll_pos + inner - margin {
            self.target_scroll_pos = cursor_pixel - inner + margin;
        }
        self.target_scroll_pos = self.target_scroll_pos.clamp(0.0, host.font_width(&value) as f32);
        host.set_display_pos(0);
        self.prev_cursor_pixel = cursor_pixel;
    }

    /// Upstream getDrawOffset @Unique.
    fn get_draw_offset(&self, host: &dyn EditBoxHost) -> f32 {
        let shown = host.plain_substr(&host.value(), java_round_f32(self.smooth_scroll_pos));
        host.font_width(&shown) as f32 - self.smooth_scroll_pos
    }
}
