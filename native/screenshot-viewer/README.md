# Screenshot Viewer

screenshot-viewer (Horizon Rust port), slice 1: the gallery model. NOTE: the master repo keeps this port as src/lib.rs + modules; this bundle inlines the modules into one runtime.rs file, compile-equivalent, for the single-module .hnmod format.  1:1 port of the upstream logic core (MIT): screenshots are listed from the screenshot folder newest-first, paged through a fixed-size grid, and the selected one can be deleted or copied to the clipboard. The image render surface and clipboard OS hook land in the next slice.

Port by sparticle. Upstream: https://github.com/LGatodu47/ScreenshotViewer (MIT).
