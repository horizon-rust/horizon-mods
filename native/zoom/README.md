# Zoom

> [!WARNING]
> **Legacy.** This is the old `.hnmod` source. Launchers from 0.2.70 on can't load it, and the registry archived it on
> 2026-11-18. A port to the new mod SDK is tracked in [#1214](https://github.com/horizon-rust/horizon-mods/issues/1214).
> The install steps below no longer work.

Horizon's official Zoom: hold a key (Z by default) to zoom in, and turn the mouse wheel while you hold it to zoom
further in or out (1.25x to 10x). The hotbar stays put while you zoom.

## Install

In the Horizon launcher: **Mods**, find Zoom, **Install**. It is a WebAssembly mod, so nothing is built on your
computer.

## Settings

- **Key:** Options > Controls, under Zoom. Kept as `key.zoom.zoom:…` in `horizon-mods.txt`.
- **Options > Mods > Zoom > Config:** starting zoom (2x to 10x, default 4x), smooth zoom (eased like the spyglass,
  on), slower turning while zoomed (on). Kept in `horizon-mods/.horizon-mod-data/zoom/` (`level`, `smooth`,
  `sensitivity`), the same files earlier versions used.

## Build

With [`cargo horizon-mod`](https://github.com/horizon-rust/horizon/tree/main/docs/modding):

```sh
cargo test                       # the zoom logic
cargo horizon-mod build --wasm   # target/horizon-mod/zoom.hmod
cargo horizon-mod check
```

## Licence

MIT, see [LICENSE](LICENSE).
