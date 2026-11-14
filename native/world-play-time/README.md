# World Play Time

world-play-time (Horizon Rust port), slice 1: the playtime model. NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.  1:1 port of the upstream logic core (MIT): playtime accumulates per world (per save and per server) in seconds, is persisted through a JSON store the upstream keeps per world, and is rendered by the same human formatting the mod uses. HUD/overlay seams land in the next slice.

Port by sparticle. Upstream: https://github.com/Khajiitos/WorldPlayTime (MIT).
