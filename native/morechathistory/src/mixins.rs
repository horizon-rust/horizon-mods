//! 1:1 port of the mod's single mixin class,
//! red.jackf.morechathistory.mixins.ChatComponentMixin.

use crate::seams;

/// java: @Mixin(ChatComponent.class) public class ChatComponentMixin
pub struct ChatComponentMixin;

impl ChatComponentMixin {
    /// java: @ModifyExpressionValue(
    ///           method = {"addMessageToDisplayQueue", "addMessageToQueue",
    ///                     "addRecentChat"},
    ///           at = @At(value = "CONSTANT", args = "intValue=100"))
    ///     public int morechathistory_changeMaxHistory(int original) {
    ///         return original + 16284;
    ///     }
    pub fn morechathistory_change_max_history(original: i32) -> i32 {
        original + 16284
    }
}

/// java: int original = 100 (the CONSTANT intValue=100 the mixin wraps);
/// the wrapped value every call site then trims against.
pub fn max_history() -> i32 {
    ChatComponentMixin::morechathistory_change_max_history(seams::VANILLA_MAX_HISTORY)
}

/// The capacity a chat history deque is created with: upstream's expanded
/// deque sizing reads the same wrapped constant.
pub fn history_capacity() -> i32 {
    max_history()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_max_history_matches_upstream() {
        assert_eq!(ChatComponentMixin::morechathistory_change_max_history(100), 16384);
        assert_eq!(ChatComponentMixin::morechathistory_change_max_history(0), 16284);
        assert_eq!(max_history(), 16384);
        assert_eq!(history_capacity(), 16384);
    }
}
