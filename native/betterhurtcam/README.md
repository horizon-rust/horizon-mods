# BetterHurtCam

betterhurtcam (Horizon Rust port), slice 1: the shake model. NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.  1:1 port of the upstream logic core (MIT): on damage the camera gets a shake whose intensity comes from the hurt direction and a configurable multiplier, decays over a configurable duration, and repeats a configurable number of times. Game-facing hooks (camera rotation write) land in the next slice behind the same seam upstream uses.

Port by sparticle. Upstream: https://github.com/uku3lig/betterhurtcam (MIT).
