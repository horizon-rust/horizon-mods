# Better Ping Display

better-ping-display (Horizon Rust port), slice 1: the ping model. NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.  1:1 port of the upstream logic core (MIT): a smoothed ping value per player, colored by thresholds, rendered as "+<ms> ms" text next to the vanilla bar in the player list. List-attach seams land in the next slice.

Port by sparticle. Upstream: https://github.com/vladmarica/better-ping-display (MIT).
