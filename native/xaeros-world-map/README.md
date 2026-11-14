# Xaero's World Map

xaeros-world-map runtime (behavioral port of xaero96/Xaero's World Map, NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format. All Rights Reserved; no upstream code is included).  Ported 1:1 against the behavior the mod's page documents: the self-writing fullscreen map (M to open, mouse pan, wheel zoom), multiple maps per server address with manual or automatic switching, cave-dimension layers, the vanilla-maps mode and terrain shading flags, and "footsteps" showing the most recent path taken. The waypoint overlay rides the xaeros-minimap port's waypoint store; rendering and world reads ride Horizon (seams in the README).

Port by sparticle. Upstream: https://modrinth.com/mod/xaeros-world-map (CC0-1.0).
