# screenshot-to-clipboard (Horizon port)

Slice 1 of a Horizon port of [Screenshot to Clipboard](https://github.com/comp500/ScreenshotToClipboard)
by comp500 (MIT, (c) 2018 comp500; upstream read at commit 8ea7131, mod
1.0.10). Credits and license preserved; the upstream LICENSE is carried
verbatim.

## What slice 1 holds (1:1 logic core, 13 passing tests)

- **Screenshot pipeline** (`screenshot_to_clipboard.rs`): the mac gate on
  both AWT paths, the RGBA-only format gate with upstream's warning text,
  the exact-size byte extraction ("Invalid image" on a short or null
  buffer), and the clipboard image model mirroring upstream's AWT
  construction: CS_sRGB component color model, 8 bits per channel, alpha
  dropped due to JDK-8204187, band offsets {0, 1, 2}, interleaved raster
  with scanline stride width * components. The components=3 framebuffer
  path (the fabrishot hook) is covered too.
- **Transferable** (`screenshot_to_clipboard.rs`): the image-flavor-only
  transferable, other flavors rejected.
- **macOS pasteboard flow** (`macos_compat.rs`): clear-then-write flow
  with the mac gate and the "Failed to write image to pasteboard!" error.
- **AWT setup** (`awt_setup.rs`): the headless-forcing decision with the
  upstream println, and the init() clipboard check with the
  "java.awt.headless property was not set properly!" warning.

## Seams (platform-coupled surfaces, slice 2+)

- The system clipboard write (AWT Toolkit on Windows/Linux) and the objc
  NSPasteboard chain on macOS are platform glue; the port models them as
  a sink/trait at the same flow points.
- The screenshot hook (upstream injects into ScreenshotRecorder before
  the file save on Windows/Linux, after it on macOS) lands with Horizon's
  screenshot capture surface, same gap tracked for fabrishot on
  horizon-rust/horizon#9606.
- Upstream runs the copy on a background "Screenshot to Clipboard Copy"
  thread; the port builds the clipboard image inline.
