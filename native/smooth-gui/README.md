# Smooth Gui

smooth-gui (Horizon Rust port), slice 1: the animation model. NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.  1:1 port of the upstream logic core (MIT): screens ease in over a configurable duration with configurable slide/fade amounts; the animation state machine drives open, close and cancel (instantly closing a screen cancels without playing the close animation). Screen callbacks land in the next slice.

Port by sparticle. Upstream: https://github.com/Ezzenix/SmoothGui (MIT).
