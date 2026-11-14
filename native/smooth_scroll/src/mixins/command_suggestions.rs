//! Upstream mixin/client/Chat/CommandSuggestionsMixin.java.

/// Upstream unclampScroll @WrapOperation on the Mth.clamp(DDD)D call in
/// CommandSuggestions.mouseScrolled.
pub fn unclamp_scroll(value: f64, _min: f64, _max: f64) -> f64 {
    // return value; //TODO test on windows for bugs caused by unclamping this
    value
}
