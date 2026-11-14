# smooth-scroll (1:1 port)

Exact 1:1 Rust translation of [Smooth Scrolling](https://modrinth.com/mod/smooth-scroll)
(upstream Java: SmajloSlovakian/Minecraft-Smooth-Scrolling @639d45b), which makes
scrolling in lists, inventories and the chat smooth instead of jumping slot to slot.

Upstream copyright (C) SmajloSlovakian, licensed under GPL-3.0-only:
https://www.gnu.org/licenses/gpl-3.0.txt

The Rust translation carries the same GPL-3.0-only license and credits the
upstream author in the LICENSE shipped alongside. It is a Horizon native mod:
mixins/smooth_scroll/runtime.rs is a runtime splice of the full 1:1 module set
(src/), with crate:: mapped to super:: for the carrier, built against the
revisions pinned in compatibility.json (Minecraft 26.3, Horizon
3fb4c3138e0c5d8fdd2fdb555cf31368e8897497, Horizon Mixins
d9c83d7096b7ea5b83678da2efdfd6161f74997e).

Ported by sparticle.
