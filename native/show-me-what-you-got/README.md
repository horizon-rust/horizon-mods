# Show Me What You Got

show-me-what-you-got ported to Horizon as a native Rust mod.  Upstream: https://github.com/apace100/show-me-what-you-got at commit 0ee8852 (2023-11-18), MIT (c) 2022 apace100. 1:1 port of the mod core: the SmwygItemMatch model, the per-sender MessageItemList (take-on-read, cleared on server start), the [[smwyg:...]] tag regex + SNBT extraction and chat decoration, the share-item packet (varint start/end + compound NBT) with the server receiver's deferred add, the shared chat text-field insertion state machine (cursor/selection/erase/write clamping math from TextFieldWidgetMixin, suggestion-offset tracking, stale-link handling), the chat screen send flow (tagged history entry, packet send, no history duplicate), and the client tick share gate (left-ctrl + chat key opens the chat with the focused slot's stack, latch semantics 1:1).

Port by sparticle. Upstream: https://github.com/apace100/show-me-what-you-got (MIT).
