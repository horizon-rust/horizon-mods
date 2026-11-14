# Crosshair Indicator

crosshair-indicator (Horizon Rust port), slice 1: the progress model. NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.  1:1 port of the upstream logic core (CC0-1.0): the crosshair swaps between variants (vanilla cross, attack-indicator arc, break-progress arc) based on what the player is doing, and colors follow the vanilla attack strength. Render hooks land in the next slice behind the same seam.

Port by sparticle. Upstream: https://modrinth.com/mod/crosshair-indicator (CC0-1.0).
