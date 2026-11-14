# MoreChatHistory (Horizon port)

An exact 1:1 Rust translation of
[MoreChatHistory](https://github.com/JackFred2/MoreChatHistory)
(JackFred, CC0-1.0) for Horizon. Raises the chat history cap from vanilla's
100 entries to 16384, for the display queue, the send queue and the
recent-chat list.

Upstream is one mixin: `ChatComponentMixin.morechathistory_changeMaxHistory`
adds 16284 to the vanilla 100-message cap wherever the chat component enforces
it (addMessageToDisplayQueue, addMessageToQueue, addRecentChat), taking the
history from 100 to 16384 entries. This port maps that class code-for-code;
the engine-bound ChatComponent surfaces sit behind `seams`, and nothing else
about the chat paths changes.

Credited to sparticle.
