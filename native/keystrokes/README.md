# Keystrokes

Horizon's official Keystrokes, an official Horizon Extra: your movement keys, mouse buttons and space bar in a corner of
the screen, lit while held, with clicks per second under the mouse buttons. It looks as the extra did: 22-pixel keys
2 GUI pixels apart, translucent black with white shadowed text, light with dark text while held, 2 GUI pixels from the
edge, scaled with the GUI. The movement keys show the keys you bound them to in Options > Controls.

## Install

In the Horizon launcher: **Mods**, find Keystrokes, **Install**. It is a WebAssembly mod, so nothing is built on your
computer.

## Settings

**Options > Mods > Keystrokes > Config:** the corner (top left by default) and the clicks per second (on). Kept in
`horizon-mods/.horizon-mod-data/keystrokes/` (`corner`, `cps`), the same files earlier versions used. A corner earlier
versions stored by name (`bottom_right`) is read as before and rewritten as the option's number the first time the
keys are drawn.

The first time the keys are drawn, the extra's own settings in `horizon-extras.txt` (`keystrokesCorner`,
`keystrokesCps`, through the SDK's `official_extra_settings`) fill in whichever of the two the mod has no file for yet;
a choice made in an earlier version of the mod stays. Once every one of them is kept, `horizon-mod-data/keystrokes/extras-read` is
written, so it happens once (a setting the game didn't keep is tried again the next time you play); while the game has no extra settings to give, it looks again (once) the next time you play. The extra's
switch (`keystrokes`) isn't taken: the mod's switch in Options > Mods is the one that counts.

While the debug screen (F3) is open the keys give way to it, as 2.0.0 did: none in the top corners (F3's two columns),
nor in a bottom corner while one of F3's charts is drawn there.

## Clicks per second

Each press of the left or right mouse button while you play, in the last second, as the game counted them for the
extra, on the button you pressed (with attack on the right button, the right column counts it). A key, a controller's
trigger or a scripted press (`horizon-ctl mouse left down`) counts too: a press of the attack or use mapping in a frame
with no mouse press for it counts once, when the mapping goes down. Each mouse press stands for one mapping only,
so a left click and a key bound to use in the same frame are two clicks. A press released before the next frame (`horizon-ctl
mouse left click`) is not seen.

## Build

With [`cargo horizon-mod`](https://github.com/horizon-rust/horizon/tree/main/docs/modding):

```sh
cargo test                       # the layout, the click counter and the settings
cargo horizon-mod build --wasm   # target/horizon-mod/keystrokes.hmod
cargo horizon-mod check
```

## Earlier versions (legacy)

2.0.0 was a native `.hnmod` built into the game by the launcher, and 1.0.0 an API 0.1 WebAssembly mod. Launchers from
0.2.70 on can't load either, and their install and build steps no longer apply. Their source is in this folder's
history.

## Licence

MIT, see [LICENSE](LICENSE).
