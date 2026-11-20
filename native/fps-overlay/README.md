# FPS Counter

Horizon's official FPS Counter, an official Horizon Extra: the frames of the last second in a small box in a corner of
the screen, and, if you like, the slowest frame and the 1% low of the last 5 seconds. It looks as the extra did: white
text with a shadow on a translucent black box, 2 GUI pixels from the edge, scaled with the GUI.

## Install

In the Horizon launcher: **Mods**, find FPS Counter, **Install**. It is a WebAssembly mod, so nothing is built on your
computer.

## Settings

**Options > Mods > FPS Counter > Config:** the corner (top left by default) and the minimum and 1% low (off). Kept in
`horizon-mods/.horizon-mod-data/fps_overlay/` (`corner`, `lows`), the same files earlier versions used. A corner
earlier versions stored by name (`bottom_right`) is read as before and rewritten as the option's number the first time
the box is drawn.

## Known limits

- The SDK doesn't tell a mod when the debug screen (F3) is open, so the box stays in its corner over the debug text,
  where the extra and 2.0.0 gave way to it.
- The SDK doesn't give a mod `horizon-extras.txt`, so the extra's own settings (`fpsCorner`, `fpsLows`) aren't read; the
  files of the 1.x and 2.x mods are.

## Build

With [`cargo horizon-mod`](https://github.com/horizon-rust/horizon/tree/main/docs/modding):

```sh
cargo test                       # the counter, the text and the settings
cargo horizon-mod build --wasm   # target/horizon-mod/fps_overlay.hmod
cargo horizon-mod check
```

## Earlier versions (legacy)

2.0.0 was a native `.hnmod` built into the game by the launcher, and 1.0.0 an API 0.1 WebAssembly mod. Launchers from
0.2.70 on can't load either, and their install and build steps no longer apply. Their source is in this folder's
history.

## Licence

MIT, see [LICENSE](LICENSE).
