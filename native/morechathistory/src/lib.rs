//! 1:1 Horizon Rust port of MoreChatHistory by JackFred (CC0-1.0,
//! https://github.com/JackFred2/MoreChatHistory).
//!
//! Upstream is one mixin: `ChatComponentMixin.morechathistory_changeMaxHistory`
//! adds 16284 to the vanilla 100-message cap wherever the chat component
//! enforces it (addMessageToDisplayQueue, addMessageToQueue, addRecentChat),
//! taking the history from 100 to 16384 entries. This port maps that class
//! code-for-code; the engine-bound ChatComponent surfaces sit behind
//! `seams`, exactly where the Java mixin's injection points would be.

pub mod mixins;
pub mod seams;

// java: public class MoreChatHistory implements ClientModInitializer {
//           (empty initializer - the mod carries no runtime logic of its
//            own; everything is the mixin) }
