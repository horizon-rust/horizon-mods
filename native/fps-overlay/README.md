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

The first time the box is drawn (from 3.0.1), the extra's own settings in `horizon-extras.txt` (`fpsCorner`,
`fpsLows`, through the SDK's `official_extra_settings`) fill in whichever of the two the mod has no file for yet; a
choice made in an earlier version of the mod stays. Once every one of them is kept, `horizon-mod-data/fps_overlay/extras-read` is
written, so it happens once (a setting the game didn't keep is tried again the next time you play); while the game has no extra settings to give, it looks again (once) the next time you play. The extra's
switch (`fps`) isn't taken: the mod's switch in Options > Mods is the one that counts.

While the debug screen (F3) is open the box gives way to it, as 2.0.0 did: none in the top corners (F3's two columns),
nor in a bottom corner while one of F3's charts is drawn there.

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
