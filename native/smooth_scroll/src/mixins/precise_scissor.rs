//! Upstream mixin/client/PreciseScissor package, 1:1.

pub mod draw_context_mixin {
    //! Upstream DrawContextMixin.java: both injections (the windowScale
    //! redirect on setScissor and the containsPointInScissor wrap) are
    //! commented out upstream, so the class contributes no behavior. Kept as
    //! an empty module for the 1:1 file map.
}

pub mod screen_rect_mixin {
    //! Upstream ScreenRectMixin.java: its only injection (the windowScale
    //! redirect on transform) is commented out upstream, so the class
    //! contributes no behavior. Kept as an empty module for the 1:1 file map.
}
