# horizon-blur-plus

Blur+ (Motschen/Blur v6.3.1) ported to [Horizon](https://github.com/horizon-rust/horizon) as a native Rust mod.

Upstream: github.com/Motschen/Blur, commit 7f57397, MIT (c) 2017 tterrag. LICENSE carried verbatim.

## The port

1:1 logic core, no optimizations or behavior changes:

- Config + v3 config migration (`blur.json`), MidnightConfig-style key handling
- The ten easing curves and the animation state machine (blur radius, background alpha, gradient)
- HSB/hex color math and the packed gradient model
- The render-pass flow (onRender / renderBackground / onRenderEnd) with draw specs returned for the client
- Screen gates from every upstream mixin (title, container, sign, book, death, command block, options)
- The options coefficient and the debug HUD renderer

31 unit tests green (config, migration, easing, animation, gates, render flow).

## Known degradations

- The drawing itself (background strata, gradients, text) is a seam: the port emits draw specs; the client applies them. Gated on the screen-background render hook tracked in horizon-rust/horizon #10131.
