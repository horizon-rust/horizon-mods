# ClearDespawn

cleardespawn (Horizon Rust port), slice 1: the despawn filter model. NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.  1:1 port of the upstream logic core (MIT): when an item entity despawns, its poof particles are suppressed, except for entity ids on the ignore list. Sound and render seams land in the next slice.

Port by sparticle. Upstream: https://github.com/StrikerRockers-Mods/ClearDespawn/ (MIT).
