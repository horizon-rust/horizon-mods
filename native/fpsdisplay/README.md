# fpsdisplay (Horizon native mod)

A 1:1 Rust port of [fpsdisplaymod](https://modrinth.com/mod/fpsdisplay) (MIT):
an FPS readout that shows the current frames per second, and with advanced
stats the minimum, average and maximum of the last seven seconds, at your
chosen corner, size, colour and alpha. The package is the `.hmod` source: a
native mod built against the pinned Horizon SDK (`api 0.2`, game 26.3).

## Behavior (the original's, kept)

- The readout draws the current fps, plain or with the advanced stats line.
- The position, text colour, opacity, shadow and the advanced toggle are the
  original's config fields, held as settings on Horizon's Options > Mods
  page instead of the original's config screen.
- The readout hides with the HUD (the hud mode read).

## Wiring

- register: the original's config fields registered as settings, same
  defaults (`enabled`, `advanced_stats`, `text_size`, `text_color`,
  `text_alpha`, `text_shadows`, `offset_top`, `offset_left`,
  `keybind_mode`), labels in `assets/fpsdisplay/lang/`.
- register_gui_layers + gui_layer_draw: the readout's own HUD layer above
  all others, drawn once a frame.
- The samples come from the frame interval the draw call measures between
  two of its own calls; the history is the original's FIFO cap at 420.

## Horizon seams

- No client fps read in the mod API, so the readout measures its own frame
  interval instead. Ask filed on horizon-rust/horizon (see below).
- The text draw takes no scale, so the original's text-size setting reads
  but does not scale the glyphs yet.
- The original's keybind (toggle or push-to-show) has no keybinding
  registration API yet; the `keybind_mode` setting is on file and the key
  half of the behaviour waits on the API. Until then the `enabled` setting
  gates the readout.
- The original skips drawing while the debug overlay (F3) is visible; the
  API answers the hud mode only, so that one check waits too.

The asks are tracked on horizon-rust/horizon as issue #18692 and each one
upgrades the port back to full parity when patched.

## License

Upstream fpsdisplaymod: MIT. This port carries the same license; see
LICENSE.
