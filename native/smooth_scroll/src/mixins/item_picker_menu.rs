//! Upstream mixin/client/CreativeScreen/ItemPickerMenuMixin.java and
//! ItemPickerMenuAccessor.java.

/// The ItemPickerMenu surface: the shadowed abstract calculateRowCount()
/// (the accessor's @Invoker("calculateRowCount") is the same call).
pub trait ItemPickerMenuHost {
    fn calculate_row_count(&self) -> i32;
}

/// Upstream mixin/client/CreativeScreen/ItemPickerMenuMixin.java.
pub struct ItemPickerMenuMixin;

impl ItemPickerMenuMixin {
    /// Upstream scrollToWrap @WrapMethod: based on mouse position, use items
    /// from a row before or not (upstream keeps the vanilla call).
    pub fn scroll_to_wrap(&self, scroll_offs: f32, operation: &mut dyn FnMut(f32)) {
        operation(scroll_offs);
    }

    /// Upstream getRowIndexForScroll @WrapMethod (the vanilla body replaced).
    pub fn get_row_index_for_scroll(&self, host: &dyn ItemPickerMenuHost, scroll_offs: f32) -> i32 {
        ((scroll_offs * host.calculate_row_count() as f32) as f64 as i32).max(0)
    }
}
