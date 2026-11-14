//! Upstream mixin/client/Chat/ChatScreenMixin.java.

use crate::duck::ChatComponentDuck;

/// Upstream unclampScroll @WrapOperation on the Mth.clamp(DDD)D call in
/// ChatScreen.mouseScrolled.
pub fn unclamp_scroll(value: f64, _min: f64, _max: f64) -> f64 {
    // return value; //TODO test windows for bugs caused by unclamping this
    value
}

/// Upstream mouseScrolledWrap @WrapOperation on ChatComponent.scrollChat(I)V
/// in ChatScreen.mouseScrolled. The wrap discards the vanilla scroll call and
/// routes the mouse's scrollY into the chat component's floatyScroll.
pub fn mouse_scrolled_wrap(chat_component: &mut dyn ChatComponentDuck, _dir: i32, scroll_y: f64) {
    chat_component.floaty_scroll(scroll_y);
}
