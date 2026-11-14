//! Upstream mixin/client/Chat/ChatComponentMixin.java.
//!
//! Priority >1000 upstream: bedrockify needs to move the chatBottom first, so
//! it can be smoothed out here. Unused upstream shadows (getScale as a shadow
//! method, isChatFocused) are not carried; scale arrives as a parameter.

use std::rc::Rc;

use crate::duck::ChatComponentDuck;
use crate::seams::ExtractorSeam;
use crate::sm_sc_cfg::vars;
use crate::smooth_sc::touchpad_scrolled_y;

/// Java Math.round(double): floor(x + 0.5).
fn java_round(v: f64) -> i32 {
    (v + 0.5).floor() as i64 as i32
}

/// Upstream `@Unique private double epsilon = 1d / 10`.
const EPSILON: f64 = 1.0 / 10.0;

/// The chatBottom sentinel: `@Unique private double smoothChatBottom = -69696969`.
const SMOOTH_CHAT_BOTTOM_UNSET: f64 = -69696969.0;

/// A scissor as `Parameters.withScissor(x, width, y, height)` carries it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChatScissor {
    pub x: i32,
    pub width: i32,
    pub y1: i32,
    pub y2: i32,
}

/// Upstream `ActiveTextCollector.Parameters` as the mixin sees it: an object
/// whose scissor can be set (null = no scissor). `Option<ChatParams>` itself
/// models the transformation being null.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ChatParams {
    pub scissor: Option<ChatScissor>,
}

/// Upstream TransformationAccess, which the two inner graphics-access mixins
/// implement for ChatComponent's drawing contexts.
pub trait TransformationAccess {
    fn get_transformation(&self) -> Option<ChatParams>;
    fn set_transformation(&self, transformation: Option<ChatParams>);
    fn get_context(&self) -> Option<&dyn ExtractorSeam>;
}

/// The ChatComponent surface the mixin touches (@Shadow fields and methods).
pub trait ChatComponentHost {
    /// The chatScrollbarPos shadow field.
    fn chat_scrollbar_pos(&self) -> i32;
    fn set_chat_scrollbar_pos(&self, v: i32);
    /// The shadowed getLineHeight() / getWidth() / getLinesPerPage().
    fn line_height(&self) -> i32;
    fn width(&self) -> i32;
    fn lines_per_page(&self) -> i32;
    /// The trimmedMessages shadow list: its size and each line's addedTime.
    fn trimmed_len(&self) -> usize;
    fn trimmed_added_time(&self, index: usize) -> i64;
    /// The shadowed scrollChat(int).
    fn scroll_chat(&self, dir: i32);
}

/// Upstream mixin/client/Chat/ChatComponentMixin.java (priority 1001).
pub struct ChatComponentMixin {
    host: Rc<dyn ChatComponentHost>,
    /// everything in lines for better float precision sync
    pub(crate) smooth_scroll_pos: f64,
    pub(crate) target_scroll_pos: f64,
    pub(crate) smooth_mask_height: f64,
    pub(crate) target_mask_height: f64,
    pub(crate) smooth_chat_bottom: f64,
    pub(crate) refreshing: bool,
}

impl ChatComponentMixin {
    pub fn new(host: Rc<dyn ChatComponentHost>) -> Self {
        Self {
            host,
            smooth_scroll_pos: 0.0,
            target_scroll_pos: 0.0,
            smooth_mask_height: 0.0,
            target_mask_height: 0.0,
            smooth_chat_bottom: SMOOTH_CHAT_BOTTOM_UNSET,
            refreshing: false,
        }
    }

    /// Upstream renderWrap @Inject(method = extractRenderState, at = HEAD).
    /// `foreground` is displayMode.foreground.
    pub fn render_wrap(&mut self, ticks: i64, foreground: bool) {
        let host = self.host.as_ref();
        self.smooth_scroll_pos = (self.smooth_scroll_pos - self.target_scroll_pos)
            * (vars::chat_smoothness() as f64)
                .powf(crate::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + self.target_scroll_pos;

        // ~~snap on less than half a pixel difference~~
        // snap when there is a low enough scroll offset
        if (self.smooth_scroll_pos - self.target_scroll_pos).abs() * (host.line_height() as f64) < EPSILON {
            self.smooth_scroll_pos = self.target_scroll_pos;
        }

        host.set_chat_scrollbar_pos(self.smooth_scroll_pos.floor() as i32);

        // mask height
        let mut shown_line_count = 0usize;
        for r in 0..host.trimmed_len().min(host.lines_per_page() as usize) {
            if ticks - host.trimmed_added_time(r) < 200 || foreground {
                shown_line_count += 1;
            }
        }

        self.target_mask_height = shown_line_count as f64;
        self.smooth_mask_height = (self.smooth_mask_height - self.target_mask_height)
            * (vars::chat_opening_smoothness() as f64)
                .powf(crate::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + self.target_mask_height;

        if (self.smooth_mask_height - self.target_mask_height).abs() * (host.line_height() as f64) < EPSILON {
            self.smooth_mask_height = self.target_mask_height;
        }
    }

    /// Upstream modifyYPos @ModifyVariable(method = extractRenderState, at =
    /// @At(value = "STORE"), name = "chatBottom"). Returns chatBottom unchanged.
    pub fn modify_y_pos(&mut self, graphics: &dyn ExtractorSeam, chat_bottom: i32, scale: f32) -> i32 {
        let host = self.host.as_ref();
        if self.smooth_chat_bottom == SMOOTH_CHAT_BOTTOM_UNSET {
            self.smooth_chat_bottom = chat_bottom as f64 / host.line_height() as f64;
        }
        self.smooth_chat_bottom = (self.smooth_chat_bottom - chat_bottom as f64 / host.line_height() as f64)
            * (vars::chat_opening_smoothness() as f64)
                .powf(crate::smooth_sc::SmoothSc::get_last_frame_duration() as f64)
            + chat_bottom as f64 / host.line_height() as f64;

        // originally i mapped the chat rendering to y = 0, but the chatting mod expects chatbottom to be unchanged
        // this means, we have to compensate for the moved chat with pose translating by -chatbottom
        graphics.pose_translate(
            0.0,
            self.smooth_chat_bottom as f32 * host.line_height() as f32 * scale - chat_bottom as f32 * scale,
        );
        chat_bottom
    }

    /// Upstream forVisibleLineWrap @WrapOperation on the forEachLine call in
    /// extractRenderState. `transformation` is the graphics viewed as
    /// TransformationAccess (None when it is not one).
    pub fn for_visible_line_wrap(
        &mut self,
        graphics: &dyn ExtractorSeam,
        transformation: Option<&dyn TransformationAccess>,
        chat_bottom: i32,
        operation: &mut dyn FnMut() -> i32,
    ) -> i32 {
        if let Some(transformation) = transformation {
            self.en_mask(transformation, chat_bottom);
        }
        graphics.pose_translate(0.0, self.get_draw_offset() as f32);

        let ret = operation();

        graphics.pose_translate(0.0, -(self.get_draw_offset() as f32));
        if let Some(transformation) = transformation {
            self.de_mask(transformation);
        }
        ret
    }

    /// Upstream enMask @Unique.
    fn en_mask(&self, transformation_access: &dyn TransformationAccess, chat_bottom: i32) {
        let host = self.host.as_ref();
        let mut mask_top = java_round((-self.smooth_mask_height) * host.line_height() as f64) + chat_bottom;
        let mut mask_bottom = 0 + chat_bottom; //(int) Math.round(smoothChatBottom * getLineHeight());

        // this lets underlined text, diacritics and stuff overflow two pixels above or under chat
        if self.smooth_scroll_pos == self.target_scroll_pos && self.get_draw_offset() == 0.0 && self.smooth_mask_height != 0.0 {
            if (self.smooth_mask_height - self.target_mask_height).abs() * (host.line_height() as f64) < EPSILON {
                mask_top -= 32;
            }
            mask_bottom += 32;
        }

        // this only affects text and the other only affects everything else... wtf mojank?
        let a = transformation_access.get_transformation();
        if a.is_some() {
            transformation_access.set_transformation(Some(ChatParams {
                scissor: Some(ChatScissor { x: -10, width: host.width() + 1000, y1: mask_top, y2: mask_bottom }),
            }));
        }

        let context = transformation_access.get_context();
        if let Some(context) = context {
            context.enable_scissor(-10, mask_top, host.width() + 1000, mask_bottom);
        }
        if vars::enable_mask_debug() {
            let context = context.expect("upstream dereferences context here unconditionally");
            context.fill(
                -10000,
                -10000,
                context.gui_width() + 999999999,
                context.gui_height() + 999999999,
                crate::seams::argb_color(50, 255, 255, 0),
            );
        }
    }

    /// Upstream deMask @Unique.
    fn de_mask(&self, transformation_access: &dyn TransformationAccess) {
        if transformation_access.get_transformation().is_some() {
            transformation_access.set_transformation(Some(ChatParams { scissor: None })); // a.withScissor(null)
        }

        if let Some(context) = transformation_access.get_context() {
            context.disable_scissor();
        }
    }

    /// Upstream scrollWrap @WrapMethod(method = "scrollChat").
    /// this disables the vanilla scrolling but still clamps the values
    /// the method gets called in addMessageToDisplayQueue, but we replace that behaviour in onNewMessage, so that doesn't matter
    /// mod compatibility hopefully doesn't hinder
    pub fn scroll_wrap(operation: &mut dyn FnMut(i32)) {
        operation(0);
    }

    /// Upstream floatyScroll (ChatComponentDuck), with the host's scrollChat
    /// as seen through scrollWrap.
    pub fn floaty_scroll_impl(&mut self, scroll_y: f64, scroll_chat_wrapped: &mut dyn FnMut(i32)) {
        // target + mousescrollamount * lineamount
        let host = self.host.as_ref();
        let new_target = self.target_scroll_pos
            + (scroll_y / 7.0)
                * (if vars::chat_amount() != 0.0 {
                    vars::chat_amount() / host.line_height() as f32
                } else {
                    7.0
                }) as f64;
        self.target_scroll_pos = self.clamp_scroll(new_target, scroll_chat_wrapped);

        if touchpad_scrolled_y() {
            self.smooth_scroll_pos = self.target_scroll_pos;
        }
    }

    /// Upstream clampScroll @Unique.
    fn clamp_scroll(&mut self, mut clamping: f64, scroll_chat_wrapped: &mut dyn FnMut(i32)) -> f64 {
        let host = self.host.as_ref();
        host.set_chat_scrollbar_pos(clamping.ceil() as i32);

        // we'll only use the clamp from the call
        scroll_chat_wrapped(0);

        if clamping > host.chat_scrollbar_pos() as f64 {
            clamping = host.chat_scrollbar_pos() as f64;
        }
        if clamping < 0.0 {
            clamping = 0.0;
        }

        clamping
    }

    /// Upstream opacity @ModifyVariable(method = "forEachLine", at = @At(value =
    /// "STORE"), name = "alpha").
    pub fn opacity(&self, alpha: f32) -> f32 {
        if vars::chat_opening_smoothness() == 0.0 {
            return alpha;
        }
        1.0
    }

    /// Upstream onNewMessage @ModifyVariable(method = "addMessageToDisplayQueue",
    /// at = @At("STORE"), name = "lines"). `new_lines` is lines.size(); the
    /// list itself passes through unchanged.
    pub fn on_new_message(&mut self, new_lines: usize, scroll_chat_wrapped: &mut dyn FnMut(i32)) {
        if self.refreshing {
            return;
        }
        if self.target_scroll_pos > 0.0 {
            self.target_scroll_pos += new_lines as f64;
            self.target_scroll_pos = self.clamp_scroll(self.target_scroll_pos, scroll_chat_wrapped);
        }
        self.smooth_scroll_pos += new_lines as f64;
        scroll_chat_wrapped(0);
    }

    /// Upstream addLinesAbove @ModifyVariable(method = "forEachLine", at =
    /// @At("STORE"), name = "perPage").
    pub fn add_lines_above(&self, _per_page: i32) -> i32 {
        (self.smooth_mask_height + self.get_draw_offset() / self.host.line_height() as f64).ceil() as i32
    }

    /// Upstream resetScrollWrap @WrapMethod(method = "resetChatScroll").
    pub fn reset_scroll_wrap(&mut self, operation: &mut dyn FnMut()) {
        operation();
        self.target_scroll_pos = self.host.chat_scrollbar_pos() as f64;
    }

    /// Upstream scrollbarVisibleLines @ModifyVariable(method =
    /// extractRenderState, at = @At(value = "STORE"), name = "count").
    pub fn scrollbar_visible_lines(&self, count: i32) -> i32 {
        count - (if self.get_draw_offset() == 0.0 { 0 } else { 1 })
    }

    /// Upstream scrollbarSmooth @ModifyVariable(method = extractRenderState,
    /// at = @At(value = "STORE"), name = "y").
    pub fn scrollbar_smooth(&self, _y: i32, total: i32, chat_bottom: i32, chat_height: i32) -> i32 {
        java_round(self.smooth_scroll_pos * chat_height as f64 / total as f64 - chat_bottom as f64)
    }

    /// Upstream refreshH @Inject(refreshTrimmedMessages, HEAD).
    pub fn refresh_head(&mut self) {
        self.refreshing = true;
    }

    /// Upstream refreshT @Inject(refreshTrimmedMessages, TAIL).
    pub fn refresh_tail(&mut self) {
        self.refreshing = false;
    }

    /// Upstream getDrawOffset @Unique.
    pub fn get_draw_offset(&self) -> f64 {
        -(self.host.chat_scrollbar_pos() as f64 - self.smooth_scroll_pos) * self.host.line_height() as f64
    }
}

impl ChatComponentDuck for ChatComponentMixin {
    fn floaty_scroll(&mut self, scroll_y: f64) {
        let host = self.host.clone();
        self.floaty_scroll_impl(scroll_y, &mut |_dir| {
            // the wrapped scrollChat: the wrap discards the direction and
            // calls the host's scrollChat(0)
            Self::scroll_wrap(&mut |d| host.scroll_chat(d));
        });
    }
}
