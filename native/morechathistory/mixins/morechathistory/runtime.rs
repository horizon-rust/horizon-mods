//! morechathistory launcher runtime splice (1:1 port of the CC0-1.0
//! MoreChatHistory mod by JackFred).
//!
//! This file is the Horizon native-mod wiring the launcher compiles into its
//! client runtime: it drives the 1:1 mixin module
//! (ChatComponentMixin.morechathistory_changeMaxHistory, original + 16284)
//! from the launcher's own chat surfaces. Every call below forwards to the
//! exact constant the Java mixin would have wrapped, in upstream order.

use crate::mixins::ChatComponentMixin;
use crate::seams;

/// Launcher-side wiring for the 1:1 port: the wrapped history cap applied at
/// the three upstream call sites. Upstream has no initializer class (the mod
/// is one mixin); this splice is only the host binding and adds no policy of
/// its own.
#[derive(Clone, Debug, Default)]
pub struct MoreChatHistoryRuntime;

impl MoreChatHistoryRuntime {
    /// java: @ModifyExpressionValue on ChatComponent.addMessageToDisplayQueue,
    /// addMessageToQueue and addRecentChat (CONSTANT intValue=100): the
    /// wrapped cap every history trim uses.
    pub fn max_history(&self) -> i32 {
        ChatComponentMixin::morechathistory_change_max_history(seams::VANILLA_MAX_HISTORY)
    }

    /// The capacity a chat history deque is created with: upstream's expanded
    /// deque sizing reads the same wrapped constant.
    pub fn history_capacity(&self) -> i32 {
        self.max_history()
    }

    /// java: ChatComponent.addMessageToDisplayQueue - trim a history to the
    /// wrapped cap, dropping from the oldest side the way ArrayListDeque does.
    pub fn add_message_to_display_queue(&self, history: &mut Vec<String>, message: String) {
        seams::add_message_to_display_queue(history, message)
    }
}
